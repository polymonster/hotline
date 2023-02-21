/// Operating system module (Windows, Application, Input).
pub mod os;

/// Graphics and compute abstraction module.
pub mod gfx;

/// Hardware accelerated audio and video decoding.
pub mod av;

/// Image reading/writing module support for (png, jpg, bmp, tiff, dds).
pub mod image;

/// Imgui rendering and platform implementation.
pub mod imgui;

/// Immediate mode primitive rendering API.
pub mod imdraw;

/// High level graphics (data driven render pipelines, shaders, views).
pub mod pmfx;

/// Primitive geometry meshes (quad, cube, sphere, etc).
pub mod primitives;

/// Hotline clinet context contains an `App`, `Device`, `SwapChain` and main `Window` automatically setup
/// It can load code dynamically from other `dylibs` or `dlls` abnd provides a very thin run loop for you to hook your own plugins into.
pub mod client;

/// Trait's and macros to assist the creation of plugins in other dynamically loaded libraries
pub mod plugin;

/// Module to aid data / code file watching, rebuilding and reloading
pub mod reloader;

/// Shared types and resources for use with bevy ecs
pub mod ecs_base;

/// Use bitmask for flags
#[macro_use]
extern crate bitflags;

/// Generic errors for modules to define their own
pub struct Error {
    pub msg: String,
}

/// Generic debug for errors
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

/// Conversion for windows-rs win32 errors
#[cfg(target_os = "windows")]
impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Error {
        Error {
            msg: err.message().to_string_lossy(),
        }
    }
}

/// std errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error {
            msg: err.to_string()
        }
    }
}

/// Returns the config name for the current configuration, this is useful to local items in target/debug
#[cfg(debug_assertions)]
pub const fn get_config_name() -> &'static str {
    "debug"
}

/// Returns the config name for the current configuration, this is useful to local items in target/release
#[cfg(not(debug_assertions))]
pub const fn get_config_name() -> &'static str {
    "release"
}

/// Return an absolute path for a resource given the relative resource name from the /hotline-data/src_data dir
pub fn get_src_data_path(asset: &str) -> String {
    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap().join("../../../hotline-data/src_data");
    String::from(asset_path.join(asset).to_str().unwrap())
}

/// Return an absolute path for a resource given the relative resource name from the /data dir
pub fn get_data_path(asset: &str) -> String {
    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap().join("..");
    String::from(asset_path.join(asset).to_str().unwrap())
}

/// Return an absolute path for a resource given the relative path from the /executable dir
pub fn get_exe_path(asset: &str) -> String {
    let exe_path = std::env::current_exe().ok().unwrap();
    println!("{}", String::from(exe_path.join(asset).to_str().unwrap()));
    String::from(exe_path.join(asset).to_str().unwrap())
}

/// Recursivley get files from folder as a vector
fn get_files_recursive(dir: &str, mut files: Vec<String>) -> Vec<String> {
    let paths = std::fs::read_dir(dir).unwrap();
    for path in paths {
        let path = path.unwrap().path();
        if std::fs::read_dir(&path).is_ok() {
            files = get_files_recursive(path.to_str().unwrap(), files);
        }
        else {
            files.push(path.to_str().unwrap().to_string());
        }   
    }
    files
}

/// This is a hardcoded compile time selection of os backend for windows as win32
#[cfg(target_os = "windows")]
pub use os::win32 as os_platform;

/// This is a hardcoded compile time selection of os backend for windows as d3d12
#[cfg(target_os = "windows")]
pub use gfx::d3d12 as gfx_platform;