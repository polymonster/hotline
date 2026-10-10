use crate::client::*;
use crate::gfx;
use crate::os;
use crate::reloader;

use std::process::ExitStatus;
use std::process::Command;
use std::io::{self, Write};

/// General dll plugin responder, will check for source code changes and run cargo build to re-build the library
pub struct PluginReloadResponder {
    /// Name of the plugin
    pub name: String,
    /// Path to the plugins build director, where you would run `cargo build -p <name>`
    pub path: String,
    /// Full path to the build binary dylib or dll
    pub output_filepath: String,
    /// Array of source code files to track and check for changes
    pub files: Vec<String>,
    /// Source directory of the plugin, which is scanned for new files to track
    pub src_path: String
}

/// Public trait for defining a plugin in a another library implement this trait and instantiate it with `hotline_plugin!`
pub trait Plugin<D: gfx::Device, A: os::App> {
    /// Create a new instance of the plugin
    fn create() -> Self where Self: Sized;
    /// Called when the plugin is loaded and after a reload has happened, setup resources and state in here
    fn setup(&mut self, client: &mut Client<D, A>);
    /// Called each and every frame, here put your update and render logic
    fn update(&mut self, client: Client<D, A>) -> Client<D, A>;
    // Called where it is safe to make imgui calls
    fn ui(&mut self, client: &mut Client<D, A>);
    // Called when the plugin is to be unloaded, this will clean up
    fn unload(&mut self, client: &mut Client<D, A>);
}

/// Info about a cargo package needed to build and load it as a plugin
pub struct CargoPackageInfo {
    /// Root of the cargo workspace the package is built in, where to run `cargo build -p <name>`
    pub workspace_root: std::path::PathBuf,
    /// Target directory the workspace builds into
    pub target_dir: std::path::PathBuf,
    /// True if the package builds a `dylib` or `cdylib` which can be loaded as a plugin
    pub is_dylib: bool
}

/// Returns info about the package `name` in the cargo workspace which builds the package at `manifest`
pub fn get_cargo_package_info(manifest: &std::path::Path, name: &str) -> Option<CargoPackageInfo> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--no-deps")
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .ok()?;
    if !output.status.success() {
        println!("{}", String::from_utf8_lossy(&output.stderr));
        return None;
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let workspace_root = metadata.get("workspace_root")?.as_str()?;
    let target_dir = metadata.get("target_directory")?.as_str()?;

    // find the lib target of the package to check if it is dynamic
    let package = metadata.get("packages")?.as_array()?.iter()
        .find(|p| p.get("name").and_then(|n| n.as_str()) == Some(name))?;
    let is_dylib = package.get("targets")?.as_array()?.iter().any(|t| {
        t.get("crate_types").and_then(|c| c.as_array()).map_or(false, |types| {
            types.iter().any(|c| c.as_str() == Some("cdylib") || c.as_str() == Some("dylib"))
        })
    });

    Some(CargoPackageInfo {
        workspace_root: std::path::PathBuf::from(workspace_root),
        target_dir: std::path::PathBuf::from(target_dir),
        is_dylib
    })
}

/// Utility function to build all plugins, this can be used to bootstrap them if they don't exist
pub fn build_all() {
    let path = super::get_data_path("..");
    let output = if super::get_config_name() == "release" {
        Command::new("cargo")
            .current_dir(path)
            .arg("build")
            .arg("--release")
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!")
    }
    else {
        Command::new("cargo")
            .current_dir(path)
            .arg("build")
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!")
    };

    if !output.stdout.is_empty() {
        println!("{}", String::from_utf8(output.stdout).unwrap());
    }

    if !output.stderr.is_empty() {
        println!("{}", String::from_utf8(output.stderr).unwrap());
    }
}

/// Reload responder implementation for `PluginLib` uses cargo build, and hot lib reloader
impl reloader::ReloadResponder for PluginReloadResponder {
    fn add_file(&mut self, path: &str) {
        self.files.push(path.to_string());
    }

    fn get_files(&self) -> Vec<String> {
        // scan for new files so we can dd them and pickup changes
        // TODO; this could be more easily be configured in a plugin meta data file
        let src_files = super::get_files_recursive(&self.src_path, Vec::new());
        let mut result = self.files.to_vec();
        result.extend(src_files);
        result
    }

