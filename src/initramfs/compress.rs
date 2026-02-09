use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression as GzCompression;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    #[default]
    Gzip,
    Zstd,
    None,
}

impl std::str::FromStr for Compression {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gzip" | "gz" => Ok(Compression::Gzip),
            "zstd" | "zst" => Ok(Compression::Zstd),
            "none" | "raw" => Ok(Compression::None),
            _ => Err(format!("Unknown compression: {}", s)),
        }
    }
}

impl std::fmt::Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Compression::Gzip => write!(f, "gzip"),
            Compression::Zstd => write!(f, "zstd"),
            Compression::None => write!(f, "none"),
        }
    }
}

/// Compress data and write to output path
pub fn compress_archive(data: &[u8], output_path: &Path, compression: Compression) -> Result<u64> {
    info!(
        "Compressing {} bytes with {} to {:?}",
        data.len(),
        compression,
        output_path
    );

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create output file: {:?}", output_path))?;
    let mut writer = BufWriter::new(file);

    match compression {
        Compression::Gzip => {
            let mut encoder = GzEncoder::new(&mut writer, GzCompression::default());
            encoder.write_all(data)?;
            encoder.finish()?;
        }
        Compression::Zstd => {
            let mut encoder = zstd::stream::Encoder::new(&mut writer, 3)?;
            encoder.write_all(data)?;
            encoder.finish()?;
        }
        Compression::None => {
            writer.write_all(data)?;
        }
    }

    writer.flush()?;

    let output_size = std::fs::metadata(output_path)?.len();
    info!(
        "Compressed {} bytes -> {} bytes ({:.1}% ratio)",
        data.len(),
        output_size,
        (output_size as f64 / data.len() as f64) * 100.0
    );

    Ok(output_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Read;
    use tempfile::TempDir;

    #[test]
    fn test_compression_from_str() {
        assert_eq!("gzip".parse::<Compression>().unwrap(), Compression::Gzip);
        assert_eq!("gz".parse::<Compression>().unwrap(), Compression::Gzip);
        assert_eq!("zstd".parse::<Compression>().unwrap(), Compression::Zstd);
        assert_eq!("zst".parse::<Compression>().unwrap(), Compression::Zstd);
        assert_eq!("none".parse::<Compression>().unwrap(), Compression::None);
        assert_eq!("raw".parse::<Compression>().unwrap(), Compression::None);
        assert!("invalid".parse::<Compression>().is_err());
    }

    #[test]
    fn test_compression_display() {
        assert_eq!(format!("{}", Compression::Gzip), "gzip");
        assert_eq!(format!("{}", Compression::Zstd), "zstd");
        assert_eq!(format!("{}", Compression::None), "none");
    }

    #[test]
    fn test_compression_default() {
        assert_eq!(Compression::default(), Compression::Gzip);
    }

    #[test]
    fn test_gzip_compression() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test.gz");
        // Use repetitive data that compresses well
        let data: Vec<u8> = b"hello world ".repeat(100).to_vec();

        let size = compress_archive(&data, &output_path, Compression::Gzip).unwrap();

        assert!(output_path.exists());
        assert!(size > 0);

        // Verify it's valid gzip and decompresses correctly
        let file = File::open(&output_path).unwrap();
        let mut decoder = flate2::read::GzDecoder::new(file);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_zstd_compression() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test.zst");
        let data = b"hello world hello world hello world";

        let size = compress_archive(data, &output_path, Compression::Zstd).unwrap();

        assert!(output_path.exists());
        assert!(
            size < data.len() as u64,
            "Compressed size should be smaller"
        );

        // Verify it's valid zstd
        let compressed = fs::read(&output_path).unwrap();
        let decompressed = zstd::decode_all(&compressed[..]).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn test_no_compression() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test.cpio");
        let data = b"hello world";

        let size = compress_archive(data, &output_path, Compression::None).unwrap();

        assert_eq!(size, data.len() as u64);
        assert_eq!(fs::read(&output_path).unwrap(), data);
    }
}
