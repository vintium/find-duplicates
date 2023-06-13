use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;

use crate::file_id::get_file_identifier;

use indexmap::{indexset, IndexSet};

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

    pub fn from_id(id: u64) -> Self {
        Self {
            id,
            paths: indexset![],
        }
    }

    pub fn try_add_path(&mut self, p: PathBuf) -> Result<(), ()> {
        if get_file_identifier(&p) == self.id {
            self.paths.insert(p);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn paths(&self) -> &IndexSet<PathBuf> {
        &self.paths
    }
}

impl Hash for MetaFile {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for MetaFile {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for MetaFile {}

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

pub fn collect_into_metafiles<I>(acc: &mut IndexSet<MetaFile>, paths: I)
where
    I: Iterator<Item = PathBuf>,
{
    for p in paths {
        let id = get_file_identifier(&p);
        match acc.take(&MetaFile::from_id(id)) {
            Some(mut mf) => {
                assert!(mf.try_add_path(p).is_ok());
                assert!(acc.insert(mf));
            }
            None => {
                assert!(acc.insert(MetaFile::new(id, indexset![p])));
            }
        }
    }
}
