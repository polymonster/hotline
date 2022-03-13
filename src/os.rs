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

/// Used to index into array returned by app::get_mouse_buttons
pub enum MouseButton {
    Left,
    Middle,
    Right,
    X1,
    X2,
    Count,
}

/// Information to describe the dimensions of display monitors
#[derive(Clone)]
pub struct MonitorInfo {
    pub rect: Rect<i32>,
    pub client_rect: Rect<i32>,
    pub dpi_scale: f32,
    pub primary: bool,
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

/// 2-Dimension point for screen coordinates
#[derive(Copy, Clone)]
pub struct Point<T> {
    /// x position
    pub x: T,
    /// y position
    pub y: T,
}

bitflags! {
    /// Window style flags to change the window appearance
    pub struct WindowStyleFlags: u32 {
        /// No flags
        const NONE = 0;
        /// Visible
        const VISIBLE = 1<<0;
        /// Popup window
        const POPUP = 1<<1;
        /// Overlapped window has a title bar and border
        const OVERLAPPED_WINDOW = 1<<2;
        /// Has a smaller title bar
        const TOOL_WINDOW = 1<<3;
        /// Forces top level window onto the task bar when visible
        const APP_WINDOW = 1<<4;
        /// Placed above all non top most windows
        const TOPMOST = 1<<5;
    }
}

/// Filled out to specify various window parameters when a window is created by `App::create_window`
#[derive(Clone)]
pub struct WindowInfo {
    /// Title appears in the title bar of the window
    pub title: String,
    /// Specify the position and size of the window
    pub rect: Rect<i32>,
    /// Specify window styles
    pub style: WindowStyleFlags,
}

/// A native platform window handle that can be passed around in a lightweight way
pub trait NativeHandle<A: App> {}

/// An interface which all platforms need to implement for general operating system calls
pub trait App: 'static + Any + Sized {
    type Window: Window<Self>;
    type NativeHandle: NativeHandle<Self>;
    /// Create an application instance
    fn create(info: AppInfo) -> Self;
    /// Create a new operating system window
    fn create_window(&self, info: WindowInfo, parent: Option<Self::NativeHandle>) -> Self::Window;
    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&mut self) -> bool;
    /// Retuns the mouse in screen coordinates
    fn get_mouse_pos(&self) -> Point<i32>;
    /// Retuns the mouse vertical wheel position
    fn get_mouse_wheel(&self) -> f32;
    /// Retuns the mouse horizontal wheel positions
    fn get_mouse_hwheel(&self) -> f32;
    /// Retuns the mouse button states, up or down
    fn get_mouse_buttons(&self) -> [bool; MouseButton::Count as usize];
    /// Enumerate all display monitors
    fn enumerate_display_monitors() -> Vec<MonitorInfo>;
}

/// An instance of an operating system window
pub trait Window<A: App>: Any + Sized {
    /// Bring window to front and draw ontop of all others
    fn bring_to_front(&self);
    /// Show window, specify true to show window or false to hide
    fn show(&self, show: bool, activate: bool);
    /// Returns true if the window is focused
    fn is_focused(&self) -> bool;
    /// Sets focus to this window
    fn set_focused(&self);
    /// Returns true if the mouse if hovering this window
    fn is_mouse_hovered(&self) -> bool;
    /// Returns the screen position for the top-left corner of the window
    fn get_screen_pos(&self) -> Point<i32>;
    /// Set the window display title that appears on the title bar
    fn set_title(&self, title: String);
    /// Set window position in screen space
    fn set_pos(&self, pos: Point<i32>);
    /// Set the window position and size in 1
    fn set_rect(&mut self, rect: Rect<i32>);
    /// Returns the internal window rect
    fn get_rect(&self) -> Rect<i32>;
    /// Returns a gfx friendly full window rect to use as `gfx::Viewport` or `gfx::Scissor`
    fn get_viewport_rect(&self) -> Rect<i32>;
    /// Return mouse position in relative coordinates from the top left corner of the window
    fn get_mouse_client_pos(&self, mouse_pos: &Point<i32>) -> Point<i32>;
    /// Set only the size of the window
    fn set_size(&mut self, width: i32, height: i32);
    /// Returns the size of the window as tuple
    fn get_size(&self) -> (i32, i32);
    /// Must be called each frame to handle resizes
    fn update(&mut self);
    /// gets the internal native handle
    fn get_native_handle(&self) -> A::NativeHandle;
    /// Close the window
    fn close(&mut self);
    /// const pointer
    fn as_ptr(&self) -> *const Self;
    /// mut pointer
    fn as_mut_ptr(&mut self) -> *mut Self;
}

impl Default for Point<f32> {
    fn default() -> Self {
        Point::<f32> { x: 0.0, y: 0.0 }
    }
}

impl Default for Point<i32> {
    fn default() -> Self {
        Point::<i32> { x: 0, y: 0 }
    }
}

impl Default for Point<u32> {
    fn default() -> Self {
        Point::<u32> { x: 0, y: 0 }
    }
}
