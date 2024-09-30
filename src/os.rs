/// Implements this interface for windows win32 platfrom
#[cfg(target_os = "windows")]
pub mod win32;

// Implements this interface for macos
#[cfg(target_os = "macos")]
pub mod macos;

use std::any::Any;
use serde::{Deserialize, Serialize};

type Error = super::Error;

/// Information to describe the Application properties
pub struct AppInfo {
    /// Name of the application
    pub name: String,
    /// Set to true to create a default window
    pub window: bool,
    /// Specify the number of buffers in the swap chain
    pub num_buffers: u32,
    /// Signify if this app wants to be dpi aware
    pub dpi_aware: bool,
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

/// Enums for system key presses
pub enum SysKey {
    Ctrl,
    Shift,
    Alt,
    Count
}

/// Enums for vitual keys
pub enum Key {
    Tab,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Insert,
    Delete,
    Backspace,
    Space,
    Enter,
    Escape,
    KeyPadEnter,
}

/// Enums for different mouse cursors
#[derive(Eq, PartialEq)]
pub enum Cursor {
    None,
    Arrow,
    TextInput,
    ResizeAll,
    ResizeEW,
    ResizeNS,
    ResizeNESW,
    ResizeNWSE,
    Hand,
    NotAllowed,
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
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq)]
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

/// 2-Dimension size for window coordinates
pub type Size<T> = Point<T>;

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
        /// Signify window is for imgui
        const IMGUI = 1<<6;
    }

    /// Event flags to query from other systems (such as imgui) to respond to native os events.
    pub struct WindowEventFlags : u32 {
        /// No flags
        const NONE = 0;
        /// Window was requested to be closed
        const CLOSE = 1<<0;
        /// Window was requested to move
        const MOVE = 1<<1;
        /// Window was requested to resize
        const SIZE = 1<<2;
    }

    /// Flags to control the open file dialog window
    pub struct OpenFileDialogFlags : u32 {
        /// Open dialog to look for files
        const FILES = 1<<0;
        /// Open dialog to look for folders
        const FOLDERS = 1<<1;
        /// Allow multiple selections
        const MULTI_SELECT = 1<<2;
    }
}

/// Filled out to specify various window parameters when a window is created by `App::create_window`
#[derive(Clone)]
pub struct WindowInfo<A: App> {
    /// Title appears in the title bar of the window
    pub title: String,
    /// Specify the position and size of the window
    pub rect: Rect<i32>,
    /// Specify window styles
    pub style: WindowStyleFlags,
    /// Specify a parent handle for child windows (optional)
    pub parent_handle: Option<A::NativeHandle>,
}

/// A native platform window handle that can be passed around in a lightweight way
pub trait NativeHandle<A: App> {
    /// returns the handle as an isize (ie. HWND)
    fn get_isize(&self) -> isize;
    /// returns a copy of the internal handle
    fn copy(&self) -> Self;
}

