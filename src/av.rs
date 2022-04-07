use std::any::Any;
use crate::gfx;


/// Implements this interface for windows media foundation with Direct3D
pub mod winmf;

/// Error types for different gfx backends and FFI calls
#[derive(Debug)]
pub enum ErrorType {
    WindowsMediaFoundation,
    InitFailed,
    InvalidSource,
}

/// Errors passed back from av backends
pub type Error = super::Error<ErrorType>;

pub trait VideoPlayer<D: gfx::Device>: 'static + Sized + Any {
    fn create(device: &D) -> Result<Self, Error>;
    fn set_source(&self, file: String) -> Result<(), Error>;
    fn play(&self)-> Result<(), Error>;
    fn transfer_frame(&self, texture: &D::Texture) -> Result<(), Error>;
}