use initramfs_builder::{Compression, InitramfsBuilder};
use std::io::Read;
use std::path::PathBuf;
use tokio::fs;

// Create a basic init script for testing
async fn create_test_init_script(dir: &std::path::Path) -> PathBuf {
    let init_path = dir.join("init.sh");
    let content = r#"#!/bin/sh
mount -t proc proc /proc
mount -t sysfs sysfs /sys
exec /bin/sh
"#;
    fs::write(&init_path, content).await.unwrap();
    init_path
}

// Create a dummy binary to inject
async fn create_test_binary(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"#!/bin/sh\necho hello\n").await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
    }
    path
}

// Parse CPIO newc format and extract entries
fn parse_cpio_entries(data: &[u8]) -> Vec<(String, u32, usize)> {
    let mut entries = Vec::new();
    let mut offset = 0;

    while offset + 110 <= data.len() {
        let header = &data[offset..offset + 110];
        let magic = std::str::from_utf8(&header[0..6]).unwrap_or("");
        if magic != "070701" {
            break;
        }

        let mode = u32::from_str_radix(std::str::from_utf8(&header[14..22]).unwrap_or("0"), 16)
            .unwrap_or(0);
        let filesize =
            usize::from_str_radix(std::str::from_utf8(&header[54..62]).unwrap_or("0"), 16)
                .unwrap_or(0);
        let namesize =
            usize::from_str_radix(std::str::from_utf8(&header[94..102]).unwrap_or("0"), 16)
                .unwrap_or(0);

        let name_start = offset + 110;
        if name_start + namesize > data.len() {
            break;
        }

        let name = std::str::from_utf8(&data[name_start..name_start + namesize - 1])
            .unwrap_or("")
            .to_string();

        if name == "TRAILER!!!" {
            break;
        }

        let header_plus_name = 110 + namesize;
        let name_padding = (4 - (header_plus_name % 4)) % 4;
        let data_start = name_start + namesize + name_padding;

        let data_padding = (4 - (filesize % 4)) % 4;
        offset = data_start + filesize + data_padding;

        entries.push((name, mode, filesize));
    }

    entries
}

fn decompress_gzip(data: &[u8]) -> Vec<u8> {
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).unwrap();
    out
}

// Test 1: CPIO content validation
#[tokio::test]
async fn test_build_produces_valid_cpio() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let output = tmp.path().join("output.cpio.gz");

    let result = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .build(&output)
        .await?;

    let metadata = std::fs::metadata(&output)?;
    assert!(metadata.len() > 0);
    assert!(result.entries > 0);

    let compressed = std::fs::read(&output)?;
    let raw_cpio = decompress_gzip(&compressed);
    let entries = parse_cpio_entries(&raw_cpio);

    assert!(!entries.is_empty());

    let paths: Vec<&str> = entries.iter().map(|(p, _, _)| p.as_str()).collect();
    assert!(paths.iter().any(|p| *p == "bin"
        || p.starts_with("bin/")
        || *p == "usr/bin"
        || p.starts_with("usr/bin/")));
    assert!(paths.iter().any(|p| *p == "etc" || p.starts_with("etc/")));

    println!("CPIO contains {} entries", entries.len());
    Ok(())
}

// Test 2: BuildResult metadata
#[tokio::test]
async fn test_build_result_metadata() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let output = tmp.path().join("output.cpio.gz");
    let init_script = create_test_init_script(tmp.path()).await;
    let inject_file = create_test_binary(tmp.path(), "my-tool").await;

    let result = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .inject(&inject_file, "/usr/bin/my-tool")
        .init_script(&init_script)
        .build(&output)
        .await?;

    assert!(result.entries > 0);
    assert!(result.uncompressed_size > 0);
    assert!(result.compressed_size > 0);
    assert!(result.compressed_size < result.uncompressed_size);
    assert_eq!(result.compression, Compression::Gzip);
    assert_eq!(result.injected_files, 1);
    assert!(result.has_custom_init);

    println!(
        "BuildResult: {} entries, {}B compressed, {}B uncompressed",
        result.entries, result.compressed_size, result.uncompressed_size
    );
    Ok(())
}

