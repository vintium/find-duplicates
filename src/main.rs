use find_duplicates::metafile::collect_into_metafiles;
use find_duplicates::metafile::MetaFile;
use find_duplicates::recursive_dir_reader::RecReadDir;
use indexmap::indexset;
use indexmap::IndexSet;

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::process;

use adler32::adler32;

use rayon::prelude::*;

fn usage(application_name: &str) {
    println!("USAGE: {} [flags] <input>", application_name);
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
    println!("  and where <input> is one or more paths to directories.");
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

fn build_file_list(options: &Options) -> IndexSet<MetaFile> {
    if !options.quiet {
        print!("Building file list... \r");
    }
    let mut acc: IndexSet<MetaFile> = indexset![];
    for target_dir in &options.target_dirs {
        let read_dir_iterator: Box<dyn Iterator<Item = _>> = if options.recursive {
            Box::new(RecReadDir::new(target_dir).expect("read_dir call failed"))
        } else {
            Box::new(target_dir.read_dir().expect("read_dir call failed"))
        };
        let path_iterator = read_dir_iterator.filter_map(Result::ok).map(|a| a.path());
        collect_into_metafiles(&mut acc, path_iterator, false);
    }
    println!("Building file list... {}      ", acc.len());
    if !options.quiet {
        println!("Found {} files.", acc.len());
    }
    acc
}

/*
   I'm using the term 'sizewise dup' to describe 2 or more files which
   share the same size, therefore appearing to be duplicates from a
   sizewise perspective.
*/

// a map whose keys are filesizes and whose values are vecs of files with a
// given size.          /* TODO consider changing to set */
type SizewiseDups = HashMap<u64, HashSet<MetaFile>>;

fn find_sizewise_dups(files: impl IntoIterator<Item = MetaFile>) -> SizewiseDups {
    let mut files_by_size: SizewiseDups = HashMap::new();
    for f in files {
        let Ok(metadata) = f.paths()[0].metadata() else { continue; };
        // it would be an error if there were directories in the file list
        assert!(!metadata.is_dir());
        let file_size = metadata.len();
        files_by_size
            .entry(file_size)
            .or_insert(HashSet::with_capacity(1))
            .insert(f);
    }
    files_by_size.retain(|_, files| files.len() > 1);
    files_by_size
}

fn calc_file_checksumsr(
    files: impl IntoParallelIterator<Item = MetaFile>,
) -> HashSet<(u32, MetaFile)> {
    files
        .into_par_iter()
        .map(|f| {
            let p = &f.paths()[0];
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
type Dups = HashMap<u32, HashSet<MetaFile>>;

fn find_dups(mut sizewise_dups: SizewiseDups) -> Dups {
    let mut calculation_count: usize = 0;
    let grps = sizewise_dups.len();
    let mut files_by_checksum: Dups = HashMap::new();
    for (grp, (size, files)) in sizewise_dups.drain().enumerate() {
        assert!(files.len() > 1);
        eprint!(
            "(group {}/{}): calculating checksums of {} files with size {}...\r",
            grp,
            grps,
            files.len(),
            size
        );
        calculation_count += files.len();
        let mut checksums = calc_file_checksumsr(files);
        for (checksum, f) in checksums.drain() {
            files_by_checksum
                .entry(checksum)
                .or_insert(HashSet::with_capacity(1))
                .insert(f);
        }
    }
    eprintln!("\nCalculated checksums of {} files.", calculation_count);
    // collect all of the dups we found
    files_by_checksum.retain(|_, files| files.len() > 1);
    files_by_checksum
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
    let dups = find_dups(sizewise_dups);
    println!("Found {} duplicates.", dups.len());
    if dups.len() < 25 || !atty::is(Stream::Stdout) {
        print_dups(&dups);
    }
    println!("took: {:?}", start.elapsed());
}
