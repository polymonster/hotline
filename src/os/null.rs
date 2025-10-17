#![allow(warnings)]

#[derive(Clone)]
pub struct Window;

#[derive(Clone)]
pub struct NativeHandle;

#[derive(Clone)]
pub struct App;

use super::Point as Point;
use super::Size as Size;
use super::Rect as Rect;
use super::WindowEventFlags;
use super::WindowStyleFlags;
use super::WindowInfo;
use super::MouseButton;
use super::Error;
use super::Key;
use super::SysKey;
use super::Cursor;
use super::MonitorInfo;
use super::OpenFileDialogFlags;

impl super::Window<App> for Window {
    fn bring_to_front(&self) {
        unimplemented!()
    }
    fn show(&self, show: bool, activate: bool)  {
        unimplemented!()
    }
    fn update(&mut self, app: &mut App) {
        unimplemented!()
    }
    fn close(&mut self)  {
        unimplemented!()
    }
    fn update_style(&mut self, flags: WindowStyleFlags, rect: Rect<i32>)  {
        unimplemented!()
    }
    fn is_focused(&self) -> bool {
        unimplemented!()
    }
    fn is_minimised(&self) -> bool {
        unimplemented!()
    }
    fn set_focused(&self) {
        unimplemented!()
    }
    fn is_mouse_hovered(&self) -> bool {
        unimplemented!()
    }
    fn set_title(&self, title: String) {
        unimplemented!()
    }
    fn set_pos(&self, pos: Point<i32>) {
        unimplemented!()
    }
    fn set_size(&self, size: Size<i32>) {
        unimplemented!()
    }
    fn get_pos(&self) -> Point<i32> {
        unimplemented!()
    }
    fn get_viewport_rect(&self) -> Rect<i32> {
        unimplemented!()
    }
    fn get_size(&self) -> Size<i32> {
        unimplemented!()
    }
    fn get_window_rect(&self) -> Rect<i32> {
        unimplemented!()
    }
    fn get_mouse_client_pos(&self, mouse_pos: Point<i32>) -> Point<i32> {
        unimplemented!()
    }
    fn get_dpi_scale(&self) -> f32 {
        unimplemented!()
    }
    fn get_native_handle(&self) -> NativeHandle {
        unimplemented!()
    }
    fn get_events(&self) -> WindowEventFlags {
        unimplemented!()
    }
    fn clear_events(&mut self)  {
        unimplemented!()
    }
    fn as_ptr(&self) -> *const Self  {
        unimplemented!()
    }
    fn as_mut_ptr(&mut self) -> *mut Self  {
        unimplemented!()
    }
}

impl super::NativeHandle<App> for NativeHandle {
    fn get_isize(&self) -> isize {
        unimplemented!()
    }
    fn copy(&self) -> Self {
        unimplemented!()
    }
}

impl super::App for App {
    type Window = Window;
    type NativeHandle = NativeHandle;

    fn create(info: super::AppInfo) -> Self {
        unimplemented!()
    }

    fn create_window(&mut self, info: WindowInfo<Self>) -> Self::Window {
        unimplemented!()
    }

    fn destroy_window(&mut self, window: &Self::Window) {
        unimplemented!()
    }

    fn run(&mut self) -> bool  {
        unimplemented!()
    }

    fn exit(&mut self, exit_code: i32) {
        unimplemented!()
    }

    fn get_mouse_pos(&self) -> Point<i32> {
        unimplemented!()
    }

    fn get_mouse_wheel(&self) -> f32 {
        unimplemented!()
    }

    fn get_mouse_hwheel(&self) -> f32 {
        unimplemented!()
    }

    fn get_mouse_buttons(&self) -> [bool; MouseButton::Count as usize] {
        unimplemented!()
    }

    fn get_mouse_pos_delta(&self) -> Size<i32> {
        unimplemented!()
    }

    fn get_utf16_input(&self) -> Vec<u16> {
        unimplemented!()
    }

    fn get_keys_down(&self) -> [bool; 256] {
        unimplemented!()
    }

    fn get_keys_pressed(&self) -> [bool; 256] {
        unimplemented!()
    }

    fn is_sys_key_down(&self, key: SysKey) -> bool {
        unimplemented!()
    }

    fn is_sys_key_pressed(&self, key: SysKey) -> bool {
        unimplemented!()
    }

    fn get_key_code(key: Key) -> i32 {
        unimplemented!()
    }

    fn set_input_enabled(&mut self, keyboard: bool, mouse: bool) {
        unimplemented!()
    }

    fn get_input_enabled(&self) -> (bool, bool) {
        unimplemented!()
    }

    fn enumerate_display_monitors() -> Vec<MonitorInfo> {
        unimplemented!()
    }

    fn set_cursor(&self, cursor: &Cursor) {
        unimplemented!()
    }

    fn open_file_dialog(flags: OpenFileDialogFlags, exts: Vec<&str>) -> Result<Vec<String>, Error> {
        unimplemented!()
    }

    fn get_console_window_rect(&self) -> Rect<i32> {
        unimplemented!()
    }

    fn set_console_window_rect(&self, rect: Rect<i32>) {
        unimplemented!()
    }
}