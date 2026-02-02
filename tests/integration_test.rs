use initramfs_builder::{InitramfsBuilder, Compression};
use std::process::{Command, Stdio};
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tokio::fs;

const KERNEL_URL: &str = "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.10/x86_64/vmlinux-6.1.102";
const KERNEL_PATH: &str = "tests/fixtures/vmlinux-6.1.102";

async fn ensure_kernel() -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(KERNEL_PATH);
    if path.exists() {
        return Ok(path);
    }

    println!("Downloading kernel from {}...", KERNEL_URL);
    let response = reqwest::get(KERNEL_URL).await?;
    let bytes = response.bytes().await?;
    
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    
    fs::write(&path, bytes).await?;
    println!("Kernel downloaded to {:?}", path);
    Ok(path)
}

fn compile_agent() -> anyhow::Result<PathBuf> {
    let src = Path::new("tests/agent/main.rs");
    let out_dir = Path::new("tests/fixtures");
    if !out_dir.exists() {
        std::fs::create_dir_all(out_dir)?;
    }
    let out = out_dir.join("agent");

    println!("Compiling agent...");
    let status = Command::new("rustc")
        .arg(src)
        .arg("-o")
        .arg(&out)
        .arg("--target")
        .arg("x86_64-unknown-linux-musl")
        .args(&["-C", "target-feature=+crt-static"])
        .status()?;

    if !status.success() {
         println!("Static compilation failed, trying dynamic...");
         let status2 = Command::new("rustc")
            .arg(src)
            .arg("-o")
            .arg(&out)
            .status()?;
            
         if !status2.success() {
             anyhow::bail!("Failed to compile agent");
         }
    }

    Ok(out)
}

#[tokio::test]
async fn test_end_to_end_boot() -> anyhow::Result<()> {
    let kernel_path = ensure_kernel().await?;
    let agent_path = compile_agent()?;
    let output_cpio = PathBuf::from("tests/fixtures/test.cpio.gz");
    
    let init_script_content = r#"#!/bin/sh
mount -t proc proc /proc
mount -t sysfs sysfs /sys
echo "Mounting devtmpfs..."
mount -t devtmpfs devtmpfs /dev
echo "Starting agent..."
/usr/bin/agent
echo "Agent finished."
sleep 5
poweroff -f
"#;
    let init_script_path = PathBuf::from("tests/fixtures/init.sh");
    fs::write(&init_script_path, init_script_content).await?;

    println!("Building initramfs...");
    InitramfsBuilder::new()
        .image("debian:stable-slim")
        .inject(agent_path, "/usr/bin/agent")
        .init_script(init_script_path)
        .build(&output_cpio)
        .await?;

    println!("Booting QEMU...");
    let mut child = Command::new("qemu-system-x86_64")
        .arg("-kernel")
        .arg(kernel_path)
        .arg("-initrd")
        .arg(output_cpio)
        .arg("-append")
        .arg("console=ttyS0 rdinit=/init panic=1")
        .arg("-nographic")
        .arg("-no-reboot")
        .arg("-m")
        .arg("256")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("Failed to open stdout");
    let reader = BufReader::new(stdout);

    let mut success = false;
    let timeout = Duration::from_secs(30);
    let start = std::time::Instant::now();

    let handle = std::thread::spawn(move || {
        for line in reader.lines() {
            if let Ok(l) = line {
                println!("[QEMU] {}", l);
                if l.contains("Agent verification successful!") {
                    return true;
                }
            }
        }
        false
    });

    while start.elapsed() < timeout {
        if handle.is_finished() {
            success = handle.join().unwrap_or(false);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let _ = child.kill();
    
    if success {
        println!("Test PASSED!");
        Ok(())
    } else {
        anyhow::bail!("Test FAILED: Agent success message not found");
    }
}
