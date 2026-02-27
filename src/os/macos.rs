#![cfg(target_os = "macos")]

extern crate objc;

use core::panic;
use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;
use std::sync::RwLock;

use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent, ElementState},
    event_loop::{self, ControlFlow},
    keyboard::{Key, PhysicalKey, KeyCode},
    platform::pump_events::{EventLoopExtPumpEvents, PumpStatus},
    raw_window_handle::{HasWindowHandle, RawWindowHandle}
};

use cocoa::base::id as cocoa_id;

use crate::os::Rect;

/// Input state tracking (similar to ProcData in win32.rs)
#[derive(Clone)]
struct InputState {
    // Mouse state - screen coordinates
    mouse_pos: super::Point<i32>,
    mouse_pos_prev: super::Point<i32>,
    // Mouse state - window-local coordinates (from CursorMoved)
    mouse_client_pos: super::Point<i32>,
    mouse_down: [bool; super::MouseButton::Count as usize],
    mouse_wheel: f32,
    mouse_hwheel: f32,
    hovered_window_id: Option<winit::window::WindowId>,

    // Keyboard state
    key_down: [bool; 256],
    key_press: [bool; 256],
    key_debounce: [bool; 256],

    // System keys (Ctrl, Shift, Alt)
    sys_key_down: [bool; super::SysKey::Count as usize],
    sys_key_press: [bool; super::SysKey::Count as usize],
    sys_key_debounce: [bool; super::SysKey::Count as usize],

    // Text input
    utf16_inputs: Vec<u16>,

    // Input enabled flags
    keyboard_enabled: bool,
    mouse_enabled: bool,
}

impl InputState {
    fn new() -> Self {
        InputState {
            mouse_pos: super::Point { x: 0, y: 0 },
            mouse_pos_prev: super::Point { x: 0, y: 0 },
            mouse_client_pos: super::Point { x: 0, y: 0 },
            mouse_down: [false; super::MouseButton::Count as usize],
            mouse_wheel: 0.0,
            mouse_hwheel: 0.0,
            hovered_window_id: None,
            key_down: [false; 256],
            key_press: [false; 256],
            key_debounce: [false; 256],
            sys_key_down: [false; super::SysKey::Count as usize],
            sys_key_press: [false; super::SysKey::Count as usize],
            sys_key_debounce: [false; super::SysKey::Count as usize],
            utf16_inputs: Vec::new(),
            keyboard_enabled: true,
            mouse_enabled: true,
        }
    }
}

#[derive(Clone)]
pub struct App {
    event_loop: Arc<RwLock<winit::event_loop::EventLoop<()>>>,
    input_state: Arc<RwLock<InputState>>,
    windows: Arc<RwLock<HashMap<winit::window::WindowId, Arc<winit::window::Window>>>>,
}

unsafe impl Send for App {}
unsafe impl Sync for App {}

#[derive(Clone)]
pub struct Window {
    winit_window: Arc<winit::window::Window>,
    window_id: winit::window::WindowId,
    input_state: Arc<RwLock<InputState>>,
    events: Arc<RwLock<super::WindowEventFlags>>,
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

impl InputState {
    /// Debounce logic for keys - updates press and debounce based on current down state
    fn debounce_keys(&mut self) {
        for i in 0..256 {
            if self.key_down[i] && !self.key_press[i] && !self.key_debounce[i] {
                // First frame key down: trigger press
                self.key_press[i] = true;
                self.key_debounce[i] = true;
            } else if self.key_press[i] {
                // Subsequent frames: clear press
                self.key_press[i] = false;
            } else if !self.key_down[i] {
                // Key released: clear debounce
                self.key_debounce[i] = false;
            }
        }
    }

    /// Debounce logic for system keys
    fn debounce_sys_keys(&mut self) {
        for i in 0..super::SysKey::Count as usize {
            if self.sys_key_down[i] && !self.sys_key_press[i] && !self.sys_key_debounce[i] {
                self.sys_key_press[i] = true;
                self.sys_key_debounce[i] = true;
            } else if self.sys_key_press[i] {
                self.sys_key_press[i] = false;
            } else if !self.sys_key_down[i] {
                self.sys_key_debounce[i] = false;
            }
        }
    }
}

impl App {
    /// Update input state at the start of each frame
    fn update_input_state(&self) {
        let mut state = self.input_state.write().unwrap();

        // Reset per-frame values
        state.mouse_wheel = 0.0;
        state.mouse_hwheel = 0.0;
        state.utf16_inputs.clear();

        // Store previous mouse position for delta calculation
        // Current mouse_pos is computed on-demand in get_mouse_pos()
        state.mouse_pos_prev = state.mouse_pos;

        // Compute current screen position from window + client pos
        if let Some(window_id) = state.hovered_window_id {
            if let Some(window) = self.windows.read().unwrap().get(&window_id) {
                if let Ok(pos) = window.outer_position() {
                    state.mouse_pos = super::Point {
                        x: pos.x + state.mouse_client_pos.x,
                        y: pos.y + state.mouse_client_pos.y,
                    };
                }
            }
        }

        // Debounce keys
        state.debounce_keys();
        state.debounce_sys_keys();
    }
}

impl super::App for App {
    type Window = Window;
    type NativeHandle = NativeHandle;

