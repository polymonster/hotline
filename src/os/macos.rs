#![cfg(target_os = "macos")]

extern crate objc;

use std::time::Duration;

use winit::{
    event::{Event, WindowEvent}, event_loop::ControlFlow, platform::pump_events::{EventLoopExtPumpEvents, PumpStatus}, raw_window_handle::{HasWindowHandle, RawWindowHandle}
};

use cocoa::{appkit::NSView, base::id as cocoa_id};

pub struct App {
    event_loop: winit::event_loop::EventLoop<()>
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

pub struct Window {
    winit_window: winit::window::Window
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

#[derive(Clone, Copy)]
pub struct NativeHandle {

}

pub fn nsview_from_window(window: &Window) -> *mut objc::runtime::Object {
    if let Ok(RawWindowHandle::AppKit(rw)) = window.winit_window.window_handle().map(|wh| wh.as_raw()) {
        let view = rw.ns_view.as_ptr() as cocoa_id;
        view
    }
    else {
        std::ptr::null_mut()
    }
}

impl super::App for App {
    type Window = Window;
    type NativeHandle = NativeHandle;

    /// Create an application instance
    fn create(info: super::AppInfo) -> Self {
        App {
            event_loop: winit::event_loop::EventLoop::new().unwrap()
        }
    }

    /// Create a new operating system window
    fn create_window(&mut self, info: super::WindowInfo<Self>) -> Self::Window {
        let window = winit::window::WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(info.rect.width, info.rect.height))
            .with_position(winit::dpi::LogicalPosition::new(info.rect.x, info.rect.y))
            .with_title(info.title)
            .build(&self.event_loop)
            .unwrap();
        Window {
            winit_window: window
        }
    }

    /// Destroy window, unregistering app tracking
    fn destroy_window(&mut self, window: &Self::Window) {

    }

    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&mut self) -> bool {
        let mut resume = true;
        let status = self.event_loop.pump_events(Some(Duration::ZERO), |event, elwt| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        resume = false;
                    }
                    _ => {

                    }
                }
                _ => {

                }
            }
        });
        resume
    }

    /// Request to exit the application
    fn exit(&mut self, exit_code: i32) {

    }

    /// Retuns the mouse in screen coordinates
    fn get_mouse_pos(&self) -> super::Point<i32> {
        super::Point {
            x: 0,
            y: 0
        }
    }

    /// Retuns the mouse vertical wheel position
    fn get_mouse_wheel(&self) -> f32 {
        0.0
    }

    /// Retuns the mouse horizontal wheel positions
    fn get_mouse_hwheel(&self) -> f32 {
        0.0
    }

    /// Retuns the mouse button states, up or down
    fn get_mouse_buttons(&self) -> [bool; super::MouseButton::Count as usize] {
        [false; 5]
    }

    /// Returns the distance the mouse has moved since the last frame
    fn get_mouse_pos_delta(&self) -> super::Size<i32> {
        super::Size {
            x: 0,
            y: 0
        }
    }

    /// Returns a vector of utf-16 characters that have been input since the last frame
    fn get_utf16_input(&self) -> Vec<u16> {
        vec![]
    }

    /// Returns an array of bools containing 0-256 keys down (true) or up (false)
    fn get_keys_down(&self) -> [bool; 256] {
        [false; 256]
    }

    /// Returns an array of bools containing 0-256 of keys pressed, will trigger only once and then require debouce
    fn get_keys_pressed(&self) -> [bool; 256] {
        [false; 256]
    }

    /// Returns true if the sys key is down and false if the key is up
    fn is_sys_key_down(&self, key: super::SysKey) -> bool {
        false
    }

    /// Returns true if the sys key is pressed this frame and
    /// requires debounce until it is pressed again
    fn is_sys_key_pressed(&self, key: super::SysKey) -> bool {
        false
    }

    /// Get os system virtual key code from Key
    fn get_key_code(key: super::Key) -> i32 {
        0
    }

    /// Set's whethere input from keybpard or mouse is available or not
    fn set_input_enabled(&mut self, keyboard: bool, mouse: bool) {

    }

    /// Get value for whether (keyboard, mouse) input is enabled
    fn get_input_enabled(&self) -> (bool, bool) {
        (false, false)
    }

    /// Enumerate all display monitors
    fn enumerate_display_monitors() -> Vec<super::MonitorInfo> {
        vec![]
    }

    /// Sets the mouse cursor
    fn set_cursor(&self, cursor: &super::Cursor) {

    }

    /// Opens a native open file dialog window, exts are provided to filer selections. ie vec![".txt", ".png"]
    fn open_file_dialog(flags: super::OpenFileDialogFlags, exts: Vec<&str>) -> Result<Vec<String>, super::Error> {
        Ok(vec![])
    }

    /// Returns the wndow rectangle for the console window associated with the current app
    fn get_console_window_rect(&self) -> super::Rect<i32> {
        super::Rect {
            x: 0,
            y: 0,
            width: 0,
            height: 0
        }
    }

    /// Sets the console window rect that belongs to this app
    fn set_console_window_rect(&self, rect: super::Rect<i32>) {

    }
}

