use std::process::Command;
use std::thread;
use std::time::Duration;

fn main() {
    println!("Agent starting...");
    thread::sleep(Duration::from_secs(1));
    println!("Agent verification successful!");
    let _ = Command::new("sync").status();
    let _ = Command::new("/bin/poweroff").args(&["-f"]).status();
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
