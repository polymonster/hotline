use std::process::Command;

fn get_hotline_data() {
    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        if !std::path::Path::new("hotline-data").exists()  {
            println!("hotline_rs::build: cloning data respository");
            let output = Command::new("git")
                .arg("clone")
                .arg("https://github.com/polymonster/hotline-data.git")
                .output()
                .expect("hotline_rs::build: git clone hotline-data failed");
            println!("{}", String::from_utf8(output.stdout).unwrap());
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }
        else {
            println!("hotline_rs::build: updating data respository");
            let output = Command::new("git")
                .current_dir("hotline-data")
                .arg("pull")
                .output()
                .expect("hotline_rs::build: git pull hotline-data failed");
            println!("{}", String::from_utf8(output.stdout).unwrap());
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }
    }
}

fn main() {
    get_hotline_data();
}