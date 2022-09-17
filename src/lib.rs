/// Operating system module.
pub mod os;

/// Graphics and compute module.
pub mod gfx;

/// Hardware accelerated audio and video decoding
pub mod av;

/// Image reading/writing module support for (png, jpg, bmp, tiff, dds)
pub mod image;

/// Imgui rendering and platform implementation using imgui_sys
pub mod imgui;

/// Immediate mode primitive rendering API
pub mod imdraw;

/// Camera
pub mod camera;

/// High level graphics
pub mod pmfx;

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

// conversion for windows-rs win32 errors
#[cfg(target_os = "windows")]
impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Error {
        Error {
            msg: err.message().to_string_lossy(),
        }
    }
}

// std errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error {
            msg: err.to_string()
        }
    }
}