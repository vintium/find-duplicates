#![feature(windows_by_handle)]

use find_duplicates::metafile::MetaFile;
use find_duplicates::recursive_dir_reader::RecReadDir;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use adler32::adler32;

use rayon::prelude::*;

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
    target_dirs: Vec<PathBuf>,
    verbose: bool,
    recursive: bool,
    quiet: bool,
}

impl Options {
    fn default() -> Options {
        Options {
            target_dirs: Vec::new(),
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
            }
            "-q" | "--quiet" => {
                if res.verbose {
                    usage(&program_name);
                    eprintln!("ERROR: incompatible flags: cannot be quiet and verbose.");
                    process::exit(1);
                }
                res.quiet = true;
            }
            "-r" | "--recursive" => res.recursive = true,
            "-h" | "--help" => {
                usage(&program_name);
                process::exit(1);
            }
            otherwise => {
                let maybe_path = PathBuf::from(otherwise);
                if maybe_path.is_dir() {
                    res.target_dirs.push(maybe_path);
                } else {
                    usage(&program_name);
                    eprintln!("ERROR: no such directory or flag: {}", otherwise);
                    process::exit(1);
                }
            }
        }
    }

    if res.target_dirs.is_empty() {
        usage(&program_name);
        eprintln!("ERROR: no directories provided.");
        process::exit(1);
    }
    res
}

/*
  TERM: I'm using 'file identifier' to mean a number that is shared across (hard  or
  soft) linked files.
*/

#[cfg(unix)]
fn get_file_identifier(fp: &Path) -> u64 {
    /* on unix, we can use the inode number as a file identifier. */
    use std::os::unix::fs::MetadataExt;
    /* NOTE: this function expects the path passed in to
    have been pre-verified to exist. */
    let md = fs::metadata(fp).unwrap();
    md.ino()
}

#[cfg(windows)]
fn get_file_identifier(fp: &Path) -> u64 {
    /* on windows, we can use the nFileIndex{Low,High} as a file identifier. */
    use std::os::windows::fs::MetadataExt;
    /* NOTE: this function expects the path passed in to
    have been pre-verified to exist. */
    let md = fs::metadata(fp).unwrap();
    md.file_index().unwrap()
}

type EntriesByIdentifiers = HashMap<u64, Vec<fs::DirEntry>>;

fn collect_into_entries_by_identifiers<I>(acc: &mut EntriesByIdentifiers, read_dir_iterator: I)
where
    I: Iterator<Item = std::io::Result<fs::DirEntry>>,
{
    let _ = read_dir_iterator
        .enumerate()
        .map(|(i, a)| {
            print!("Building file list... {}\r", i);
            a
        })
        .filter_map(Result::ok)
        .filter(|a| !fs::metadata(a.path()).expect("failed to stat").is_dir())
        .map(|a| {
            let fi = get_file_identifier(&a.path());
            match acc.entry(fi) {
                Entry::Occupied(mut e) => {
                    e.get_mut().push(a);
                }
                Entry::Vacant(e) => {
                    e.insert(vec![a]);
                }
            }
        })
        .count(); /* use `count` to exhaust this
                  iterator and run each iteration */
}

fn build_file_list(options: &Options) -> Vec<MetaFile> {
    if !options.quiet {
        print!("Building file list... \r");
    }
    let mut acc = EntriesByIdentifiers::new();
    for target_dir in &options.target_dirs {
        if options.recursive {
            let read_dir_iterator = RecReadDir::new(target_dir).expect("read_dir call failed");
            collect_into_entries_by_identifiers(&mut acc, read_dir_iterator);
        } else {
            let read_dir_iterator = target_dir.read_dir().expect("read_dir call failed");
            collect_into_entries_by_identifiers(&mut acc, read_dir_iterator);
        }
    }
    let res: Vec<MetaFile> = acc
        .drain()
        .map(|(id, files)| MetaFile::new(id, files))
        .collect();
    println!("Building file list... {}      ", res.len());
    if !options.quiet {
        println!("Found {} files.", res.len());
    }
    res
}

