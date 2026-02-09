//! # initramfs-builder
//!
//! Convert Docker/OCI images to bootable initramfs for microVMs.
//!
//! ## Example
//!
//! ```no_run
//! use initramfs_builder::{InitramfsBuilder, Compression};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     InitramfsBuilder::new()
//!         .image("python:3.11-alpine")
//!         .compression(Compression::Gzip)
//!         .exclude(&["/usr/share/doc/*", "/var/cache/*"])
//!         .inject("./cloude-agentd", "/usr/bin/cloude-agentd")
//!         .init_script("./init.sh")
//!         .build("python.cpio.gz")
//!         .await?;
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod image;
pub mod initramfs;
pub mod registry;

pub use error::{BuilderError, Result};
pub use initramfs::{compress_archive, Compression};
pub use registry::{PullOptions, RegistryAuth, RegistryClient};

use anyhow::Context;
use image::RootfsBuilder;
use initramfs::CpioArchive;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone)]
pub struct InjectFile {
    pub src: PathBuf,
    pub dest: PathBuf,
    pub executable: bool,
}

impl InjectFile {
    pub fn new(src: impl Into<PathBuf>, dest: impl Into<PathBuf>) -> Self {
        Self {
            src: src.into(),
            dest: dest.into(),
            executable: false,
        }
    }

    pub fn executable(mut self) -> Self {
        self.executable = true;
        self
    }
}

pub struct InitramfsBuilder {
    image: Option<String>,
    compression: Compression,
    exclude_patterns: Vec<String>,
    platform_os: String,
    platform_arch: String,
    auth: RegistryAuth,
    inject_files: Vec<InjectFile>,
    init_script: Option<PathBuf>,
}

impl InitramfsBuilder {
    pub fn new() -> Self {
        Self {
            image: None,
            compression: Compression::default(),
            exclude_patterns: Vec::new(),
            platform_os: "linux".to_string(),
            platform_arch: "amd64".to_string(),
            auth: RegistryAuth::default(),
            inject_files: Vec::new(),
            init_script: None,
        }
    }

    pub fn image(mut self, image: &str) -> Self {
        self.image = Some(image.to_string());
        self
    }

    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    pub fn exclude(mut self, patterns: &[&str]) -> Self {
        self.exclude_patterns
            .extend(patterns.iter().map(|s| s.to_string()));
        self
    }

    pub fn platform(mut self, os: &str, arch: &str) -> Self {
        self.platform_os = os.to_string();
        self.platform_arch = arch.to_string();
        self
    }

    /// Set authentication credentials
    pub fn auth(mut self, auth: RegistryAuth) -> Self {
        self.auth = auth;
        self
    }

    /// Inject a file into the initramfs
    ///
    /// # Arguments
    /// * `src` - Source path on host filesystem
    /// * `dest` - Destination path inside initramfs (e.g., "/usr/bin/myagent")
    pub fn inject(mut self, src: impl Into<PathBuf>, dest: impl Into<PathBuf>) -> Self {
        self.inject_files
            .push(InjectFile::new(src, dest).executable());
        self
    }

    /// Inject a file with custom options
    pub fn inject_file(mut self, file: InjectFile) -> Self {
        self.inject_files.push(file);
        self
    }

    /// Set a custom init script that will be placed at /init
    /// This script runs as PID 1 when the kernel boots
    pub fn init_script(mut self, path: impl Into<PathBuf>) -> Self {
        self.init_script = Some(path.into());
        self
    }

    /// Build the initramfs and write it to the output path
    pub async fn build<P: AsRef<Path>>(self, output: P) -> anyhow::Result<BuildResult> {
        let image = self.image.as_ref().context("No image specified")?;

        info!("Building initramfs from {}", image);

        let client = RegistryClient::new(self.auth);
        let exclude_refs: Vec<&str> = self.exclude_patterns.iter().map(|s| s.as_str()).collect();

        let mut rootfs_builder = RootfsBuilder::new(client)
            .platform(&self.platform_os, &self.platform_arch)
            .exclude(&exclude_refs);

        let rootfs_path = rootfs_builder.build(image).await?;

        for inject in &self.inject_files {
            let dest_path = if inject.dest.is_absolute() {
                rootfs_path.join(inject.dest.strip_prefix("/").unwrap_or(&inject.dest))
            } else {
                rootfs_path.join(&inject.dest)
            };

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            info!("Injecting {:?} -> {:?}", inject.src, inject.dest);
            fs::copy(&inject.src, &dest_path)
                .with_context(|| format!("Failed to inject {:?}", inject.src))?;

            if inject.executable {
                let mut perms = fs::metadata(&dest_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest_path, perms)?;
            }
        }

        let init_dest = rootfs_path.join("init");
        if let Some(init_src) = &self.init_script {
            info!("Setting init script from {:?}", init_src);
            fs::copy(init_src, &init_dest)
                .with_context(|| format!("Failed to copy init script from {:?}", init_src))?;
        } else {
            info!("Generating default init script");
            let default_init = r#"#!/bin/sh
mount -t proc proc /proc 2>/dev/null
mount -t sysfs sysfs /sys 2>/dev/null
mount -t devtmpfs devtmpfs /dev 2>/dev/null

for cmd in /docker-entrypoint.sh /entrypoint.sh /usr/bin/entrypoint.sh; do
    [ -x "$cmd" ] && exec "$cmd"
done

exec /bin/sh
"#;
            fs::write(&init_dest, default_init)?;
        }

        let mut perms = fs::metadata(&init_dest)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&init_dest, perms)?;

        info!("Creating CPIO archive from {:?}", rootfs_path);

        let archive = CpioArchive::from_directory(&rootfs_path)?;

        let mut cpio_data = Vec::new();
        archive.write_to(&mut cpio_data)?;

        info!(
            "CPIO archive: {} entries, {} bytes uncompressed",
            archive.len(),
            cpio_data.len()
        );

        let output_size = compress_archive(&cpio_data, output.as_ref(), self.compression)?;

        Ok(BuildResult {
            entries: archive.len(),
            uncompressed_size: cpio_data.len() as u64,
            compressed_size: output_size,
            compression: self.compression,
            injected_files: self.inject_files.len(),
            has_custom_init: self.init_script.is_some(),
        })
    }
}

impl Default for InitramfsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct BuildResult {
    pub entries: usize,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub compression: Compression,
    pub injected_files: usize,
    pub has_custom_init: bool,
}
