use std::env;
use std::process;
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use std::collections::HashSet;


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
  println!("    -y, --no-warn        disable large-output warnings.");
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
  warn: bool,
  quiet: bool,
}

impl Options {
  fn default() -> Options {
    Options {
      target_dir: PathBuf::from(""),
      verbose: false,
      quiet: false,
      recursive: false,
      warn: true, 
    }
  }
}


fn parse_args(mut args: env::Args) -> Options {
  let program_name = args.next()
                         .expect("program name 0th element of args");
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
      "-y" | "--no-warn"   => res.warn = false,
      "-h" | "--help"      => {
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
      }
    } 
  }
  
  if res.target_dir.to_str().unwrap() == "" {
    usage(&program_name);
    eprintln!("ERROR: no directory provided.");
    process::exit(1);
  } 
  res
}

// TODO: Make this more idiomatic, use iterators the whole way thru
fn rec_read_dir(de: fs::DirEntry, acc: &mut Vec<fs::DirEntry>) {
  if de.file_type().expect("failed to stat").is_dir() {
    for md in de.path().read_dir().expect("read_dir call failed") {
      rec_read_dir(md.expect("failed to stat"), acc);
    }
  } else {
    acc.push(de);
    print!("Building file list... {} \r", acc.len());
  }
}

fn build_file_list(options: &Options) -> Vec<fs::DirEntry> {
  if !options.quiet {
    print!("Building file list... \r");
  }

  if options.recursive {
    let mut acc = Vec::<fs::DirEntry>::new();
    for md in options.target_dir.read_dir().expect("read_dir call failed") {
      rec_read_dir(md.expect("failed to stat"), &mut acc);
    }
    if !options.quiet {
      println!("\nFound {} files.", acc.len());
    }
    acc
  } else {
    let res: Vec<fs::DirEntry> = options.target_dir
                                  .read_dir()
                                  .expect("read_dir call failed")
                                  .enumerate()
                                  .map(|(i, a)| {
                                    print!("Building file list... {}\r", i);
                                    a.expect("failed to stat")
                                  })
                                  .filter(|a| { 
                                    !a.file_type()
                                      .expect("failed to stat")
                                      .is_dir()
                                  })
                                  .collect();
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

// a map whose keys are filesizes and whose values are sets of files with a
// given size.          /* TODO consider changing to slice or set */
type SizewiseDups = HashMap<u64, Vec<fs::DirEntry>>;

fn find_sizewise_dups(options: &Options,
                      mut files: Vec<fs::DirEntry>) -> SizewiseDups { 
  // keep track of how many files we started with for logging
  let amt_files = files.len();
  // keep track of sizes for which 2 or more files have been found
  let mut dup_sizes: HashSet<u64> = HashSet::new(); 
  // build map of filesizes to lists of files with that size
  let mut maybe_dups: SizewiseDups = HashMap::new();
  for (n, de) in files.drain(..).enumerate() {
    print!("Size-checking {}/{} files...\r", n, amt_files);
    let md = de.metadata().expect("failed to stat");
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
  println!("Size-checked {}/{} files.       ", amt_files, amt_files);
  // collect all of the size-wise dups we found
  let mut res: SizewiseDups = HashMap::new();
  for dup_size in dup_sizes {
    res.insert(dup_size, maybe_dups.remove(&dup_size).unwrap());
  } 
  res
}

fn main() {
  let options = parse_args(env::args());
  println!("{:?}", options);
  let file_list = build_file_list(&options);
  //println!("{:?}", file_list);
  let sizewise_dups = find_sizewise_dups(&options, file_list);
  //println!("{:?}", sizewise_dups);

}