impl super::Window<App> for Window {
    /// Bring window to front and draw ontop of all others
    fn bring_to_front(&self) {

    }

    /// Show window, specify true to show window or false to hide
    fn show(&self, show: bool, activate: bool) {

    }

    /// Must be called each frame to handle resizes, parent App my have events to forward
    fn update(&mut self, app: &mut App) {

    }

    /// Close the window
    fn close(&mut self) {

    }

    /// Change the windows style
    fn update_style(&mut self, flags: super::WindowStyleFlags, rect: super::Rect<i32>) {

    }

    /// Returns true if the window is focused
    fn is_focused(&self) -> bool {
        false
    }

    /// Returns true if the window is minimised
    fn is_minimised(&self) -> bool {
        false
    }

    /// Sets focus to this window
    fn set_focused(&self) {

    }

    /// Returns true if the mouse if hovering this window
    fn is_mouse_hovered(&self) -> bool {
        false
    }

    /// Set the window display title that appears on the title bar
    fn set_title(&self, title: String) {

    }

    /// Set window position in screen space
    fn set_pos(&self, pos: super::Point<i32>) {

    }

    /// Set window size in screen coordinates
    fn set_size(&self, size: super::Size<i32>) {

    }

    /// Returns the screen position for the top-left corner of the window
    fn get_pos(&self) -> super::Point<i32> {
        let pos = self.winit_window.outer_position().unwrap();
        super::Point {
            x: pos.x,
            y: pos.y
        }
    }

    /// Returns a gfx friendly full window rect to use as `gfx::Viewport` or `gfx::Scissor`
    fn get_viewport_rect(&self) -> super::Rect<i32> {
        let size = self.winit_window.inner_size();
        super::Rect {
            x: 0,
            y: 0,
            width: size.width as i32,
            height: size.height as i32
        }
    }

    /// Returns the screen position for the top-left corner of the window
    fn get_size(&self) -> super::Size<i32> {
        let size = self.winit_window.inner_size();
        super::Size {
            x: size.width as i32,
            y: size.height as i32
        }
    }

    /// Returns the screen rect of the window screen pos x, y , size x, y.
    fn get_window_rect(&self) -> super::Rect<i32> {
        let pos = self.winit_window.outer_position().unwrap();
        let size = self.winit_window.inner_size();
        super::Rect {
            x: pos.x,
            y: pos.y,
            width: size.width as i32,
            height: size.height as i32
        }
    }

    /// Return mouse position in relative coordinates from the top left corner of the window
    fn get_mouse_client_pos(&self, mouse_pos: super::Point<i32>) -> super::Point<i32> {
        super::Point {
            x: 0,
            y: 0
        }
    }

    /// Return the dpi scale for the current monitor the window is on
    fn get_dpi_scale(&self) -> f32 {
        0.0
    }

    /// Gets the internal native handle
    fn get_native_handle(&self) -> NativeHandle {
        NativeHandle {

        }
    }

    /// Gets window events tracked from os update, to handle events inside external systems
    fn get_events(&self) -> super::WindowEventFlags {
        super::WindowEventFlags {
            bits: 0
        }
    }
    /// Clears events after they have been responded to
    fn clear_events(&mut self) {

    }

    /// Const pointer
    fn as_ptr(&self) -> *const Self {
        self.as_ptr()
    }

    /// Mut pointer
    fn as_mut_ptr(&mut self) -> *mut Self {
        self.as_mut_ptr()
    }
}

impl super::NativeHandle<App> for NativeHandle {
    fn get_isize(&self) -> isize {
        0
    }
    fn copy(&self) -> NativeHandle {
        *self
    }
}