    /// Create an application instance
    fn create(info: super::AppInfo) -> Self {
        App {
            event_loop: Arc::new(RwLock::new(winit::event_loop::EventLoop::new().unwrap())),
            input_state: Arc::new(RwLock::new(InputState::new())),
            windows: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new operating system window
    fn create_window(&mut self, info: super::WindowInfo<Self>) -> Self::Window {
        let window = winit::window::WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(info.rect.width, info.rect.height))
            .with_position(winit::dpi::LogicalPosition::new(info.rect.x, info.rect.y))
            .with_title(info.title)
            .build(&*self.event_loop.read().unwrap())
            .unwrap();
        let window_id = window.id();
        let winit_window = Arc::new(window);

        // Register window for position lookups
        self.windows.write().unwrap().insert(window_id, winit_window.clone());

        Window {
            winit_window,
            window_id,
            input_state: self.input_state.clone(),
            events: Arc::new(RwLock::new(super::WindowEventFlags::NONE)),
        }
    }

    /// Destroy window, unregistering app tracking
    fn destroy_window(&mut self, window: &Self::Window) {
        panic!();
    }

    /// Call to update windows and os state each frame, when false is returned the app has been requested to close
    fn run(&mut self) -> bool {
        objc::rc::autoreleasepool(|| {
            // Update input state at frame start
            self.update_input_state();

            let mut resume = true;
            let input_state = self.input_state.clone();

            let _ = self.event_loop.write().and_then(|mut event_loop| {
                let _status = event_loop.pump_events(Some(Duration::ZERO), |event, _elwt| {
                    match event {
                        Event::WindowEvent { event, window_id } => {
                            let mut state = input_state.write().unwrap();

                            match event {
                                WindowEvent::CloseRequested => {
                                    resume = false;
                                }
                                WindowEvent::RedrawRequested => {}

                                // Mouse cursor position (window-relative from winit)
                                WindowEvent::CursorMoved { position, .. } => {
                                    if state.mouse_enabled {
                                        // winit gives us logical coordinates relative to window content area
                                        state.mouse_client_pos = super::Point {
                                            x: position.x as i32,
                                            y: position.y as i32,
                                        };
                                        state.hovered_window_id = Some(window_id);
                                    }
                                }

                                // Mouse enter/leave for hover tracking
                                WindowEvent::CursorEntered { .. } => {
                                    state.hovered_window_id = Some(window_id);
                                }
                                WindowEvent::CursorLeft { .. } => {
                                    if state.hovered_window_id == Some(window_id) {
                                        state.hovered_window_id = None;
                                    }
                                }

                                // Mouse buttons
                                WindowEvent::MouseInput { state: element_state, button, .. } => {
                                    if state.mouse_enabled {
                                        let pressed = element_state == ElementState::Pressed;
                                        // Map to MouseButton enum order: Left=0, Middle=1, Right=2, X1=3, X2=4
                                        let index = match button {
                                            winit::event::MouseButton::Left => Some(super::MouseButton::Left as usize),
                                            winit::event::MouseButton::Middle => Some(super::MouseButton::Middle as usize),
                                            winit::event::MouseButton::Right => Some(super::MouseButton::Right as usize),
                                            winit::event::MouseButton::Back => Some(super::MouseButton::X1 as usize),
                                            winit::event::MouseButton::Forward => Some(super::MouseButton::X2 as usize),
                                            winit::event::MouseButton::Other(_) => None,
                                        };
                                        if let Some(idx) = index {
                                            state.mouse_down[idx] = pressed;
                                        }
                                    }
                                }

                                // Mouse wheel
                                WindowEvent::MouseWheel { delta, .. } => {
                                    if state.mouse_enabled {
                                        match delta {
                                            winit::event::MouseScrollDelta::LineDelta(h, v) => {
                                                state.mouse_wheel += v;
                                                state.mouse_hwheel += h;
                                            }
                                            winit::event::MouseScrollDelta::PixelDelta(pos) => {
                                                // Convert pixel delta to line delta (approximate)
                                                state.mouse_wheel += (pos.y / 20.0) as f32;
                                                state.mouse_hwheel += (pos.x / 20.0) as f32;
                                            }
                                        }
                                    }
                                }

                                // Keyboard input
                                WindowEvent::KeyboardInput { event, .. } => {
                                    if state.keyboard_enabled {
                                        let pressed = event.state == ElementState::Pressed;

                                        // Get physical key code for key_down array
                                        if let PhysicalKey::Code(key_code) = event.physical_key {
                                            let code = key_code as usize;
                                            if code < 256 {
                                                state.key_down[code] = pressed;
                                            }
                                        }

                                        // Handle text input from logical key
                                        if pressed {
                                            if let Key::Character(ref c) = event.logical_key {
                                                for ch in c.encode_utf16() {
                                                    state.utf16_inputs.push(ch);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Modifier keys
                                WindowEvent::ModifiersChanged(modifiers) => {
                                    if state.keyboard_enabled {
                                        let mods = modifiers.state();
                                        state.sys_key_down[super::SysKey::Ctrl as usize] = mods.control_key();
                                        state.sys_key_down[super::SysKey::Shift as usize] = mods.shift_key();
                                        state.sys_key_down[super::SysKey::Alt as usize] = mods.alt_key();
                                    }
                                }

                                _ => {}
                            }
                        }
                        _ => {}
                    }
                });

                Ok(())
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
        self.input_state.read().unwrap().mouse_pos
    }

    /// Retuns the mouse vertical wheel position
    fn get_mouse_wheel(&self) -> f32 {
        self.input_state.read().unwrap().mouse_wheel
    }

    /// Retuns the mouse horizontal wheel positions
    fn get_mouse_hwheel(&self) -> f32 {
        self.input_state.read().unwrap().mouse_hwheel
    }

    /// Retuns the mouse button states, up or down
    fn get_mouse_buttons(&self) -> [bool; super::MouseButton::Count as usize] {
        self.input_state.read().unwrap().mouse_down
    }

    /// Returns the distance the mouse has moved since the last frame
    fn get_mouse_pos_delta(&self) -> super::Size<i32> {
        let state = self.input_state.read().unwrap();
        super::Size {
            x: state.mouse_pos.x - state.mouse_pos_prev.x,
            y: state.mouse_pos.y - state.mouse_pos_prev.y,
        }
    }

    /// Returns a vector of utf-16 characters that have been input since the last frame
    fn get_utf16_input(&self) -> Vec<u16> {
        self.input_state.read().unwrap().utf16_inputs.clone()
    }

    /// Returns an array of bools containing 0-256 keys down (true) or up (false)
    fn get_keys_down(&self) -> [bool; 256] {
        self.input_state.read().unwrap().key_down
    }

    /// Returns an array of bools containing 0-256 of keys pressed, will trigger only once and then require debouce
    fn get_keys_pressed(&self) -> [bool; 256] {
        self.input_state.read().unwrap().key_press
    }

    /// Returns true if the sys key is down and false if the key is up
    fn is_sys_key_down(&self, key: super::SysKey) -> bool {
        self.input_state.read().unwrap().sys_key_down[key as usize]
    }

    /// Returns true if the sys key is pressed this frame and
    /// requires debounce until it is pressed again
    fn is_sys_key_pressed(&self, key: super::SysKey) -> bool {
        self.input_state.read().unwrap().sys_key_press[key as usize]
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
        let mut state = self.input_state.write().unwrap();
        state.keyboard_enabled = keyboard;
        state.mouse_enabled = mouse;
    }

    /// Get value for whether (keyboard, mouse) input is enabled
    fn get_input_enabled(&self) -> (bool, bool) {
        let state = self.input_state.read().unwrap();
        (state.keyboard_enabled, state.mouse_enabled)
    }

    fn enumerate_display_monitors(&self) -> Vec<super::MonitorInfo> {
        let event_loop = &*self.event_loop.read().unwrap();
        let primary_monitor = event_loop.primary_monitor();
        event_loop.available_monitors().map(|monitor| {
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
        // stub
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
        let state = self.input_state.read().unwrap();
        state.hovered_window_id == Some(self.window_id)
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
    fn get_mouse_client_pos(&self, _mouse_pos: super::Point<i32>) -> super::Point<i32> {
        // Use the tracked client position from CursorMoved events
        // which is already in window-local coordinates
        self.input_state.read().unwrap().mouse_client_pos
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
        *self.events.read().unwrap()
    }

    /// Clears events after they have been responded to
    fn clear_events(&mut self) {
        *self.events.write().unwrap() = super::WindowEventFlags::NONE;
    }

    /// Const pointer
    fn as_ptr(&self) -> *const Self {
        unimplemented!()
    }

    /// Mut pointer
    fn as_mut_ptr(&mut self) -> *mut Self {
        unimplemented!()
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