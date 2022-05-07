use std::process::Command;

fn main() {
    Command::new("bin/win32/pmbuild.exe")
        .arg("win32")
        .output()
        .expect("pmbuild failed");
}