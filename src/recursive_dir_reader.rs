use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct RecReadDir {
    dirs: Vec<PathBuf>,
    current: fs::ReadDir,
}

impl RecReadDir {
    pub fn new(start: &Path) -> io::Result<RecReadDir> {
        Ok(RecReadDir {
            dirs: vec![],
            current: start.read_dir()?,
        })
    }
}

impl Iterator for RecReadDir {
    type Item = io::Result<fs::DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        /*
            An std::fs::ReadDir iterates over the entries in a directory.
            In this iterator, a stack of directories (self.dirs) is maintained
            and items are yeilded from std::fs::ReadDir iterators over
            these directories in-turn until the stack is exhausted. When
            directories are found, they are added to the stack. This results in
            a recursive traversal.
        */
        if let Some(dir_entry) = self.current.next() {
            if let Ok(ref de) = dir_entry {
                if de.file_type().expect("couldn't get file type").is_dir() {
                    self.dirs.push(de.path());
                }
            }
            Some(dir_entry)
        } else {
            while let Some(path) = self.dirs.pop() {
                if let Ok(read_dir) = fs::read_dir(path) {
                    self.current = read_dir;
                    return self.next();
                }
            }
            None
        }
    }
}
