#!/usr/bin/env python3
from hashlib import blake2b
from zlib import adler32

import sys
import os
from glob import glob, iglob
from shlex import quote

DEBUG = True
PROGRAM_NAME = "find_duplicates.py"

def hash_digest(fname: str) -> str:
    hash_blake2b = blake2b()
    with open(fname, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_blake2b.update(chunk)
    return hash_blake2b.hexdigest()

def fil_checksum(fname: str) -> str:
    with open(fname, "rb") as f:
        d = f.read()
        return adler32(d)


def usage():
    print(f"USAGE: {PROGRAM_NAME} [flags] <input>")
    print( "  where [flags] can be 0 or more of the following:")
    print( "    -r, --recursive            include files in subdirectories,")
    print( "                               search recursively.")
    print()
    print( "    -v, --verbose              enable progress bars and other")
    print( "                               extra output. cannot be used with")
    print( "                               -q, --quiet.")
    print()
    print( "    -q, --quiet                disable all non-essential output,")
    print( "                               good for redirecting to files or")
    print( "                               piping to other programs. cannot be")
    print( "                               used with -v, --verbose")
    print()
    print( "    -y, --no-warn              disable large-output warnings.")
    print()
    print( "    -h, --help                 print this message.")
    print()
    print( "  and where <input> is a path to a directory.")
 


def main() -> int: 
    # Parse Args
    target_directory = ""
    verbose = False
    recursive = False
    warn = True
    quiet = False
    if len(sys.argv) > 5:
        usage()
        print(f"ERROR: too many arguments provided.")
        exit(1)
    for arg in sys.argv[1:]:
        if arg == '-r' or arg == '--recursive':
            recursive = True
        elif arg == '-q' or arg == '--quiet':
            if verbose:
                usage()
                print(f"ERROR: incompatible flags: cannot be verbose and quiet.") 
                exit(1)
            quiet = True
        elif arg == '-h' or arg == '--help':
            usage()
            exit(0)
        elif arg == '-y' or arg == '--no-warn':
            warn = False
        elif arg == '-v' or arg == '--verbose':
            if quiet:
                usage()
                print(f"ERROR: incompatible flags: cannot be verbose and quiet.") 
                exit(1)
            verbose = True
            
        elif arg.startswith('-'):
            usage()
            print(f"ERROR: unrecognized option {arg}")
            exit(1)
        else:
            target_directory = quote(arg)
    
    if target_directory == "":
        usage()
        print("ERROR: no input provided")   
        exit(1)
 
    if not os.path.exists(target_directory):
        usage()
        print(f"ERROR: No such directory: '{target_directory}'") 
        exit(1) 
    if not os.path.isdir(target_directory):
        usage()
        print(f"ERROR: Not a directory: '{target_directory}'")
        exit(1) 


    # Build file list  
    if verbose:
        files = []
        for n, fn in enumerate(iglob(f"{target_directory}/**", recursive=recursive)):
            print(f"Building file list... {n}\r", end="")
            files.append(fn)
        print()
    elif not quiet:
        print("Building file list... (for large input, this may take some time. use `-v` flag for progress indicator.)")
        files = glob(f"{target_directory}/**", recursive=recursive)
        print(f"Found {len(files)} files.")
    else:
        files = glob(f"{target_directory}/**", recursive=recursive)


    
    # group files together by size using a map
    #           filesize, list of files
    seen_sizes: dict[int, list[str]] = dict() 
    # set of the filesizes for which there exist more than one file 
    dup_sizes = set()  

    for n, f in enumerate(files):  
        f = f.replace(os.sep, "/")
        ef = quote(f)
        if not quiet:
            print(f"size-checking {n}/{len(files)}\r", end="") 

        if os.path.isdir(f):
            continue
        
        fsize = os.path.getsize(f)
        if fsize not in seen_sizes:
            seen_sizes[fsize] = [f]
        else:
            seen_sizes[fsize].append(f)
            dup_sizes.add(fsize)
    if not quiet: 
        print(f" size-checked {len(files)}/{len(files)}")

    total_dup = 0
    if len(dup_sizes) > 200 and warn:
        print("WARNING: Lots of output. ", end="")
        if input("Continue? ") == 'n':
            exit()

    for dup_size in dup_sizes:
        checksums = dict()
        dup_checksums = set() # a set of keys in checksums which have more than one file
        if not quiet: 
            print(f"checking {len(seen_sizes[dup_size])} files with size {dup_size} for identical checksums...")
        for f in seen_sizes[dup_size]:
             checksum = fil_checksum(f)
             if checksum not in checksums:
                 checksums[checksum] = [f]
             else:
                 checksums[checksum].append(f)
                 dup_checksums.add(checksum)
        
        for dup_checksum in dup_checksums:
            duplicates = checksums[dup_checksum]
            total_dup += len(duplicates) 
            print(f"files with checksum {dup_checksum}:")
            for dup in checksums[dup_checksum]:
                print(f"  {quote(dup)}")
                 
    

    print(f"\nFound {total_dup} duplicate{'s' if total_dup != 1 else ''} out of {len(files)} files in {target_directory}")

if __name__ == "__main__":
    raise SystemExit(main())
