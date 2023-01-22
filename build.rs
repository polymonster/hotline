use std::process::Command;

fn main() {
    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        let output = Command::new("bin/win32/pmbuild.exe")
        .arg("win32")
        .output()
        .expect("pmbuild failed");

        println!("{}", String::from_utf8(output.stdout).unwrap());
        println!("{}", String::from_utf8(output.stderr).unwrap());
    }
}