use crate::client::*;
use crate::gfx;
use crate::os;
use crate::reloader;

use std::any::Any;
use std::process::ExitStatus;
use std::process::Command;
use std::sync::Arc;
use std::sync::Mutex;

pub type PluginReloadResponder = Arc<Mutex<Box<dyn reloader::ReloadResponder>>>;
pub type PluginInstance = *mut core::ffi::c_void;

#[derive(PartialEq, Eq)]
pub enum PluginState {
    None,
    Reload,
    Setup
}

pub struct PluginCollection {
    pub name: String,
    pub responder: PluginReloadResponder,
    pub reloader: reloader::Reloader,
    pub instance: PluginInstance,
    pub state: PluginState
}

/// Public trait for defining a plugin in a nother library
pub trait Plugin<D: gfx::Device, A: os::App> {
    /// Create a new instance of the plugin
    fn create() -> Self where Self: Sized;
    /// Called when the plugin is loaded and after a reload has happened, setup resources and state in here
    fn setup(&mut self, client: Client<D, A>) -> Client<D, A>;
    /// Called each and every frame, here put your update and render logic
    fn update(&mut self, client: Client<D, A>) -> Client<D, A>;
    /// Called when the plugin source has been modified and a reload is required, here handle any cleanup logic
    fn reload(&mut self, client: Client<D, A>) -> Client<D, A>;
    
    // Called when the plugin is to be unloaded, this will clean up
    // fn unload(&mut self, client: Client<D, A>) -> Client<D, A>;
}

/// Plugins are created on the heap and the instance is passed from the client to the plugin function calls
pub fn new_plugin<T : Plugin<crate::gfx_platform::Device, crate::os_platform::App> + Sized>() -> *mut T {
    unsafe {
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<T>(),
            8,
        )
        .unwrap();
        std::alloc::alloc_zeroed(layout) as *mut T
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
        // c-abi wrapper for `Plugin::create`
        #[no_mangle]
        pub fn create() -> *mut core::ffi::c_void {
            let ptr = new_plugin::<$input>() as *mut core::ffi::c_void;
            unsafe {
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                *plugin = $input::create();
            }
            ptr
        }
        
        // c-abi wrapper for `Plugin::update`
        #[no_mangle]
        pub fn update(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.update(client)
            }
        }
        
        // c-abi wrapper for `Plugin::setup`
        #[no_mangle]
        pub fn setup(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.setup(client)
            }
        }
        
        // c-abi wrapper for `Plugin::reload`
        #[no_mangle]
        pub fn reload(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.reload(client)
            }
        }
    }
}

/// General dll plugin responder, will check for source code changes and run cargo build to re-build the library
pub struct PluginLib {
    /// Name of the plugin
    pub name: String,
    /// Path to the plugins build director, where you would run `cargo build -p <name>`
    pub path: String,
    /// Full path to the build binary dylib or dll
    pub output_filepath: String,
    /// Array of source code files to track and check for changes
    pub files: Vec<String>
}

/// Reload responder implementation for `PluginLib` uses cargo build, and hot lib reloader
impl reloader::ReloadResponder for PluginLib {
    fn get_files(&self) -> &Vec<String> {
        &self.files
    }

    fn get_base_mtime(&self) -> std::time::SystemTime {
        let meta = std::fs::metadata(&self.output_filepath);
        if meta.is_ok() {
            std::fs::metadata(&self.output_filepath).unwrap().modified().unwrap()
        }
        else {
            std::time::SystemTime::now()
        }
    }

    fn build(&mut self) -> ExitStatus {
        let output = Command::new("cargo")
            .current_dir(format!("{}", self.path))
            .arg("build")
            .arg("-p")
            .arg(format!("{}", self.name))
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!");

        if output.stdout.len() > 0 {
            println!("{}", String::from_utf8(output.stdout).unwrap());
        }

        if output.stderr.len() > 0 {
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }

        output.status
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}