/*
   I'm using the term 'sizewise dup' to describe 2 or more files which
   share the same size, therefore appearing to be duplicates from a
   sizewise perspective.
*/

// a map whose keys are filesizes and whose values are vecs of files with a
// given size.          /* TODO consider changing to set */
type SizewiseDups = HashMap<u64, Vec<MetaFile>>;

fn find_sizewise_dups(mut files: Vec<MetaFile>) -> SizewiseDups {
    // keep track of how many files we started with for logging
    let amt_files = files.len();
    // keep track of sizes for which 2 or more files have been found
    let mut dup_sizes: HashSet<u64> = HashSet::new();
    // build map of filesizes to lists of files with that size
    let mut maybe_dups: SizewiseDups = HashMap::new();
    for (n, de) in files.drain(..).enumerate() {
        print!("Size-checking {}/{} files...\r", n, amt_files);
        let md = de.files()[0].metadata().expect("failed to stat");
        // it would be an error if there were directories in the file list
        assert!(!md.is_dir());
        let fsize = md.len();
        match maybe_dups.entry(fsize) {
            Entry::Occupied(mut e) => {
                e.get_mut().push(de);
                dup_sizes.insert(fsize);
            }
            Entry::Vacant(e) => {
                e.insert(vec![de]);
            }
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

fn calc_file_checksumsr(mut fs: Vec<MetaFile>) -> Vec<(u32, MetaFile)> {
    fs.par_drain(..)
        .map(|f| {
            let p = f.files()[0].path();
            let bytes_of_file: Vec<u8> = std::fs::read(p).unwrap();
            (adler32(bytes_of_file.as_slice()).unwrap(), f)
        })
        .collect()
}

/*
   I'm using the term 'dup' to describe 2 or more files which
   share the same checksum, therefore appearing to be duplicates from a
   checksumwise perspective.
*/

// a map whose keys are checksums and whose values are vecs of files with a
// given checksum.     /* TODO consider changing to set */
type Dups = HashMap<u32, Vec<MetaFile>>;

fn filter_non_dups(mut sizewise_dups: SizewiseDups) -> Dups {
    let mut calculation_count: usize = 0;
    let _total = sizewise_dups.values().flatten().count();
    let grps = sizewise_dups.len();
    // keep track of checksums for which 2 or more files have been found
    let mut dup_checksums: HashSet<u32> = HashSet::new();
    // build map of checksums to lists of files with that checksum
    let mut maybe_dups: Dups = HashMap::new();
    for (grp, (size, files)) in sizewise_dups.drain().enumerate() {
        assert!(files.len() > 1);
        print!(
            "(group {}/{}): calculating checksums of {} files with size {}...\r",
            grp,
            grps,
            files.len(),
            size
        );
        std::io::stdout().flush().unwrap();
        calculation_count += files.len();
        let mut cs = calc_file_checksumsr(files);
        for (checksum, fil) in cs.drain(..) {
            match maybe_dups.entry(checksum) {
                Entry::Occupied(mut e) => {
                    e.get_mut().push(fil);
                    dup_checksums.insert(checksum);
                }
                Entry::Vacant(e) => {
                    e.insert(vec![fil]);
                }
            }
        }
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

fn print_dups(ds: &Dups) {
    for d in ds {
        println!("files with checksum {}:", d.0);
        for lg in d.1 {
            println!("  {}", lg);
        }
    }
}

use atty::Stream;
use std::time::Instant;

fn main() {
    let options = parse_args(env::args());
    let mut start = Instant::now();
    let file_list = build_file_list(&options);
    println!("took: {:?}", start.elapsed());
    start = Instant::now();
    let sizewise_dups = find_sizewise_dups(file_list);
    println!(
        "Found {} groups of files with equal sizes. {} files total.",
        sizewise_dups.len(),
        sizewise_dups.values().flatten().count()
    );
    println!("took: {:?}", start.elapsed());
    start = Instant::now();
    let dups = filter_non_dups(sizewise_dups);
    println!("Found {} duplicates.", dups.len());
    if dups.len() < 25 || !atty::is(Stream::Stdout) {
        print_dups(&dups);
    }
    println!("took: {:?}", start.elapsed());
}
