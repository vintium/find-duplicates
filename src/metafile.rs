use std::fmt;
use std::path::PathBuf;

use indexmap::IndexSet;

pub struct MetaFile {
    id: u64,                  /* file identifier */
    files: IndexSet<PathBuf>, /* files linked to the identifier */
}

impl MetaFile {
    pub fn new(id: u64, files: IndexSet<PathBuf>) -> Self {
        Self { id, files }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn files(&self) -> &IndexSet<PathBuf> {
        &self.files
    }
}

impl fmt::Display for MetaFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.files[0].as_os_str().to_string_lossy())?;
        if self.files.len() > 1 {
            write!(f, " (aka ")?;
        }
        for idx in 1..(self.files.len() - 1) {
            let de = &self.files[idx];
            write!(f, "{:?}, ", de.as_os_str().to_string_lossy())?;
        }
        if self.files.len() > 1 {
            write!(
                f,
                "{:?})",
                self.files[self.files.len() - 1]
                    .as_os_str()
                    .to_string_lossy()
            )?;
        }
        Ok(())
    }
}
