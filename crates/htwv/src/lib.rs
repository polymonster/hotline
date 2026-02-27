//! HTWV - HLSL To Vulkan to Metal shader compilation toolchain
//!
//! This crate compiles HLSL shaders through SPIR-V to Metal Shading Language (MSL).
//! It is only functional on macOS - on other platforms it provides stub functions
//! that return errors.

#[cfg(target_os = "macos")]
#[allow(warnings)]
mod spirv_cross_bindings;

#[cfg(target_os = "macos")]
mod macos_impl;

#[cfg(target_os = "macos")]
pub use macos_impl::*;

// Stub implementations for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn compile_dir(
    _input_dir: &str,
    _output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("htwv shader compilation is only available on macOS".into())
}

#[cfg(not(target_os = "macos"))]
pub fn compile_piepline(
    _filepath: &str,
    _input_dir: &str,
    _output_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    Err("htwv shader compilation is only available on macOS".into())
}