    fn get_last_mtime(&self) -> std::time::SystemTime {
        let meta = std::fs::metadata(&self.output_filepath);
        if meta.is_ok() {
            std::fs::metadata(&self.output_filepath).unwrap().modified().unwrap()
        }
        else {
            std::time::SystemTime::now()
        }
    }

    fn build(&mut self) -> ExitStatus {
        let output = if super::get_config_name() == "release" {
            Command::new("cargo")
                .current_dir(&self.path)
                .arg("build")
                .arg("--release")
                .arg("-p")
                .arg(&self.name)
                .output()
                .expect("hotline::hot_lib:: hot lib failed to build!")
        }
        else {
            Command::new("cargo")
                .current_dir(&self.path)
                .arg("build")
                .env("CARGO_TERM_COLOR", "always")
                .arg("-p")
                .arg(&self.name)
                .output()
                .expect("hotline::hot_lib:: hot lib failed to build!")
        };

        let mut stdout = io::stdout().lock();

        if !output.stdout.is_empty() {
            stdout.write_all(&output.stdout).unwrap();
        }

        if !output.stderr.is_empty() {
            stdout.write_all(&output.stderr).unwrap();
        }

        output.status
    }
}

/// Macro to instantiate a new hotline plugin, simply defined a concrete plugin type:
/// struct EmptyPlugin;
///
/// You can implement the `Plugin` trait for `EmptyPlugin`
/// impl Plugin<gfx_platform::Device, os_platform::App> for EmptyPlugin {
/// ..
/// }
///
/// Then use this macro to make the plugin loadable from a dll
/// hotline_plugin![EmptyPlugin];
#[macro_export]
macro_rules! hotline_plugin {
    ($input:ident) => {
        /// Plugins are created on the heap and the instance is passed from the client to the plugin function calls
        // c-abi wrapper for `Plugin::create`
        #[no_mangle]
        pub extern "C" fn create() -> *mut core::ffi::c_void {
            let plugin = $input::create();
            let ptr = Box::into_raw(Box::new(plugin));
            ptr.cast()
        }

        // c-abi wrapper for `Plugin::update`
        // each lib has its own imgui global context, so it is set from the client's on every call. after a reload the
        // new lib can be updated before `ui` has been called on it
        #[no_mangle]
        pub extern "C" fn update(client_ptr: *mut client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void, imgui_ctx: *mut core::ffi::c_void) {
            unsafe {
                (*client_ptr).imgui.set_current_context(imgui_ctx);
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                // take client on the stack to pass ownership (bevy ecs needs to move resources in/out of World)
                let client: client::Client<gfx_platform::Device, os_platform::App> = std::ptr::read(client_ptr);
                let client = plugin.update(client);
                std::ptr::write(client_ptr, client);
            }
        }

        // c-abi wrapper for `Plugin::setup`
        #[no_mangle]
        pub extern "C" fn setup(client: *mut client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void, imgui_ctx: *mut core::ffi::c_void) {
            unsafe {
                (*client).imgui.set_current_context(imgui_ctx);
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                plugin.setup(&mut (*client));
            }
        }

        // c-abi wrapper for `Plugin::unload`
        #[no_mangle]
        pub extern "C" fn unload(client: *mut client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void, imgui_ctx: *mut core::ffi::c_void) {
            unsafe {
                (*client).imgui.set_current_context(imgui_ctx);
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                plugin.unload(&mut (*client));
            }
        }

        // c-abi wrapper for `Plugin::ui`
        #[no_mangle]
        pub extern "C" fn ui(client: *mut client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void, imgui_ctx: *mut core::ffi::c_void) {
            unsafe {
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                (*client).imgui.set_current_context(imgui_ctx);
                plugin.ui(&mut (*client));
            }
        }
    }
}

/// Plugin instances are crated by the `Plugin::create` function, created on the heap
/// and passed around as a void* through the hotline_plugin macro to become a `Plugin` trait
pub type PluginInstance = *mut core::ffi::c_void;