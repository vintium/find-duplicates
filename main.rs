use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fmt::Write as OtherWrite;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;

use adler32::adler32;

use futures::future::join_all;

use tokio::runtime::{Runtime, Handle};


fn usage(pn: &str) {
   println!("USAGE: {} [flags] <input>", pn);
   println!("  where [flags] can be 0 or more of the following:");
   println!("    -r, --recursive      include files in subdirectories,");
   println!("                         search recursively.");
   println!();
   println!("    -v, --verbose        enable progress bars and other");
   println!("                         extra output. cannot be used with");
   println!("                         -q, --quiet.");
   println!();
   println!("    -q, --quiet          disable all non-essential output,");
   println!("                         good for redirecting to files or");
   println!("                         piping to other programs. cannot");
   println!("                         be used with -v, --verbose");
   println!();
   println!("    -h, --help           print this message.");
   println!();
   println!("  and where <input> is a path to a directory.");
}

// TODO: consider factoring target_dir out of options since it's
// more like an argument than a flag
#[derive(Debug)]
struct Options {
   target_dir: PathBuf,
   verbose: bool,
   recursive: bool,
   quiet: bool,
}

impl Options {
   fn default() -> Options {
      Options {
         target_dir: PathBuf::from(""),
         verbose: false,
         quiet: false,
         recursive: false,
      }
   }
}

fn parse_args(mut args: env::Args) -> Options {
   let program_name = args.next().expect("program name 0th element of args");
   let mut res = Options::default();
   for arg in args {
      match arg.as_str() {
         "-v" | "--verbose" => {
            if res.quiet {
               usage(&program_name);
               eprintln!("ERROR: incompatible flags: cannot be quiet and verbose.");
               process::exit(1);
            }
            res.verbose = true;
         },
         "-q" | "--quiet" => {
            if res.verbose {
               usage(&program_name);
               eprintln!("ERROR: incompatible flags: cannot be quiet and verbose.");
               process::exit(1);
            }
            res.quiet = true;
         },
         "-r" | "--recursive" => res.recursive = true,
         "-h" | "--help" => {
            usage(&program_name);
            process::exit(1);
         },
         otherwise => {
            let maybe_path = PathBuf::from(otherwise);
            if maybe_path.is_dir() {
               res.target_dir = maybe_path;
            } else {
               usage(&program_name);
               eprintln!("ERROR: no such directory or flag: {}", otherwise);
               process::exit(1);
            }
         },
      }
   }

   if res.target_dir.to_str().unwrap() == "" {
      usage(&program_name);
      eprintln!("ERROR: no directory provided.");
      process::exit(1);
   }
   res
}

/*
  I'm using 'file identifier' to mean a number that is shared across (hard  or
  soft) linked files.
*/

// on unix, we can use the inode number as a file identifier.
#[cfg(unix)]
fn get_file_identifier(fp: &Path) -> u64 {
   /* NOTE: this function expects the path passed in to
   have been pre-verified to exist. */
   use std::os::unix::fs::MetadataExt;
   let md = fs::metadata(fp).unwrap();
   md.ino()
}

// on windows, we can use the nFileIndex{Low,High} as a file identifier.
#[cfg(windows)]
fn get_file_identifier(fp: &Path) -> u64 {
   /* NOTE: this function expects the path passed in to
   have been pre-verified to exist. */
   use std::os::windows::fs::MetadataExt;
   todo!("This function is untested! also, it needs nightly!");
   let md = fs::metadata(fp).unwrap();
   md.file_index().unwrap()
}

type EntriesByIdentifiers = HashMap<u64, Vec<fs::DirEntry>>;
type LinkedGroup = (
   u64,               /* file identifier */
   Vec<fs::DirEntry>, /* files linked to the identifier */
);

fn is_symlink_to_dir(de: &fs::DirEntry) -> std::io::Result<bool> {
   Ok(de.metadata()?.is_symlink() && fs::metadata(de.path())?.is_dir())
}

// TODO: Make this more idiomatic, use iterators the whole way thru
fn rec_read_dir(de: fs::DirEntry, acc: &mut EntriesByIdentifiers) {
   if de.metadata().expect("failed to stat").is_dir() {
      match de.path().read_dir() {
         Ok(rd) => {
            for md in rd {
               rec_read_dir(md.expect("failed to stat"), acc);
            }
         },
         Err(e) => {
            eprintln!("Error: could not read directory {:?}: {}", de.path(), e);
         },
      }
   } else if is_symlink_to_dir(&de).expect("failed to stat") {
      /* ignore symlink directories */
   } else {
      let fi = get_file_identifier(&de.path());
      if acc.contains_key(&fi) {
         acc.get_mut(&fi).unwrap().push(de);
      } else {
         acc.insert(fi, vec![de]);
      }
      print!("Building file list... {} \r", acc.len());
   }
}

