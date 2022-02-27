/// Implements this interface for windows win32 platfrom
pub mod win32;

use std::any::Any;

/// Information to describe the Application properties
pub struct AppInfo {
    /// Name of the application
    pub name: String,
    /// Set to true to create a default window
    pub window: bool,
    /// Specify the number of buffers in the swap chain
    pub num_buffers: u32,
}

/// An interface which all platforms need to implement for general operating system calls
pub trait App: 'static + Any + Sized {
    type Window: Window<Self>;
    /// Create an application instance
    fn create(info: AppInfo) -> Self;
    /// Create a new operating system window
    fn create_window(&self, info: WindowInfo) -> Self::Window;
    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&self) -> bool;
}

/// Describes a rectangle starting at the top left corner specified by x,y with the size of width and height
#[derive(Copy, Clone)]
pub struct Rect<T> {
    /// Top left position x
    pub x: T,
    /// Top left position y
    pub y: T,
    /// Width of the window starting at x
    pub width: T,
    /// Height of the window starting at y
    pub height: T,
}

/// Filled out to specify various window parameters when a window is created by `App::create_window`
#[derive(Clone)]
pub struct WindowInfo {
    /// Title appears in the title bar of the window
    pub title: String,
    /// Specify the position and size of the window
    pub rect: Rect<i32>,
}

/// An instance of an operating system window
pub trait Window<A: App>: Any + Sized {
    fn bring_to_front(&self);
    /// Set the window position and size in 1
    fn set_rect(&mut self, rect: Rect<i32>);
    /// Returns the window position and size inside rect
    fn get_rect(&self) -> Rect<i32>;
    /// Returns a gfx friendly full window rect to use as `gfx::Viewport` or `gfx::Scissor`
    fn get_viewport_rect(&self) -> Rect<i32>;
    /// Set only the size of the window
    fn set_size(&mut self, width: i32, height: i32);
    /// Returns the size of the window as tuple
    fn get_size(&self) -> (i32, i32);
    /// Must be called each frame to handle resizes
    fn update(&mut self);
    /// Close the window
    fn close(&mut self);
    /// const pointer
    fn as_ptr(&self) -> *const Self;
    /// mut pointer
    fn as_mut_ptr(&mut self) -> *mut Self;
}
