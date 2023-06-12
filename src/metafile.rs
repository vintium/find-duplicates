use std::fmt;
use std::fs;

pub struct MetaFile {
    id: u64,                  /* file identifier */
    files: Vec<fs::DirEntry>, /* files linked to the identifier */
}

impl MetaFile {
    pub fn new(id: u64, files: Vec<fs::DirEntry>) -> Self {
        Self { id, files }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn files(&self) -> &[fs::DirEntry] {
        &self.files
    }
}

impl fmt::Display for MetaFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}",
            self.files[0].path().as_os_str().to_string_lossy()
        )?;
        if self.files.len() > 1 {
            write!(f, " (aka ")?;
        }
        for idx in 1..(self.files.len() - 1) {
            let de = &self.files[idx];
            write!(f, "{:?}, ", de.path().as_os_str().to_string_lossy())?;
        }
        if self.files.len() > 1 {
            write!(
                f,
                "{:?})",
                self.files[self.files.len() - 1]
                    .path()
                    .as_os_str()
                    .to_string_lossy()
            )?;
        }
        Ok(())
    }
}