fn build_file_list(options: &Options) -> Vec<LinkedGroup> {
   if !options.quiet {
      print!("Building file list... \r");
   }

   if options.recursive {
      let mut acc = EntriesByIdentifiers::new();
      for md in options.target_dir.read_dir().expect("read_dir call failed") {
         rec_read_dir(md.expect("failed to stat"), &mut acc);
      }
      if !options.quiet {
         println!("\nFound {} files.", acc.len());
      }
      acc.drain().collect()
   } else {
      let mut acc = EntriesByIdentifiers::new();

      let _ = options
         .target_dir
         .read_dir()
         .expect("read_dir call failed")
         .enumerate()
         .map(|(i, a)| {
            print!("Building file list... {}\r", i);
            a.expect("failed to stat")
         })
         .filter(|a| !fs::metadata(a.path()).expect("failed to stat").is_dir())
         .map(|a| {
            let fi = get_file_identifier(&a.path());
            if acc.contains_key(&fi) {
               acc.get_mut(&fi).unwrap().push(a);
            } else {
               acc.insert(fi, vec![a]);
            }
         })
         .count(); /* use `count` to exhaust this
                   iterator and run each iteration */
      let res: Vec<LinkedGroup> = acc.drain().collect();
      println!("Building file list... {}", res.len());
      if !options.quiet {
         println!("Found {} files.", res.len());
      }
      res
   }
}

/*
   I'm using the term 'sizewise dup' to describe 2 or more files which
   share the same size, therefore appearing to be duplicates from a
   sizewise perspective.
*/

// a map whose keys are filesizes and whose values are vecs of files with a
// given size.          /* TODO consider changing to set */
type SizewiseDups = HashMap<u64, Vec<LinkedGroup>>;

fn find_sizewise_dups(mut files: Vec<LinkedGroup>) -> SizewiseDups {
   // keep track of how many files we started with for logging
   let amt_files = files.len();
   // keep track of sizes for which 2 or more files have been found
   let mut dup_sizes: HashSet<u64> = HashSet::new();
   // build map of filesizes to lists of files with that size
   let mut maybe_dups: SizewiseDups = HashMap::new();
   for (n, de) in files.drain(..).enumerate() {
      print!("Size-checking {}/{} files...\r", n, amt_files);
      let md = de.1[0].metadata().expect("failed to stat");
      // it would be an error if there were directories in the file list
      assert!(!md.is_dir());
      let fsize = md.len();
      if maybe_dups.contains_key(&fsize) {
         maybe_dups.get_mut(&fsize).unwrap().push(de);
         dup_sizes.insert(fsize);
      } else {
         maybe_dups.insert(fsize, vec![de]);
      }
   }
   println!("Size-checked {}/{} files.          ", amt_files, amt_files);
   // collect all of the size-wise dups we found
   let mut res: SizewiseDups = HashMap::new();
   for dup_size in dup_sizes {
      res.insert(dup_size, maybe_dups.remove(&dup_size).unwrap());
   }
   res
}

/*
// Adler32 algorithm and implementation taken from here:
// https://en.wikipedia.org/wiki/Adler-32#Example_implementation
const MOD_ADLER: u32 = 65521;
fn my_adler32(data: Vec<u8>) -> u32 {
  let mut a: u32 = 1;
  let mut b: u32 = 0;
  for byte in data {
    a = (a + (byte as u32)) % MOD_ADLER;
    b = (b + a) % MOD_ADLER;
  }

  (b << 16) | a

}
*/

fn calc_file_checksum(f: &fs::DirEntry) -> u32 {
   adler32(fs::File::open(f.path()).unwrap()).unwrap()
}

fn calc_file_checksumst(mut fs: Vec<LinkedGroup>) -> Vec<(u32, LinkedGroup)> {
   let mut paths: Vec<PathBuf> = fs.iter().map(|a| a.1[0].path().to_owned()).collect();
   let mut ts: Vec<thread::JoinHandle<(usize, u32)>> = vec![];
   for (idx, path) in paths.drain(..).enumerate() {
      ts.push(thread::spawn(move || {
         let r = adler32(fs::File::open(path).unwrap()).unwrap();
         (idx, r)
      }));
   }
   let results: Vec<(usize, u32)> = ts.drain(..).map(|t| t.join().unwrap()).collect();
   let mut removable: HashMap<usize, LinkedGroup> = HashMap::new();
   fs.drain(..)
      .enumerate()
      .map(|(idx, lg)| removable.insert(idx, lg))
      .count();
   let res: Vec<(u32, LinkedGroup)> = results
      .iter()
      .map(|(idx, checksum)| (*checksum, removable.remove(idx).unwrap()))
      .collect();
   res
}

async fn calc_file_checksuma(f: LinkedGroup) -> (u32, LinkedGroup) {
    let p = f.1[0].path().to_owned();
    let bytes_of_file: Vec<u8> = tokio::fs::read(p).await.unwrap();
    (adler32(bytes_of_file.as_slice()).unwrap(), f)
}

