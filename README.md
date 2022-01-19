# find-duplicates
A decenly fast tool to find duplicate files. Handles symbolic and hard links and treats them seperately to duplicates.


# Quickstart
Install rust and use cargo to compile the program.
```console
$ cargo build --release
```
*NOTE: it's important to use `--release` mode, because it will be noticably slower without optimizations.*

use the program to find some duplicates:
```console
$ ./target/release/find-duplicates -r ~/some-dir-with-maybe-duplicate-files
```

# Help
```
USAGE: find-duplicates [flags] <input>
  where [flags] can be 0 or more of the following:
    -r, --recursive      include files in subdirectories,
                         search recursively.

    -v, --verbose        enable progress bars and other
                         extra output. cannot be used with
                         -q, --quiet.

    -q, --quiet          disable all non-essential output,
                         good for redirecting to files or
                         piping to other programs. cannot
                         be used with -v, --verbose

    -h, --help           print this message.

  and where <input> is a path to a directory.
```
