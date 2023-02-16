use std::process::Command;

fn get_hotline_data() {
    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        if !std::path::Path::new("hotline-data").exists()  {
            println!("hotline_rs::build: cloning data respository");
            let output = Command::new("git")
            .arg("clone")
            .arg("https://github.com/polymonster/hotline-data.git")
            .output()
            .expect("hotline_rs::build: git clone failed");
            println!("{}", String::from_utf8(output.stdout).unwrap());
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }
    }
}

#[cfg(target_os = "windows")]
fn build_data() {
    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        let output = Command::new("hotline-data/pmbuild.cmd")
        .arg("win32")
        .output()
        .expect("pmbuild failed");
        println!("{}", String::from_utf8(output.stdout).unwrap());
        println!("{}", String::from_utf8(output.stderr).unwrap());
    }
}

#[cfg(not(target_os = "windows"))]
fn build_data() {
}

fn main() {
    get_hotline_data();
    println!("hotline_rs::build: building data");
    build_data()
}