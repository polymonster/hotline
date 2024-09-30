#![cfg(target_os = "macos")]

extern crate objc;

use core::panic;
use std::time::Duration;

use winit::{
    dpi::{LogicalPosition, LogicalSize}, event::{Event, WindowEvent}, event_loop::{self, ControlFlow}, platform::pump_events::{EventLoopExtPumpEvents, PumpStatus}, raw_window_handle::{HasWindowHandle, RawWindowHandle}
};

use cocoa::{appkit::{NSEvent, NSLeftArrowFunctionKey, NSNextFunctionKey, NSView}, base::id as cocoa_id };

use crate::os::Rect;

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
    handle: isize
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

pub fn isize_id_from_window(window: &Window) -> isize {
    if let Ok(RawWindowHandle::AppKit(rw)) = window.winit_window.window_handle().map(|wh| wh.as_raw()) {
        let view = rw.ns_view.as_ptr() as isize;
        view
    }
    else {
        0
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
        panic!();
    }

    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&mut self) -> bool {
        objc::rc::autoreleasepool(|| {
            let mut resume = true;
            let status = self.event_loop.pump_events(Some(Duration::ZERO), |event, elwt| {
                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            resume = false;
                        }
                        WindowEvent::RedrawRequested => {
                        }
                        _ => {

                        }
                    }
                    _ => {

                    }
                }
            });
            resume
        })
    }

    /// Request to exit the application
    fn exit(&mut self, exit_code: i32) {
        panic!();
    }

    /// Retuns the mouse in screen coordinates
    fn get_mouse_pos(&self) -> super::Point<i32> {
        panic!();
        super::Point {
            x: 0,
            y: 0
        }
    }

    /// Retuns the mouse vertical wheel position
    fn get_mouse_wheel(&self) -> f32 {
        panic!();
        0.0
    }

    /// Retuns the mouse horizontal wheel positions
    fn get_mouse_hwheel(&self) -> f32 {
        panic!();
        0.0
    }

    /// Retuns the mouse button states, up or down
    fn get_mouse_buttons(&self) -> [bool; super::MouseButton::Count as usize] {
        panic!();
        [false; 5]
    }

    /// Returns the distance the mouse has moved since the last frame
    fn get_mouse_pos_delta(&self) -> super::Size<i32> {
        panic!();
        super::Size {
            x: 0,
            y: 0
        }
    }

    /// Returns a vector of utf-16 characters that have been input since the last frame
    fn get_utf16_input(&self) -> Vec<u16> {
        panic!();
        vec![]
    }

    /// Returns an array of bools containing 0-256 keys down (true) or up (false)
    fn get_keys_down(&self) -> [bool; 256] {
        panic!();
        [false; 256]
    }

    /// Returns an array of bools containing 0-256 of keys pressed, will trigger only once and then require debouce
    fn get_keys_pressed(&self) -> [bool; 256] {
        panic!();
        [false; 256]
    }

    /// Returns true if the sys key is down and false if the key is up
    fn is_sys_key_down(&self, key: super::SysKey) -> bool {
        panic!();
        false
    }

    /// Returns true if the sys key is pressed this frame and
    /// requires debounce until it is pressed again
    fn is_sys_key_pressed(&self, key: super::SysKey) -> bool {
        panic!();
        false
    }

    /// Get os system virtual key code from Key
    fn get_key_code(key: super::Key) -> i32 {
        match key {
            super::Key::Tab => winit::keyboard::KeyCode::Tab as i32,
            super::Key::Left => winit::keyboard::KeyCode::ArrowLeft as i32,
            super::Key::Right => winit::keyboard::KeyCode::ArrowRight as i32,
            super::Key::Up => winit::keyboard::KeyCode::ArrowUp as i32,
            super::Key::Down => winit::keyboard::KeyCode::ArrowDown as i32,
            super::Key::PageUp => winit::keyboard::KeyCode::PageUp as i32,
            super::Key::PageDown => winit::keyboard::KeyCode::PageDown as i32,
            super::Key::Home => winit::keyboard::KeyCode::Home as i32,
            super::Key::End => winit::keyboard::KeyCode::End as i32,
            super::Key::Insert => winit::keyboard::KeyCode::Insert as i32,
            super::Key::Delete => winit::keyboard::KeyCode::Delete as i32,
            super::Key::Backspace => winit::keyboard::KeyCode::Backspace as i32,
            super::Key::Space => winit::keyboard::KeyCode::Space as i32,
            super::Key::Enter => winit::keyboard::KeyCode::Enter as i32,
            super::Key::Escape => winit::keyboard::KeyCode::Escape as i32,
            super::Key::KeyPadEnter => winit::keyboard::KeyCode::NumpadEnter as i32,
        }
    }

    /// Set's whethere input from keybpard or mouse is available or not
    fn set_input_enabled(&mut self, keyboard: bool, mouse: bool) {
        panic!();
    }

    /// Get value for whether (keyboard, mouse) input is enabled
    fn get_input_enabled(&self) -> (bool, bool) {
        panic!();
        (false, false)
    }

    fn enumerate_display_monitors2(&self) -> Vec<super::MonitorInfo> {
        let primary_monitor = self.event_loop.primary_monitor();
        self.event_loop.available_monitors().map(|monitor| {
            let winit::dpi::PhysicalSize { width, height } = monitor.size();
            let winit::dpi::PhysicalPosition { x, y } = monitor.position();
            super::MonitorInfo {
                rect: Rect {
                    x, y,
                    width: width as i32,
                    height: height as i32
                },
                client_rect: Rect {
                    x, y,
                    width: width as i32,
                    height: height as i32
                },
                dpi_scale: monitor.scale_factor() as f32,
                primary: primary_monitor.as_ref() == Some(&monitor)
            }
        }).collect()
    }

    /// Enumerate all display monitors
    fn enumerate_display_monitors() -> Vec<super::MonitorInfo> {
        let event_loop = winit::event_loop::EventLoop::new().unwrap();
        let primary_monitor = event_loop.primary_monitor();

        event_loop.available_monitors().map(|monitor| {
            /*
            if let Some(name) = monitor.name() {
                info!("{intro}: {name}");
            } else {
                info!("{intro}: [no name]");
            }
            */

            let winit::dpi::PhysicalSize { width, height } = monitor.size();

            /*
            info!(
                "  Current mode: {width}x{height}{}",
                if let Some(m_hz) = monitor.refresh_rate_millihertz() {
                    format!(" @ {}.{} Hz", m_hz / 1000, m_hz % 1000)
                } else {
                    String::new()
                }
            );
            */

            let winit::dpi::PhysicalPosition { x, y } = monitor.position();

            //info!("  Position: {x},{y}");
            //info!("  Scale factor: {}", monitor.scale_factor());
            //info!("  Available modes (width x height x bit-depth):");

            /*
            for mode in monitor.video_modes() {
                let PhysicalSize { width, height } = mode.size();
                let bits = mode.bit_depth();
                let m_hz = mode.refresh_rate_millihertz();

                //info!("    {width}x{height}x{bits} @ {}.{} Hz", m_hz / 1000, m_hz % 1000);
            }
            */

            super::MonitorInfo {
                rect: Rect {
                    x, y,
                    width: width as i32,
                    height: height as i32
                },
                client_rect: Rect {
                    x, y,
                    width: width as i32,
                    height: height as i32
                },
                dpi_scale: monitor.scale_factor() as f32,
                primary: primary_monitor.as_ref() == Some(&monitor)
            }
        }).collect()
    }

    /// Sets the mouse cursor
    fn set_cursor(&self, cursor: &super::Cursor) {
        panic!();
    }

    /// Opens a native open file dialog window, exts are provided to filer selections. ie vec![".txt", ".png"]
    fn open_file_dialog(flags: super::OpenFileDialogFlags, exts: Vec<&str>) -> Result<Vec<String>, super::Error> {
        panic!();
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
        panic!();
    }
}

