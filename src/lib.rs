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

/// Use bitmask for flags
#[macro_use]
extern crate bitflags;

/// Generic errors for modules to define their own
pub struct Error<E> {
    pub error_type: E,
    pub msg: String,
}

/// Generic debug for errors
impl<E: std::fmt::Debug> std::fmt::Debug for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{:?} Error: \n{}\n", self.error_type, self.msg)
    }
}