/// An interface which all platforms need to implement for general operating system calls
pub trait App: 'static + Any + Sized + Send + Sync {
    // A platform specific concrete window type
    type Window: Window<Self>;
    /// A platform specific handle to window (or other os handles) ie `HWND`
    type NativeHandle: NativeHandle<Self>;
    /// Create an application instance
    fn create(info: AppInfo) -> Self;
    /// Create a new operating system window
    fn create_window(&mut self, info: WindowInfo<Self>) -> Self::Window;
    /// Destroy window, unregistering app tracking
    fn destroy_window(&mut self, window: &Self::Window);
    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&mut self) -> bool;
    /// Request to exit the application
    fn exit(&mut self, exit_code: i32);
    /// Retuns the mouse in screen coordinates
    fn get_mouse_pos(&self) -> Point<i32>;
    /// Retuns the mouse vertical wheel position
    fn get_mouse_wheel(&self) -> f32;
    /// Retuns the mouse horizontal wheel positions
    fn get_mouse_hwheel(&self) -> f32;
    /// Retuns the mouse button states, up or down
    fn get_mouse_buttons(&self) -> [bool; MouseButton::Count as usize];
    /// Returns the distance the mouse has moved since the last frame
    fn get_mouse_pos_delta(&self) -> Size<i32>;
    /// Returns a vector of utf-16 characters that have been input since the last frame
    fn get_utf16_input(&self) -> Vec<u16>;
    /// Returns an array of bools containing 0-256 keys down (true) or up (false)
    fn get_keys_down(&self) -> [bool; 256];
    /// Returns an array of bools containing 0-256 of keys pressed, will trigger only once and then require debouce
    fn get_keys_pressed(&self) -> [bool; 256];
    /// Returns true if the sys key is down and false if the key is up
    fn is_sys_key_down(&self, key: SysKey) -> bool;
    /// Returns true if the sys key is pressed this frame and requires debounce until it is pressed again
    fn is_sys_key_pressed(&self, key: SysKey) -> bool;
    /// Get os system virtual key code from Key
    fn get_key_code(key: Key) -> i32;
    /// Set's whethere input from keybpard or mouse is available or not
    fn set_input_enabled(&mut self, keyboard: bool, mouse: bool);
    /// Get value for whether (keyboard, mouse) input is enabled
    fn get_input_enabled(&self) -> (bool, bool);
    /// Enumerate all display monitors
    fn enumerate_display_monitors() -> Vec<MonitorInfo>;
    // TODO: ^^
    fn enumerate_display_monitors2(&self) -> Vec<MonitorInfo>;
    /// Sets the mouse cursor
    fn set_cursor(&self, cursor: &Cursor);
    /// Opens a native open file dialog window, exts are provided to filer selections. ie vec![".txt", ".png"]
    fn open_file_dialog(flags: OpenFileDialogFlags, exts: Vec<&str>) -> Result<Vec<String>, Error>;
    /// Returns the wndow rectangle for the console window associated with the current app
    fn get_console_window_rect(&self) -> Rect<i32>;
    /// Sets the console window rect that belongs to this app
    fn set_console_window_rect(&self, rect: Rect<i32>);
}

/// An instance of an operating system window
pub trait Window<A: App>: 'static + Send + Sync + Any + Sized {
    /// Bring window to front and draw ontop of all others
    fn bring_to_front(&self);
    /// Show window, specify true to show window or false to hide
    fn show(&self, show: bool, activate: bool);
    /// Must be called each frame to handle resizes, parent App my have events to forward
    fn update(&mut self, app: &mut A);
    /// Close the window
    fn close(&mut self);
    /// Change the windows style
    fn update_style(&mut self, flags: WindowStyleFlags, rect: Rect<i32>);
    /// Returns true if the window is focused
    fn is_focused(&self) -> bool;
    /// Returns true if the window is minimised
    fn is_minimised(&self) -> bool;
    /// Sets focus to this window
    fn set_focused(&self);
    /// Returns true if the mouse if hovering this window
    fn is_mouse_hovered(&self) -> bool;
    /// Set the window display title that appears on the title bar
    fn set_title(&self, title: String);
    /// Set window position in screen space
    fn set_pos(&self, pos: Point<i32>);
    /// Set window size in screen coordinates
    fn set_size(&self, size: Size<i32>);
    /// Returns the screen position for the top-left corner of the window
    fn get_pos(&self) -> Point<i32>;
    /// Returns a gfx friendly full window rect to use as `gfx::Viewport` or `gfx::Scissor`
    fn get_viewport_rect(&self) -> Rect<i32>;
    /// Returns the screen position for the top-left corner of the window
    fn get_size(&self) -> Size<i32>;
    /// Returns the screen rect of the window screen pos x, y , size x, y.
    fn get_window_rect(&self) -> Rect<i32>;
    /// Return mouse position in relative coordinates from the top left corner of the window
    fn get_mouse_client_pos(&self, mouse_pos: Point<i32>) -> Point<i32>;
    /// Return the dpi scale for the current monitor the window is on
    fn get_dpi_scale(&self) -> f32;
    /// Gets the internal native handle
    fn get_native_handle(&self) -> A::NativeHandle;
    /// Gets window events tracked from os update, to handle events inside external systems
    fn get_events(&self) -> WindowEventFlags;
    /// Clears events after they have been responded to
    fn clear_events(&mut self);
    /// Const pointer
    fn as_ptr(&self) -> *const Self;
    /// Mut pointer
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

impl<A> Default for WindowInfo<A> where A: App {
    fn default() -> Self {
        Self {
            title: "hotline".to_string(),
            rect: Rect {
                x: 100,
                y: 100,
                width: 1280,
                height: 720,
            },
            style: WindowStyleFlags::NONE,
            parent_handle: None,
        }
    }
}