impl super::Window<App> for Window {
    /// Bring window to front and draw ontop of all others
    fn bring_to_front(&self) {
        // TODO: maybe?
        self.winit_window.focus_window();
    }

    /// Show window, specify true to show window or false to hide
    fn show(&self, show: bool, activate: bool) {
        self.winit_window.set_visible(show);
    }

    /// Must be called each frame to handle resizes, parent App my have events to forward
    fn update(&mut self, app: &mut App) {
        // stub
        // panic!();
    }

    /// Close the window
    fn close(&mut self) {
        // stub
        //panic!();
    }

    /// Change the windows style
    fn update_style(&mut self, flags: super::WindowStyleFlags, rect: super::Rect<i32>) {
        // stub?
        //panic!();
    }

    /// Returns true if the window is focused
    fn is_focused(&self) -> bool {
        self.winit_window.has_focus()
    }

    /// Returns true if the window is minimised
    fn is_minimised(&self) -> bool {
        if let Some(min) = self.winit_window.is_minimized() {
            min
        }
        else {
            false
        }
    }

    /// Sets focus to this window
    fn set_focused(&self) {
        self.winit_window.focus_window();
    }

    /// Returns true if the mouse if hovering this window
    fn is_mouse_hovered(&self) -> bool {
        panic!();
        false
    }

    /// Set the window display title that appears on the title bar
    fn set_title(&self, title: String) {
        self.winit_window.set_title(&title.as_str());
    }

    /// Set window position in screen space
    fn set_pos(&self, pos: super::Point<i32>) {
        self.winit_window.set_outer_position(LogicalPosition {
            x: pos.x,
            y: pos.y
        });
    }

    /// Set window size in screen coordinates
    fn set_size(&self, size: super::Size<i32>) {
        self.winit_window.request_inner_size(LogicalSize::new(size.x, size.y));
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
        panic!();
        super::Point {
            x: 0,
            y: 0
        }
    }

    /// Return the dpi scale for the current monitor the window is on
    fn get_dpi_scale(&self) -> f32 {
        self.winit_window.scale_factor() as f32
    }

    /// Gets the internal native handle
    fn get_native_handle(&self) -> NativeHandle {
        NativeHandle {
            handle: isize_id_from_window(self)
        }
    }

    /// Gets window events tracked from os update, to handle events inside external systems
    fn get_events(&self) -> super::WindowEventFlags {
        panic!();
        super::WindowEventFlags {
            bits: 0
        }
    }
    /// Clears events after they have been responded to
    fn clear_events(&mut self) {
        panic!();
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
        self.handle
    }
    fn copy(&self) -> NativeHandle {
        NativeHandle {
            handle: self.handle
        }
    }
}