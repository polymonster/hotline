use std::process::Command;

#[cfg(target_os = "windows")]
fn build_data() {
    let output = Command::new("bin/win32/pmbuild.exe")
    .arg("win32")
    .output()
    .expect("pmbuild failed");
    println!("{}", String::from_utf8(output.stdout).unwrap());
    println!("{}", String::from_utf8(output.stderr).unwrap());
}

fn main() {
    build_data()
}