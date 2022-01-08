# find-duplicates
A (aspiring to be fast) tool to find duplicate files.

# Quickstart
Currently, the working version of this tool is a python script. to use it:

```
find_duplicates.py [-r | -v | -q | -y |  -h] <directory>
```
for example:
```console
$ find_duplicates.py -r ~/some-dir-with-maybe-duplicate-files
```

# Help
```
USAGE: ./main [flags] <input>
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

    -y, --no-warn        disable large-output warnings.

    -h, --help           print this message.

  and where <input> is a path to a directory.
```
