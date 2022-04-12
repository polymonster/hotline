use std::any::Any;
use crate::gfx;

/// Implements this interface for windows media foundation with Direct3D12
pub mod wmf;

/// Errors passed back from av backends
pub type Error = super::Error;

pub trait VideoPlayer<D: gfx::Device>: 'static + Sized + Any {
    fn create(device: &D) -> Result<Self, Error>;
    fn set_source(&self, file: String) -> Result<(), Error>;
    fn is_loaded(&self) -> bool;
    fn is_playing(&self) -> bool;
    fn is_ended(&self) -> bool;
    fn play(&self) -> Result<(), Error>;
    fn update(&mut self, device: &mut D) -> Result<(), Error>;
    fn get_texture(&self) -> &Option<D::Texture>;
}