// Test 3: Init script injection
#[tokio::test]
async fn test_init_script_injection() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let output = tmp.path().join("output.cpio.gz");
    let init_script = create_test_init_script(tmp.path()).await;

    let result = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .init_script(&init_script)
        .build(&output)
        .await?;

    assert!(result.has_custom_init);

    let compressed = std::fs::read(&output)?;
    let raw_cpio = decompress_gzip(&compressed);
    let entries = parse_cpio_entries(&raw_cpio);

    let init_entry = entries.iter().find(|(path, _, _)| path == "init");
    assert!(init_entry.is_some());

    let (_, mode, size) = init_entry.unwrap();
    assert!(
        mode & 0o100 != 0,
        "init should be executable, got mode {:o}",
        mode
    );
    assert!(*size > 0);

    println!("init entry: mode={:o}, size={}", mode, size);
    Ok(())
}

// Test 4: File injection
#[tokio::test]
async fn test_file_injection() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let output = tmp.path().join("output.cpio.gz");
    let inject_file = create_test_binary(tmp.path(), "custom-tool").await;

    let result = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .inject(&inject_file, "/usr/bin/custom-tool")
        .build(&output)
        .await?;

    assert_eq!(result.injected_files, 1);

    let compressed = std::fs::read(&output)?;
    let raw_cpio = decompress_gzip(&compressed);
    let entries = parse_cpio_entries(&raw_cpio);

    let injected = entries
        .iter()
        .find(|(path, _, _)| path == "usr/bin/custom-tool");
    assert!(
        injected.is_some(),
        "CPIO should contain 'usr/bin/custom-tool'"
    );

    let (_, mode, size) = injected.unwrap();
    assert!(mode & 0o100 != 0);
    assert!(*size > 0);

    println!("Injected file: mode={:o}, size={}", mode, size);
    Ok(())
}

// Test 5: Compression modes
#[tokio::test]
async fn test_compression_modes() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let modes = vec![
        ("gzip", Compression::Gzip, "output.cpio.gz"),
        ("zstd", Compression::Zstd, "output.cpio.zst"),
        ("none", Compression::None, "output.cpio"),
    ];

    let mut sizes: Vec<(String, u64)> = Vec::new();

    for (label, compression, filename) in &modes {
        let output = tmp.path().join(filename);

        let result = InitramfsBuilder::new()
            .image("debian:stable-slim")
            .compression(*compression)
            .build(&output)
            .await?;

        let file_size = std::fs::metadata(&output)?.len();
        assert!(file_size > 0);
        assert_eq!(result.compression, *compression);

        sizes.push((label.to_string(), file_size));
        println!("{}: {} bytes", label, file_size);
    }

    let none_size = sizes.iter().find(|(l, _)| l == "none").unwrap().1;
    let gzip_size = sizes.iter().find(|(l, _)| l == "gzip").unwrap().1;
    assert!(none_size > gzip_size);

    Ok(())
}

// Test 6: Exclude patterns
#[tokio::test]
async fn test_exclude_patterns() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let output_full = tmp.path().join("full.cpio.gz");
    let result_full = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .build(&output_full)
        .await?;

    let output_excluded = tmp.path().join("excluded.cpio.gz");
    let result_excluded = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .exclude(&["usr/share/doc/*", "usr/share/man/*", "var/cache/*"])
        .build(&output_excluded)
        .await?;

    assert!(result_excluded.entries < result_full.entries);
    assert!(result_excluded.compressed_size < result_full.compressed_size);

    println!(
        "Full: {} entries, Excluded: {} entries",
        result_full.entries, result_excluded.entries
    );
    Ok(())
}

// Test 7: Reproducibility
#[tokio::test]
async fn test_reproducibility() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;

    let output1 = tmp.path().join("build1.cpio.gz");
    let result1 = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .build(&output1)
        .await?;

    let output2 = tmp.path().join("build2.cpio.gz");
    let result2 = InitramfsBuilder::new()
        .image("debian:stable-slim")
        .compression(Compression::Gzip)
        .build(&output2)
        .await?;

    assert_eq!(result1.entries, result2.entries);

    let cpio1 = decompress_gzip(&std::fs::read(&output1)?);
    let cpio2 = decompress_gzip(&std::fs::read(&output2)?);
    let entries1 = parse_cpio_entries(&cpio1);
    let entries2 = parse_cpio_entries(&cpio2);

    let paths1: Vec<&str> = entries1.iter().map(|(p, _, _)| p.as_str()).collect();
    let paths2: Vec<&str> = entries2.iter().map(|(p, _, _)| p.as_str()).collect();
    assert_eq!(paths1, paths2);

    println!(
        "Reproducibility: both builds have {} entries with identical paths",
        result1.entries
    );
    Ok(())
}
