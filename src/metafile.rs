use std::fmt;
use std::path::PathBuf;

use indexmap::IndexSet;

pub struct MetaFile {
    id: u64,                  /* id from the OS; this must be an identifier that any two
                              files that are linked together (hardly or softly) will share;
                              inode on unix, nFileIndex{Low,High} on windows */
    paths: IndexSet<PathBuf>, /* paths to files which share `id` as their identifier */
}

impl MetaFile {
    pub fn new(id: u64, paths: IndexSet<PathBuf>) -> Self {
        Self { id, paths }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn paths(&self) -> &IndexSet<PathBuf> {
        &self.paths
    }
}

impl fmt::Display for MetaFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.paths[0].as_os_str().to_string_lossy())?;
        if self.paths.len() > 1 {
            write!(f, " (aka ")?;
        }
        for idx in 1..(self.paths.len() - 1) {
            let de = &self.paths[idx];
            write!(f, "{:?}, ", de.as_os_str().to_string_lossy())?;
        }
        if self.paths.len() > 1 {
            write!(
                f,
                "{:?})",
                self.paths[self.paths.len() - 1]
                    .as_os_str()
                    .to_string_lossy()
            )?;
        }
        Ok(())
    }
}
