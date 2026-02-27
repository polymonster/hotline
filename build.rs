use htwv;

use std::process::Command;

#[cfg(target_os = "windows")]
fn main() {
    println!("cargo:rerun-if-changed=shaders");

    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        let pmbuild = "hotline-data\\pmbuild.cmd";

        let status = Command::new("cmd")
            .args(["/C", pmbuild, "win32-data"])
            .status()
            .unwrap_or_else(|e| panic!("failed to run '{pmbuild}': {e}"));

        if !status.success() {
            panic!("pmbuild win32-data failed with status: {status}");
        }
    }
}

#[cfg(target_os = "macos")]
fn main() {
    use std::path::Path;

    // Rerun when source shaders change
    println!("cargo:rerun-if-changed=shaders");
    // Rerun when output dir changes (including deletion)
    println!("cargo:rerun-if-changed=target/data/shaders");

    if std::env::var("CARGO_FEATURE_BUILD_DATA").is_ok() {
        let output_dir = Path::new("target/data/shaders");

        // Check if we actually need to rebuild
        let needs_build = !output_dir.exists()
            || std::fs::read_dir(output_dir)
                .map(|mut d| d.next().is_none())
                .unwrap_or(true);

        let pmbuild = "pmbuild";
        let status = Command::new(pmbuild)
            .args(["mac-data"])
            .status()
            .unwrap_or_else(|e| panic!("failed to run '{pmbuild}': {e}"));

        if !status.success() {
            panic!("pmbuild mac-data failed with status: {status}");
        }

        if needs_build {
            println!("cargo:warning=Compiling shaders...");
            match htwv::compile_dir("shaders", "target/data/shaders") {
                Ok(_) => println!("cargo:warning=Shader compilation succeeded"),
                Err(e) => {} //panic!("Shader compilation failed: {e}"),
            }
        }
    }
}