fn calc_file_checksumsa(mut fs: Vec<LinkedGroup>, handle: &Handle) -> Vec<(u32, LinkedGroup)> {
   let futures = fs.drain(..).map(|f| calc_file_checksuma(f));
   handle.block_on(join_all(futures))
}


/*
   I'm using the term 'dup' to describe 2 or more files which
   share the same checksum, therefore appearing to be duplicates from a
   checksumwise perspective.
*/

// a map whose keys are checksums and whose values are vecs of files with a
// given checksum.     /* TODO consider changing to set */
type Dups = HashMap<u32, Vec<LinkedGroup>>;

fn filter_non_dups(mut sizewise_dups: SizewiseDups) -> Dups {
   let mut calculation_count: usize = 0;
   let total = sizewise_dups.values().flatten().count();
   let grps = sizewise_dups.len();
   // keep track of checksums for which 2 or more files have been found
   let mut dup_checksums: HashSet<u32> = HashSet::new();
   // build map of checksums to lists of files with that checksum
   let mut maybe_dups: Dups = HashMap::new();
   let rt = tokio::runtime::Runtime::new().unwrap();
   for (grp, (size, mut files)) in sizewise_dups.drain().enumerate() {
      assert!(files.len() > 1);

      // multithreaded approach
      /*
      print!(
         "(group {}/{}): calculating checksums of {} files with size {}...\r",
         grp,
         grps,
         files.len(),
         size
      );
      std::io::stdout().flush().unwrap();
      calculation_count += files.len();
      let mut cs = calc_file_checksumst(files);
      for (checksum, fil) in cs.drain(..) {
         if maybe_dups.contains_key(&checksum) {
            maybe_dups.get_mut(&checksum).unwrap().push(fil);
            dup_checksums.insert(checksum);
         } else {
            maybe_dups.insert(checksum, vec![fil]);
         }
      }
      */

      // async approach
      // /*
      print!(
         "(group {}/{}): calculating checksums of {} files with size {}...\r",
         grp,
         grps,
         files.len(),
         size
      );
      std::io::stdout().flush().unwrap();
      calculation_count += files.len();
      let mut cs = calc_file_checksumsa(files, rt.handle());
      for (checksum, fil) in cs.drain(..) {
         if maybe_dups.contains_key(&checksum) {
            maybe_dups.get_mut(&checksum).unwrap().push(fil);
            dup_checksums.insert(checksum);
         } else {
            maybe_dups.insert(checksum, vec![fil]);
         }
      }
      // */

      /*
      // singlethreaded approach
      for file in files.drain(..) {
        print!("Calculating checksum {}/{}...\r", calculation_count, total);
        std::io::stdout().flush().unwrap();
        calculation_count += 1;
        let fchecksum = calc_file_checksum(&(file.1[0]));
        if maybe_dups.contains_key(&fchecksum) {
          maybe_dups.get_mut(&fchecksum).unwrap().push(file);
          dup_checksums.insert(fchecksum);
        } else {
          maybe_dups.insert(fchecksum, vec![file]);
        }
      }
      */
   }
   println!(
      "Calculated checksums of {} files.                                      ",
      calculation_count
   );
   // collect all of the dups we found
   let mut res: Dups = HashMap::new();
   for dup_checksum in dup_checksums {
      res.insert(dup_checksum, maybe_dups.remove(&dup_checksum).unwrap());
   }
   res
}

fn fmt_linkedgroup(lg: &LinkedGroup) -> String {
   let mut acc = String::new();
   write!(acc, "{:?}", lg.1[0].path().as_os_str().to_string_lossy()).unwrap();
   if lg.1.len() > 1 {
      write!(acc, " (aka ").unwrap();
   }
   for idx in 1..(lg.1.len() - 1) {
      let de = &lg.1[idx];
      write!(acc, "{:?}, ", de.path().as_os_str().to_string_lossy()).unwrap();
   }
   if lg.1.len() > 1 {
      write!(
         acc,
         "{:?})",
         lg.1[lg.1.len() - 1].path().as_os_str().to_string_lossy()
      )
      .unwrap();
   }
   acc
}

fn print_dups(ds: &Dups) {
   for d in ds {
      println!("files with checksum {}:", d.0);
      for lg in d.1 {
         println!("  {}", fmt_linkedgroup(&lg));
      }
   }
}

fn main() {
   let options = parse_args(env::args());
   let file_list = build_file_list(&options);
   let sizewise_dups = find_sizewise_dups(file_list);
   println!(
      "Found {} groups of files with equal sizes. {} files total.",
      sizewise_dups.len(),
      sizewise_dups.values().flatten().count()
   );
   let dups = filter_non_dups(sizewise_dups);
   println!("Found {} duplicates.", dups.len());
   print_dups(&dups);
}
