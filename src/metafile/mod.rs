use std::cmp::Ordering;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
mod c_command;
mod file_id;
use file_id::get_file_identifier;

use indexmap::{indexset, IndexSet};

#[derive(Debug, Clone)]
pub struct MetaFile {
    id: u64,                     /* id from the OS; this must be an identifier that any two
                                 files that are linked together (hardly or symbolicaly) will share;
                                 inode on unix, nFileIndex{Low,High} on windows */
    files: IndexSet<PathBuf>, /* paths to files which share `id` as their identifier */
    symlinks: IndexSet<PathBuf>, /* paths to symlinks which share `id` as their identifier */
}

impl MetaFile {
    pub fn new(id: u64, files: IndexSet<PathBuf>, symlinks: IndexSet<PathBuf>) -> Self {
        Self {
            id,
            files,
            symlinks,
        }
    }

    pub fn from_id_and_path(id: u64, file: PathBuf) -> Self {
        let mut files = indexset![];
        let mut symlinks = indexset![];
        if file.is_symlink() {
            symlinks.insert(file);
        } else {
            files.insert(file);
        }
        Self::new(id, files, symlinks)
    }

    pub fn from_id(id: u64) -> Self {
        Self {
            id,
            files: indexset![],
            symlinks: indexset![],
        }
    }

    pub fn try_add_path(&mut self, p: PathBuf) -> Result<bool, ()> {
        if get_file_identifier(&p).is_ok_and(|id| id == self.id) {
            if p.is_symlink() {
                Ok(self.symlinks.insert(p))
            } else {
                Ok(self.files.insert(p))
            }
        } else {
            Err(())
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn paths(&self) -> IndexSet<&PathBuf> {
        self.files.union(&self.symlinks).collect()
    }

    pub fn c_commands(&self, other: &Self) -> bool {
        c_command::c_commands(self.paths()[0], other.paths()[0])
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

impl Ord for MetaFile {
    fn cmp(&self, other: &Self) -> Ordering {
        self.paths()[0].cmp(&other.paths()[0])
    }
}

impl PartialOrd for MetaFile {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for MetaFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.paths()[0].as_os_str().to_string_lossy())?;
        if self.paths().len() > 1 {
            write!(f, " (aka ")?;
        }
        for idx in 1..(self.paths().len() - 1) {
            let de = &self.paths()[idx];
            write!(f, "{:?}, ", de.as_os_str().to_string_lossy())?;
        }
        if self.paths().len() > 1 {
            write!(
                f,
                "{:?})",
                self.paths()[self.paths().len() - 1]
                    .as_os_str()
                    .to_string_lossy()
            )?;
        }
        Ok(())
    }
}

pub fn collect_into_metafiles(
    acc: &mut IndexSet<MetaFile>,
    paths: impl IntoIterator<Item = PathBuf>,
    keep_dirs: bool,
) {
    for p in paths {
        if !keep_dirs && fs::metadata(&p).map_or(false, |d| d.is_dir()) {
            continue;
        }
        let id = match get_file_identifier(&p) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Skipping error:\n {e}");
                continue;
            }
        };
        match acc.take(&MetaFile::from_id(id)) {
            Some(mut mf) => {
                assert!(mf.try_add_path(p).is_ok());
                assert!(acc.insert(mf));
            }
            None => {
                assert!(acc.insert(MetaFile::from_id_and_path(id, p)));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::fs;
    use std::io;
    use std::path::PathBuf;

    use indexmap::indexset;

    use super::collect_into_metafiles;

    #[test]
    fn metafiles_hard_link() -> io::Result<()> {
        /* setup */
        let file2 = PathBuf::from("test-tmp/file2");
        let file1 = PathBuf::from("test-tmp/file1");
        let link = PathBuf::from("test-tmp/file1-hardlink");
        fs::create_dir("test-tmp")?;
        fs::write(&file1, "meow")?;
        fs::write(&file2, "nya")?;
        fs::hard_link(&file1, &link)?;
        /* test */
        let mut metafiles = indexset![];
        collect_into_metafiles(
            &mut metafiles,
            [file1.clone(), file2.clone(), link.clone()],
            false,
        );
        dbg!(&metafiles);

        assert_eq!(metafiles.len(), 2);
        for file in &metafiles {
            assert!(file.paths() == indexset![&file2] || file.paths() == indexset![&file1, &link])
        }
        /* cleanup */
        fs::remove_dir_all("test-tmp")
    }

    #[ignore]
    #[test]
    fn metafiles_symlink() -> io::Result<()> {
        /* setup */
        let file2 = PathBuf::from("test-tmp/file2");
        let file1 = PathBuf::from("test-tmp/file1");
        let link = PathBuf::from("test-tmp/file1-symlink");
        fs::create_dir("test-tmp")?;
        fs::write(&file1, "meow")?;
        fs::write(&file2, "nya")?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&file1, &link)?
        }
        #[cfg(windows)]
        {
            dbg!(std::process::Command::new("powershell")
                .arg("-Command")
                .arg("New-Item")
                .arg("-ItemType")
                .arg("SymbolicLink")
                .arg("-Path")
                .arg("test-tmp\\file1-symlink")
                .arg("-Target")
                .arg("test-tmp\\file1")
                .output()?);
        }
        /* test */
        let mut metafiles = indexset![];
        collect_into_metafiles(
            &mut metafiles,
            [file1.clone(), file2.clone(), link.clone()],
            false,
        );
        dbg!(&metafiles);

        assert_eq!(metafiles.len(), 2);
        for file in &metafiles {
            assert!(file.paths() == indexset![&file2] || file.paths() == indexset![&file1, &link])
        }
        /* cleanup */
        fs::remove_dir_all("test-tmp")
    }
}
