// A null / stubbed implementation
pub mod null;

/// Implements this interface for Windows Media Foundation with Direct3D12
#[cfg(target_os = "windows")]
pub mod wmf;

use std::any::Any;
use crate::gfx;
use crate::os;

/// Errors passed back from av backends
pub type Error = super::Error;

/// An opaque video player with platform specific hardware accelerated backend implementations 
pub trait VideoPlayer<D: gfx::Device>: 'static + Sized + Any {
    /// Create a new instance of video player
    fn create(device: &D) -> Result<Self, Error>;
    /// Sets source media, filepath to a video file supported by the hardware
    fn set_source(&mut self, filepath: String) -> Result<(), Error>;
    /// Call this each frame, needs to be updated after set source is called
    fn update(&mut self, device: &mut D) -> Result<(), Error>;
    /// Plays the current video, check if is_loaded first
    fn play(&self) -> Result<(), Error>;
    /// Pause current video if curently playing
    fn pause(&self) -> Result<(), Error>;
    /// Return texture with current frame for use in rendering, will return None if not loaded or not ready
    fn get_texture(&self) -> &Option<D::Texture>;
    /// Check is video has finished after a set_source call has been made
    fn is_loaded(&self) -> bool;
    /// Check if video is currently playing
    fn is_playing(&self) -> bool;
    /// Check if video has played and ended
    fn is_ended(&self) -> bool;
    /// Return the dimensions of the video
    fn get_size(&self) -> os::Size<u32>;
}