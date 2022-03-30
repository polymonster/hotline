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
