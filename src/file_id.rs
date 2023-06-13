use std::fs;
use std::io;
use std::path::Path;

/* id from the OS; this must be an identifier that any two
files that are linked together (hardly or softly) will share;
inode on unix, nFileIndex{Low,High} on windows */

#[cfg(unix)]
pub fn get_file_identifier(fp: &Path) -> io::Result<u64> {
    /* on unix, we can use the inode number as a file identifier. */
    use std::os::unix::fs::MetadataExt;
    let md = fs::metadata(fp)?;
    Ok(md.ino())
}

#[cfg(windows)]
pub fn get_file_identifier(fp: &Path) -> io::Result<u64> {
    /* on windows, we can use the nFileIndex{Low,High} as a file identifier. */
    use std::os::windows::fs::MetadataExt;
    let md = fs::metadata(fp)?;
    // SAFETY: it is statically guaranteed that the call to `file_index` will be some.
    // From the `file_index` docs:
    // "This will return `None` if the `Metadata` instance was created from a call to
    // `DirEntry::metadata`. If this `Metadata` was created by using `fs::metadata` or
    // `File::metadata`, then this will return `Some`."
    Ok(unsafe { md.file_index().unwrap_unchecked() })
}
