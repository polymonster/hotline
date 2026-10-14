// A null / stubbed implementation
pub mod null;

/// Implements this interface for Windows Media Foundation with Direct3D12
#[cfg(target_os = "windows")]
pub mod wmf;

/// Implements this interface for AVFoundation with Metal
#[cfg(target_os = "macos")]
pub mod avf;

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

/// Encodes frames read back from the gpu into a video file with a platform specific hardware accelerated backend
pub trait VideoRecorder: 'static + Sized {
    /// Creates a video file at `filepath` (.mp4), odd `width` or `height` is rounded down to even for the encoder
    fn create(filepath: &str, width: u32, height: u32, fps: u32) -> Result<Self, Error>;
    /// Writes a BGRX8 frame of at least the recorder's size, `time` and `duration` are in seconds
    fn write_frame(&mut self, frame: &gfx::ReadBackData, time: f64, duration: f64) -> Result<(), Error>;
    /// Returns the encoded dimensions
    fn get_size(&self) -> os::Size<u32>;
    /// Flushes the encoder and closes the file, dropping a recorder without finishing also finishes it
    fn finish(self) -> Result<(), Error>;
}