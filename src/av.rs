use std::any::Any;
use crate::gfx;

/// Implements this interface for windows media foundation with Direct3D
pub mod winmf;

pub trait VideoPlayer<D: gfx::Device>: 'static + Sized + Any {
    fn create(device: &D) -> Self;
    fn set_source(&self, file: String);
}