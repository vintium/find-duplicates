use std::fs;
use std::path::Path;

/* id from the OS; this must be an identifier that any two
files that are linked together (hardly or softly) will share;
inode on unix, nFileIndex{Low,High} on windows */

#[cfg(unix)]
pub fn get_file_identifier(fp: &Path) -> u64 {
    /* on unix, we can use the inode number as a file identifier. */
    use std::os::unix::fs::MetadataExt;
    /* NOTE: this function expects the path passed in to
    have been pre-verified to exist. */
    let md = fs::metadata(fp).unwrap();
    md.ino()
}

#[cfg(windows)]
pub fn get_file_identifier(fp: &Path) -> u64 {
    /* on windows, we can use the nFileIndex{Low,High} as a file identifier. */
    use std::os::windows::fs::MetadataExt;
    /* NOTE: this function expects the path passed in to
    have been pre-verified to exist. */
    let md = fs::metadata(fp).unwrap();
    md.file_index().unwrap()
}
