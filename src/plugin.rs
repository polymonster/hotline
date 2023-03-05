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
    pub files: Vec<String>
}

/// Public trait for defining a plugin in a another library implement this trait and instantiate it with `hotline_plugin!`
pub trait Plugin<D: gfx::Device, A: os::App> {
    /// Create a new instance of the plugin
    fn create() -> Self where Self: Sized;
    /// Called when the plugin is loaded and after a reload has happened, setup resources and state in here
    fn setup(&mut self, client: Client<D, A>) -> Client<D, A>;
    /// Called each and every frame, here put your update and render logic
    fn update(&mut self, client: Client<D, A>) -> Client<D, A>;
    // Called where it is safe to make imgui calls
    fn ui(&mut self, client: Client<D, A>) -> Client<D, A>;
    // Called when the plugin is to be unloaded, this will clean up
    fn unload(&mut self, client: Client<D, A>) -> Client<D, A>;
}

/// Utility function to build all plugins, this can be used to bootstrap them if they don't exist
pub fn build_all() {
    let path = super::get_data_path("..");
    let output = if super::get_config_name() == "release" {
        Command::new("cargo")
            .current_dir(format!("{}", path))
            .arg("build")
            .arg(format!("{}", "--release"))
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!")
    }
    else {
        Command::new("cargo")
            .current_dir(format!("{}", path))
            .arg("build")
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!")
    };

    if output.stdout.len() > 0 {
        println!("{}", String::from_utf8(output.stdout).unwrap());
    }

    if output.stderr.len() > 0 {
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
        let src_path = self.path.to_string() + "/" + &self.name.to_string() + "/src";
        let src_files = super::get_files_recursive(&src_path, Vec::new());
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
                .current_dir(format!("{}", self.path))
                .arg("build")
                .arg(format!("{}", "--release"))
                .arg("-p")
                .arg(format!("{}", self.name))
                .output()
                .expect("hotline::hot_lib:: hot lib failed to build!")
        }
        else {
            Command::new("cargo")
                .current_dir(format!("{}", self.path))
                .arg("build")
                .arg("-p")
                .arg(format!("{}", self.name))
                .output()
                .expect("hotline::hot_lib:: hot lib failed to build!")
        };

        let mut stdout = io::stdout().lock();

        if output.stdout.len() > 0 {
            stdout.write_all(&output.stdout).unwrap();
            //println!("{}", String::from_utf8(output.stdout).unwrap());
        }

        if output.stderr.len() > 0 {
            stdout.write_all(&output.stderr).unwrap();
            //println!("{}", String::from_utf8(output.stderr).unwrap());
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
        pub fn create() -> *mut core::ffi::c_void {
            let plugin = $input::create();
            let ptr = Box::into_raw(Box::new(plugin));
            ptr.cast()
        }
        
        // c-abi wrapper for `Plugin::update`
        #[no_mangle]
        pub fn update(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                plugin.update(client)
            }
        }
        
        // c-abi wrapper for `Plugin::setup`
        #[no_mangle]
        pub fn setup(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                plugin.setup(client)
            }
        }
        
        // c-abi wrapper for `Plugin::reload`
        #[no_mangle]
        pub fn unload(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                plugin.unload(client)
            }
        }

        // c-abi wrapper for `Plugin::reload`
        #[no_mangle]
        pub fn ui(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void, imgui_ctx: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = ptr.cast::<$input>();
                let plugin = plugin.as_mut().unwrap();
                client.imgui.set_current_context(imgui_ctx);
                plugin.ui(client)
            }
        }
    }
}

/// Plugin instances are crated by the `Plugin::create` function, created on the heap
/// and passed around as a void* through the hotline_plugin macro to become a `Plugin` trait
pub type PluginInstance = *mut core::ffi::c_void;