use anyhow::{Context, Result};
use std::fs::{self};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tracing::debug;
use walkdir::WalkDir;

pub struct CpioArchive {
    entries: Vec<CpioEntry>,
}

struct CpioEntry {
    path: String,
    mode: u32,
    uid: u32,
    gid: u32,
    nlink: u32,
    mtime: u32,
    data: Vec<u8>,
    dev_major: u32,
    dev_minor: u32,
    rdev_major: u32,
    rdev_minor: u32,
}

impl CpioArchive {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Build a CPIO archive from a directory
    pub fn from_directory(root: &Path) -> Result<Self> {
        let mut archive = Self::new();

        for entry in WalkDir::new(root).follow_links(false) {
            let entry = entry?;
            let full_path = entry.path();

            let rel_path = full_path.strip_prefix(root).unwrap_or(full_path);

            if rel_path.as_os_str().is_empty() {
                continue;
            }

            let archive_path = format!("{}", rel_path.display());

            archive.add_path(full_path, &archive_path)?;
        }

        Ok(archive)
    }

    /// Add a file or directory to the archive
    fn add_path(&mut self, source_path: &Path, archive_path: &str) -> Result<()> {
        let metadata = fs::symlink_metadata(source_path)
            .with_context(|| format!("Failed to read metadata for {:?}", source_path))?;

        let file_type = metadata.file_type();
        let mode = metadata.permissions().mode();

        let data = if file_type.is_file() {
            fs::read(source_path)?
        } else if file_type.is_symlink() {
            let target = fs::read_link(source_path)?;
            target.to_string_lossy().as_bytes().to_vec()
        } else {
            Vec::new()
        };

        debug!(
            "Adding to cpio: {} (mode: {:o}, size: {})",
            archive_path,
            mode,
            data.len()
        );

        self.entries.push(CpioEntry {
            path: archive_path.to_string(),
            mode,
            uid: metadata.uid(),
            gid: metadata.gid(),
            nlink: metadata.nlink() as u32,
            mtime: metadata.mtime() as u32,
            data,
            dev_major: 0,
            dev_minor: 0,
            rdev_major: 0,
            rdev_minor: 0,
        });

        Ok(())
    }

    /// Write the archive to a file
    pub fn write_to<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut ino = 1u32;

        for entry in &self.entries {
            self.write_entry(writer, entry, ino)?;
            ino += 1;
        }

        // Write trailer
        self.write_trailer(writer)?;

        Ok(())
    }

    /// Write a single entry in newc format
    fn write_entry<W: Write>(&self, writer: &mut W, entry: &CpioEntry, ino: u32) -> Result<()> {
        let namesize = entry.path.len() + 1; // +1 for null terminator
        let filesize = entry.data.len();

        // newc header format (110 bytes of ASCII hex)
        let header = format!(
            "{}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
            "070701",         // magic
            ino,              // inode
            entry.mode,       // mode
            entry.uid,        // uid
            entry.gid,        // gid
            entry.nlink,      // nlink
            entry.mtime,      // mtime
            filesize,         // filesize
            entry.dev_major,  // dev major
            entry.dev_minor,  // dev minor
            entry.rdev_major, // rdev major
            entry.rdev_minor, // rdev minor
            namesize,         // namesize
            0u32,             // checksum (always 0 for newc)
        );

        writer.write_all(header.as_bytes())?;
        writer.write_all(entry.path.as_bytes())?;
        writer.write_all(&[0])?; // null terminator

        // Pad to 4-byte boundary after header+name
        let header_plus_name = 110 + namesize;
        let padding = (4 - (header_plus_name % 4)) % 4;
        writer.write_all(&vec![0u8; padding])?;

        writer.write_all(&entry.data)?;

        // Pad data to 4-byte boundary
        let data_padding = (4 - (filesize % 4)) % 4;
        writer.write_all(&vec![0u8; data_padding])?;

        Ok(())
    }

    /// Write the TRAILER!!! entry
    fn write_trailer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let trailer_name = "TRAILER!!!";
        let namesize = trailer_name.len() + 1;

        let header = format!(
            "{}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
            "070701", 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, namesize, 0
        );

        writer.write_all(header.as_bytes())?;
        writer.write_all(trailer_name.as_bytes())?;
        writer.write_all(&[0])?;

        // Pad to 4-byte boundary
        let header_plus_name = 110 + namesize;
        let padding = (4 - (header_plus_name % 4)) % 4;
        writer.write_all(&vec![0u8; padding])?;

        Ok(())
    }

    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for CpioArchive {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_empty_archive() {
        let archive = CpioArchive::new();
        assert!(archive.is_empty());
        assert_eq!(archive.len(), 0);
    }

    #[test]
    fn test_archive_from_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let archive = CpioArchive::from_directory(temp_dir.path()).unwrap();
        assert_eq!(archive.len(), 1);
    }

    #[test]
    fn test_cpio_header_magic() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test").unwrap();

        let archive = CpioArchive::from_directory(temp_dir.path()).unwrap();
        let mut output = Vec::new();
        archive.write_to(&mut output).unwrap();

        let header = String::from_utf8_lossy(&output[..6]);
        assert_eq!(header, "070701", "CPIO header should start with newc magic");
    }

    #[test]
    fn test_cpio_trailer() {
        let archive = CpioArchive::new();
        let mut output = Vec::new();
        archive.write_to(&mut output).unwrap();

        let output_str = String::from_utf8_lossy(&output);
        assert!(
            output_str.contains("TRAILER!!!"),
            "Archive should end with TRAILER!!!"
        );
    }

    #[test]
    fn test_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("a.txt"), b"aaa").unwrap();
        fs::write(temp_dir.path().join("b.txt"), b"bbb").unwrap();
        fs::create_dir(temp_dir.path().join("subdir")).unwrap();
        fs::write(temp_dir.path().join("subdir/c.txt"), b"ccc").unwrap();

        let archive = CpioArchive::from_directory(temp_dir.path()).unwrap();
        assert_eq!(archive.len(), 4); // 3 files + 1 directory
    }

    #[test]
    fn test_symlink_handling() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        fs::write(&target, b"target content").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let archive = CpioArchive::from_directory(temp_dir.path()).unwrap();

        #[cfg(unix)]
        assert_eq!(archive.len(), 2);
    }

    #[test]
    fn test_output_alignment() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("odd.txt"), b"123").unwrap(); // 3 bytes, needs padding

        let archive = CpioArchive::from_directory(temp_dir.path()).unwrap();
        let mut output = Vec::new();
        archive.write_to(&mut output).unwrap();

        // Output should be 4-byte aligned
        assert_eq!(output.len() % 4, 0, "CPIO output should be 4-byte aligned");
    }
}
