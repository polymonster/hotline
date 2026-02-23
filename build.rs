use std::process::Command;

fn main() {
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
