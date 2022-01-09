/// Implements this interface for windows win32 platfrom
pub mod win32;

use std::any::Any;

/// Contains types which operating system backends
/// need to implement concrete types for
pub trait Platform: 'static + Sized + Any {
    type Instance: Instance<Self>;
    type Window: Window<Self>;
}

/// An interface which all platforms need to implement
/// for general operating system calls
pub trait Instance<P: Platform>: 'static + Any + Sized {
    fn create() -> Self;
    fn create_window(&self, info: WindowInfo) -> P::Window;
    fn run(&self) -> bool;
}

/// Describes a rectangle starting at the top left corner specified by x,y
/// with the size of width and height.
#[derive(Copy, Clone)]
pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

/// Filled out to specify various window parameters
/// when a window is created by `Instance::create_window`
pub struct WindowInfo {
    pub title: String,
    pub rect: Rect<i32>,
}

/// An instance of an operating system window
pub trait Window<P: Platform>: Any + Sized {
    fn bring_to_front(&self);
    fn set_rect(&mut self, rect: Rect<i32>);
    fn get_rect(&self) -> Rect<i32>;
    fn set_size(&mut self, width: i32, height: i32);
    fn get_size(&self) -> (i32, i32);
    fn update(&mut self);
    fn close(&mut self);
}
