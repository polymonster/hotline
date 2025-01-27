#![cfg(target_os = "windows")]

use windows::{
    core::*,
    Win32::Foundation::*, Win32::Globalization::*, Win32::Graphics::Gdi::ClientToScreen,
    Win32::Graphics::Gdi::EnumDisplayMonitors, Win32::Graphics::Gdi::GetMonitorInfoA,
    Win32::Graphics::Gdi::MonitorFromWindow, Win32::Graphics::Gdi::ScreenToClient,
    Win32::Graphics::Gdi::ValidateRect, Win32::Graphics::Gdi::HDC, Win32::Graphics::Gdi::HMONITOR,
    Win32::Graphics::Gdi::MONITORINFO, Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
    Win32::System::LibraryLoader::*, Win32::UI::Controls::*, Win32::UI::HiDpi::*,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
    Win32::System::Com::CoCreateInstance, Win32::System::Com::CoInitialize, Win32::System::Com::CLSCTX_ALL,
    Win32::UI::Shell::*, Win32::UI::Shell::Common::COMDLG_FILTERSPEC,
    Win32::System::Console::GetConsoleWindow
};

use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::result;

use std::collections::HashMap;
use std::ffi::CString;

use crate::static_ref;
use crate::static_ref_mut;

#[derive(Clone)]
pub struct App {
    window_class: String,
    window_class_imgui: String,
    hinstance: usize,
    mouse_pos: super::Point<i32>,
    mouse_pos_delta: super::Point<i32>,
    proc_data: ProcData,
    events: HashMap<usize, super::WindowEventFlags>,
    hwnd_flags: HashMap<usize, super::WindowStyleFlags>,
    keyboard_input_enabled: bool,
    mouse_input_enabled: bool,
}

impl ProcData {
    fn new() -> Self {
        ProcData {
            mouse_hwnd: 0,
            mouse_tracked: false,
            mouse_down: [false; 5],
            mouse_wheel: 0.0,
            mouse_hwheel: 0.0,
            utf16_inputs: Vec::new(),
            key_down: [false; 256],
            key_press: [false; 256],
            key_debounce: [false; 256],
            sys_key_down: [false; super::SysKey::Count as usize],
            sys_key_press: [false; super::SysKey::Count as usize],
            sys_key_debounce: [false; super::SysKey::Count as usize],
        }
    }
}

#[derive(Clone)]
pub struct Window {
    hwnd: usize,
    ws: WINDOW_STYLE,
    wsex: WINDOW_EX_STYLE,
    events: super::WindowEventFlags,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

#[derive(Clone, Copy)]
pub struct NativeHandle {
    pub hwnd: usize,
}

#[derive(Clone)]
struct ProcData {
    mouse_hwnd: usize,
    mouse_tracked: bool,
    mouse_down: [bool; super::MouseButton::Count as usize],
    mouse_wheel: f32,
    mouse_hwheel: f32,
    utf16_inputs: Vec<u16>,
    key_down: [bool; 256],
    key_press: [bool; 256],
    key_debounce: [bool; 256],
    sys_key_down: [bool; super::SysKey::Count as usize],
    sys_key_press: [bool; super::SysKey::Count as usize],
    sys_key_debounce: [bool; super::SysKey::Count as usize],
}

impl super::NativeHandle<App> for NativeHandle {
    fn get_isize(&self) -> isize {
        self.hwnd as isize
    }
    fn copy(&self) -> NativeHandle {
        *self
    }
}

impl Window {
    pub fn get_hwnd(&self) -> HWND {
        unsafe {
            std::mem::transmute(self.hwnd)
        }
    }
}

fn as_hwnd(handle: usize) -> HWND {
    unsafe { std::mem::transmute(handle) }
}

fn as_hinstance(handle: usize) -> HINSTANCE {
    unsafe { std::mem::transmute(handle) }
}

fn hwnd_usize(h: HWND) -> usize {
    unsafe { std::mem::transmute(h.0) }
}

fn hinstance_usize(h: HINSTANCE) -> usize {
    unsafe { std::mem::transmute(h.0) }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            if self.hwnd != 0 {
                if GetCapture() == as_hwnd(self.hwnd) {
                    ReleaseCapture().expect("hotline_rs::win32::error: call to ReleaseCapture failed");
                }
                let _ = DestroyWindow(as_hwnd(self.hwnd));
            }
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterClassA(PCSTR(self.window_class.as_ptr() as _), as_hinstance(self.hinstance));
        }
    }
}

/// minimal set of flags... just what is requires for imgui so far
fn to_win32_dw_style(style: &super::WindowStyleFlags) -> WINDOW_STYLE {
    let mut win32_style = 0;
    if style.contains(super::WindowStyleFlags::POPUP) {
        win32_style |= WS_POPUP.0;
    }
    if style.contains(super::WindowStyleFlags::OVERLAPPED_WINDOW) {
        win32_style |= WS_OVERLAPPEDWINDOW.0;
    }
    if style.contains(super::WindowStyleFlags::VISIBLE) {
        win32_style |= WS_VISIBLE.0;
    }
    // default style
    if win32_style == 0 {
        win32_style |= WS_OVERLAPPEDWINDOW.0 | WS_VISIBLE.0;
    }
    WINDOW_STYLE(win32_style)
}

fn to_win32_dw_ex_style(style: &super::WindowStyleFlags) -> WINDOW_EX_STYLE {
    let mut win32_style = 0;
    if style.contains(super::WindowStyleFlags::TOOL_WINDOW) {
        win32_style |= WS_EX_TOOLWINDOW.0;
    }
    if style.contains(super::WindowStyleFlags::APP_WINDOW) {
        win32_style |= WS_EX_APPWINDOW.0;
    }
    if style.contains(super::WindowStyleFlags::TOPMOST) {
        win32_style |= WS_EX_TOPMOST.0;
    }
    WINDOW_EX_STYLE(win32_style)
}

const fn to_win32_key_code(key: super::Key) -> i32 {
    match key {
        super::Key::Tab => VK_TAB.0 as i32,
        super::Key::Left => VK_LEFT.0 as i32,
        super::Key::Right => VK_RIGHT.0 as i32,
        super::Key::Up => VK_UP.0 as i32,
        super::Key::Down => VK_DOWN.0 as i32,
        super::Key::PageUp => VK_PRIOR.0 as i32,
        super::Key::PageDown => VK_NEXT.0 as i32,
        super::Key::Home => VK_HOME.0 as i32,
        super::Key::End => VK_END.0 as i32,
        super::Key::Insert => VK_INSERT.0 as i32,
        super::Key::Delete => VK_DELETE.0 as i32,
        super::Key::Backspace => VK_BACK.0 as i32,
        super::Key::Space => VK_SPACE.0 as i32,
        super::Key::Enter => VK_RETURN.0 as i32,
        super::Key::Escape => VK_ESCAPE.0 as i32,
        super::Key::KeyPadEnter => VK_RETURN.0 as i32,
    }
}

fn adjust_window_rect(
    rect: &super::Rect<i32>,
    ws: WINDOW_STYLE,
    wsex: WINDOW_EX_STYLE,
) -> super::Rect<i32> {
    let mut rc = RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    };
    unsafe {
        AdjustWindowRectEx(&mut rc, ws, BOOL::from(false), wsex).expect("hotline_rs::win32::error: failed AdjustWindowRectEx");
    }
    super::Rect::<i32> {
        x: rc.left,
        y: rc.top,
        width: rc.right - rc.left,
        height: rc.bottom - rc.top,
    }
}

pub fn string_to_wide(string: String) -> Vec<u16> {
    unsafe {
        let null_string = CString::new(string).unwrap();
        let mut vx : Vec<u16> = Vec::new();
        let n = MultiByteToWideChar(
            windows::Win32::Globalization::CP_UTF8,
            windows::Win32::Globalization::MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            null_string.as_bytes(),
            Some(vx.as_mut_slice()),
        );
        let mut v: Vec<u16> = vec![0; n as usize];
        MultiByteToWideChar(
            windows::Win32::Globalization::CP_UTF8,
            windows::Win32::Globalization::MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            null_string.as_bytes(),
            Some(v.as_mut_slice()),
        );
        v
    }
}

pub fn wide_to_string(wide: PWSTR) -> String {
    let mut v : Vec<u16> = Vec::new();
    let mut counter = 0;
    unsafe {
        // run the string length to find the terminator
        while *wide.0.offset(counter) != 0 {
            v.push(*wide.0.offset(counter));
            counter += 1;
        }
    }
    let decoded = decode_utf16(v)
        .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
        .collect::<String>();

    // trim utf-16 nul terminators
    let x: &[_] = &['\0', '\0'];
    decoded.trim_matches(x).to_string()
}

impl App {
    fn update_input(&mut self) {
        unsafe {
            // reset input state
            self.proc_data.mouse_wheel = 0.0;
            self.proc_data.mouse_hwheel = 0.0;
            self.proc_data.utf16_inputs.clear();
            // get new mouse pos
            let mut mouse_pos = POINT::default();
            let _ = GetCursorPos(&mut mouse_pos);
            let new_mouse_pos = super::Point {
                x: mouse_pos.x,
                y: mouse_pos.y,
            };
            // mouse pos delta
            self.mouse_pos_delta = super::Point {
                x: new_mouse_pos.x - self.mouse_pos.x,
                y: new_mouse_pos.y - self.mouse_pos.y,
            };
            // set new mouse pos as current
            self.mouse_pos = new_mouse_pos;
        }

        let debounce_keys = |count: usize, down: &mut [bool], press: &mut [bool], debounce: &mut [bool]| {
            // update press states
            for i in 0..count {
                // set the key press in the first instance of the frame
                if down[i] && !press[i] && !debounce[i] {
                    // trigegr press in the first down instance
                    press[i] = true;
                    debounce[i] = true;
                }
                else if press[i] {
                    // unset the press
                    press[i] = false;
                }
                else if !down[i] {
                    // debounce the press
                    debounce[i] = false;
                }
            }
        };

        // debounce normal keys
        debounce_keys(
            256,
            &mut self.proc_data.key_down,
            &mut self.proc_data.key_press,
            &mut self.proc_data.key_debounce
        );

        // debounce sys keys
        debounce_keys(
            super::SysKey::Count as usize,
            &mut self.proc_data.sys_key_down,
            &mut self.proc_data.sys_key_press,
            &mut self.proc_data.sys_key_debounce
        );
    }

    fn wndproc(&mut self, window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let proc_data = &mut self.proc_data;
            match message {
                WM_MOUSEMOVE => {
                    proc_data.mouse_hwnd = hwnd_usize(window);
                    if !proc_data.mouse_tracked {
                        // We need to call TrackMouseEvent in order to receive WM_MOUSELEAVE events
                        TrackMouseEvent(&mut TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: window,
                            dwHoverTime: 0,
                        }).expect("hotline_rs::win32::error: TrackMouseEvent failed");
                        proc_data.mouse_tracked = true;
                    }
                    LRESULT(0)
                }
                WM_MOUSELEAVE => {
                    proc_data.mouse_hwnd = 0;
                    proc_data.mouse_tracked = false;
                    LRESULT(0)
                }
                WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                    proc_data.mouse_down[0] = true;
                    self.set_capture(window);
                    LRESULT(0)
                }
                WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                    proc_data.mouse_down[1] = true;
                    self.set_capture(window);
                    LRESULT(0)
                }
                WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                    proc_data.mouse_down[2] = true;
                    self.set_capture(window);
                    LRESULT(0)
                }
                WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                    let button = ((wparam.0 >> 16) & 0xffff) + 1;
                    proc_data.mouse_down[button] = true;
                    self.set_capture(window);
                    LRESULT(0)
                }
                WM_LBUTTONUP => {
                    proc_data.mouse_down[0] = false;
                    self.release_capture(window);
                    LRESULT(0)
                }
                WM_RBUTTONUP => {
                    proc_data.mouse_down[1] = false;
                    self.release_capture(window);
                    LRESULT(0)
                }
                WM_MBUTTONUP => {
                    proc_data.mouse_down[2] = false;
                    self.release_capture(window);
                    LRESULT(0)
                }
                WM_XBUTTONUP => {
                    let button = ((wparam.0 >> 16) & 0xffff) + 1;
                    proc_data.mouse_down[button] = true;
                    self.release_capture(window);
                    LRESULT(0)
                }
                WM_MOUSEWHEEL => {
                    let wheel_delta = ((wparam.0 >> 16) & 0xffff) as i16;
                    proc_data.mouse_wheel += (wheel_delta as f32) / (WHEEL_DELTA as f32);
                    LRESULT(0)
                }
                WM_MOUSEHWHEEL => {
                    let wheel_delta = ((wparam.0 >> 16) & 0xffff) as i16;
                    proc_data.mouse_hwheel += (wheel_delta as f32) / (WHEEL_DELTA as f32);
                    LRESULT(0)
                }
                WM_PAINT => {
                    let _ = ValidateRect(window, None);
                    LRESULT(0)
                }
                WM_CHAR => {
                    if wparam.0 > 0 && wparam.0 < 0x10000 {
                        proc_data.utf16_inputs.push(wparam.0 as u16);
                    }
                    LRESULT(0)
                }
                WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
                    let down = (message == WM_KEYDOWN) || (message == WM_SYSKEYDOWN);

                    if wparam.0 < 256 {
                        proc_data.key_down[wparam.0] = down;
                    }

                    let vk = VIRTUAL_KEY(wparam.0 as u16);
                    match vk {
                        VK_CONTROL => {
                            proc_data.sys_key_down[super::SysKey::Ctrl as usize] = down;
                        }
                        VK_SHIFT => {
                            proc_data.sys_key_down[super::SysKey::Shift as usize] = down;
                        }
                        VK_MENU => {
                            proc_data.sys_key_down[super::SysKey::Alt as usize] = down;
                        }
                        _ => {}
                    }

                    LRESULT(0)
                }
                _ => DefWindowProcA(window, message, wparam, lparam),
            }
        }
    }

    fn main_wndproc(
        &mut self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_DESTROY => {
                LRESULT(0)
            }
            WM_SYSCOMMAND => {
                if (wparam.0 & 0xfff0) == SC_KEYMENU as usize {
                    // Disable ALT application menu
                    LRESULT(0)
                } else {
                    self.wndproc(window, message, wparam, lparam)
                }
            }
            _ => self.wndproc(window, message, wparam, lparam),
        }
    }

    fn add_event(&mut self, window: HWND, flags: super::WindowEventFlags) {
        let iwindow = hwnd_usize(window);
        if let Some(window_events) = self.events.get_mut(&iwindow) {
            // or into existsing key
            *window_events |= flags;
        } else {
            // create new key
            self.events.insert(iwindow, flags);
        }
    }

    fn imgui_wndproc(
        &mut self,
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match message {
            WM_CLOSE => {
                self.add_event(window, super::WindowEventFlags::CLOSE);
                LRESULT(0)
            }
            WM_MOVE => {
                self.add_event(window, super::WindowEventFlags::MOVE);
                LRESULT(0)
            }
            WM_SIZE => {
                self.add_event(window, super::WindowEventFlags::SIZE);
                LRESULT(0)
            }
            WM_MOUSEACTIVATE => LRESULT(0),
            _ => self.wndproc(window, message, wparam, lparam),
        }
    }

    fn set_capture(&mut self, window: HWND) {
        unsafe {
            let any_down = self.proc_data.mouse_down.iter().any(|v| v == &true);
            if !any_down && GetCapture() == HWND(std::ptr::null_mut()) {
                SetCapture(window);
            }
        }
    }

    fn release_capture(&mut self, window: HWND) {
        unsafe {
            let any_down = self.proc_data.mouse_down.iter().any(|v| v == &true);
            if !any_down && GetCapture() == window {
                ReleaseCapture().expect("hotline_rs::win32::error: call to ReleaseCapture failed")
            }
        }
    }
}

impl super::App for App {
    type Window = Window;
    type NativeHandle = NativeHandle;

    fn create(info: super::AppInfo) -> Self {
        unsafe {
            // initialise com
            CoInitialize(None).unwrap();

            let window_class = info.name.to_string() + "\0";
            let window_class_imgui = info.name.to_string() + "_imgui\0";

            let instance = GetModuleHandleA(None).unwrap();
            debug_assert!(instance.0 != std::ptr::null_mut());
            let instance = HINSTANCE(instance.0);

            if info.dpi_aware {
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                if SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE).is_err() {
                    println!("hotline_rs::os::win32: SetProcessDpiAwareness failed");
                }
            }

            let wc = WNDCLASSA {
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
                hInstance: instance,
                lpszClassName: PCSTR(window_class.as_ptr() as _),
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(main_wndproc),
                ..Default::default()
            };

            if RegisterClassA(&wc) == 0 {
                panic!("hotline_rs::os::win32: class already registered!");
            }

            let wc2 = WNDCLASSA {
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
                hInstance: instance,
                lpszClassName: PCSTR(window_class_imgui.as_ptr() as _),
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(imgui_wndproc),
                ..Default::default()
            };

            if RegisterClassA(&wc2) == 0 {
                panic!("hotline_rs::os::win32: imgui class already registered!");
            }

            App {
                window_class_imgui,
                window_class,
                hinstance: hinstance_usize(instance),
                mouse_pos: super::Point::default(),
                mouse_pos_delta: super::Point::default(),
                proc_data: ProcData::new(),
                events: HashMap::new(),
                hwnd_flags: HashMap::new(),
                keyboard_input_enabled: true,
                mouse_input_enabled: true
            }
        }
    }

    fn create_window(&mut self, info: super::WindowInfo<Self>) -> Window {
        unsafe {
            let ws = to_win32_dw_style(&info.style);
            let wsex = to_win32_dw_ex_style(&info.style);

            let rect = adjust_window_rect(&info.rect, ws, wsex);

            let parent_hwnd = if info.parent_handle.is_some() {
                as_hwnd(info.parent_handle.unwrap().hwnd)
            } else {
                as_hwnd(0)
            };

            let class = if info.style.contains(super::WindowStyleFlags::IMGUI) {
                self.window_class_imgui.clone()
            } else {
                self.window_class.clone()
            };

            let null_class = class + "\0";
            let null_title = info.title.clone() + "\0";

            let hwnd = CreateWindowExA(
                wsex,
                PCSTR(null_class.as_ptr() as _),
                PCSTR(null_title.as_ptr() as _),
                ws,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                parent_hwnd,
                None,
                as_hinstance(self.hinstance),
                None,
            ).unwrap();

            let ihwnd = hwnd_usize(hwnd);

            // track window style to send to correct wnd proc
            self.hwnd_flags.insert(ihwnd, info.style);

            Window {
                hwnd: ihwnd,
                ws,
                wsex,
                events: super::WindowEventFlags::NONE,
            }
        }
    }

    fn destroy_window(&mut self, window: &Window) {
        self.hwnd_flags.remove(&window.hwnd);
    }

    fn run(&mut self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut quit = false;

            self.update_input();
            loop {
                if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE).into() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageA(&msg);

                    // handle wnd proc on self functions, to avoid need for static mutable state
                    if let Some(hwnd_flags) = self.hwnd_flags.get(&hwnd_usize(msg.hwnd)) {
                        if hwnd_flags.contains(super::WindowStyleFlags::IMGUI) {
                            self.imgui_wndproc(msg.hwnd, msg.message, msg.wParam, msg.lParam);
                        }
                        else {
                            self.main_wndproc(msg.hwnd, msg.message, msg.wParam, msg.lParam);
                        }
                    }

                    if msg.message == WM_QUIT {
                        quit = true;
                        break;
                    }
                }
                else {
                    break;
                }
            }
            !quit
        }
    }

    fn exit(&mut self, exit_code: i32) {
        unsafe {
            println!("hotline_rs::os::win32:: exiting with code ({})", exit_code);
            PostQuitMessage(exit_code);
        }
    }

    fn get_mouse_pos(&self) -> super::Point<i32> {
        self.mouse_pos
    }

    fn get_mouse_wheel(&self) -> f32 {
        self.proc_data.mouse_wheel
    }

    fn get_mouse_hwheel(&self) -> f32 {
        self.proc_data.mouse_hwheel
    }

    fn get_mouse_buttons(&self) -> [bool; super::MouseButton::Count as usize] {
        self.proc_data.mouse_down
    }

    fn get_mouse_pos_delta(&self) -> super::Size<i32> {
        self.mouse_pos_delta
    }

    fn get_utf16_input(&self) -> Vec<u16> {
        self.proc_data.utf16_inputs.to_vec()
    }

    fn get_keys_down(&self) -> [bool; 256] {
        self.proc_data.key_down
    }

    fn is_sys_key_down(&self, key: super::SysKey) -> bool {
        self.proc_data.sys_key_down[key as usize]
    }

    fn get_keys_pressed(&self) -> [bool; 256] {
        self.proc_data.key_press
    }

    fn is_sys_key_pressed(&self, key: super::SysKey) -> bool {
        self.proc_data.sys_key_press[key as usize]
    }

    fn get_key_code(key: super::Key) -> i32 {
        to_win32_key_code(key)
    }

    fn set_input_enabled(&mut self, keyboard: bool, mouse: bool) {
        self.keyboard_input_enabled = keyboard;
        self.mouse_input_enabled = mouse;
    }

    fn get_input_enabled(&self) -> (bool, bool) {
        (self.keyboard_input_enabled, self.mouse_input_enabled)
    }

    fn enumerate_display_monitors() -> Vec<super::MonitorInfo> {
        unsafe {
            static_ref_mut!(MONITOR_ENUM).clear();
            let _ = EnumDisplayMonitors(HDC::default(), None, Some(enum_func), LPARAM(0));
            let mut monitors: Vec<super::MonitorInfo> = Vec::new();
            for m in static_ref!(MONITOR_ENUM) {
                monitors.push(m.clone());
            }
            monitors
        }
    }

    fn set_cursor(&self, cursor: &super::Cursor) {
        unsafe {
            let hinstance = as_hinstance(self.hinstance);
            match cursor {
                super::Cursor::None => SetCursor(HCURSOR(std::ptr::null_mut())),
                super::Cursor::Arrow => SetCursor(LoadCursorW(hinstance, IDC_ARROW).unwrap()),
                super::Cursor::TextInput => SetCursor(LoadCursorW(hinstance, IDC_IBEAM).unwrap()),
                super::Cursor::ResizeAll => SetCursor(LoadCursorW(hinstance, IDC_SIZEALL).unwrap()),
                super::Cursor::ResizeEW => SetCursor(LoadCursorW(hinstance, IDC_SIZEWE).unwrap()),
                super::Cursor::ResizeNS => SetCursor(LoadCursorW(hinstance, IDC_SIZENS).unwrap()),
                super::Cursor::ResizeNESW => SetCursor(LoadCursorW(hinstance, IDC_SIZENESW).unwrap()),
                super::Cursor::ResizeNWSE => SetCursor(LoadCursorW(hinstance, IDC_SIZENWSE).unwrap()),
                super::Cursor::Hand => SetCursor(LoadCursorW(hinstance, IDC_HAND).unwrap()),
                super::Cursor::NotAllowed => SetCursor(LoadCursorW(hinstance, IDC_NO).unwrap()),
            };
        }
    }

    fn open_file_dialog(flags: super::OpenFileDialogFlags, exts: Vec<&str>) -> result::Result<Vec<String>, super::Error> {
        unsafe {
            let open_dialog : IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;

            // set option flags
            let mut ioptions = 0;
            if flags.contains(super::OpenFileDialogFlags::FOLDERS) {
                ioptions |= FOS_PICKFOLDERS.0;
            }

            if flags.contains(super::OpenFileDialogFlags::MULTI_SELECT) {
                ioptions |= FOS_ALLOWMULTISELECT.0;
            }

            // set options
            open_dialog.SetOptions(FILEOPENDIALOGOPTIONS(ioptions))?;

            // set file filters
            let mut wide_exts : Vec<Vec<u16>> = Vec::new();
            for ext in &exts {
                wide_exts.push(string_to_wide(format!("*{}", ext).to_string()));
            }

            // keep specs in scope
            let mut specs : Vec<COMDLG_FILTERSPEC> = Vec::new();
            if !wide_exts.is_empty() {
                for w in &wide_exts {
                    specs.push(COMDLG_FILTERSPEC {
                        pszName: PCWSTR(w.as_ptr() as _),
                        pszSpec: PCWSTR(w.as_ptr() as _),
                    });
                }
                open_dialog.SetFileTypes(&specs)?;
            }

            open_dialog.Show(HWND::default())?;
            let results : IShellItemArray = open_dialog.GetResults()?;

            let mut output_results : Vec<String> = Vec::new();
            let count = results.GetCount()?;
            for i in 0..count {
                let item : IShellItem = results.GetItemAt(i)?;
                let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
                let name = wide_to_string(name);
                output_results.push(name);
            }

            Ok(output_results)
        }
    }

    fn get_console_window_rect(&self) -> super::Rect<i32> {
        unsafe {
            let chwnd = GetConsoleWindow();
            let mut rect = RECT::default();
            let _ = GetWindowRect(chwnd, &mut rect);
            super::Rect::<i32> {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            }
        }
    }

    fn set_console_window_rect(&self, rect: super::Rect<i32>) {
        unsafe {
            let chwnd = GetConsoleWindow();
            SetWindowPos(
                chwnd,
                HWND::default(),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE,
            ).expect("hotline_rs::win32::error: call to SetWindowPos failed");
        }
    }
}

impl super::Window<App> for Window {
    fn bring_to_front(&self) {
        let hwnd = as_hwnd(self.hwnd);
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            SetFocus(hwnd).expect("hotline_rs::win32::error: call to SetFocus failed");
            SetActiveWindow(hwnd).expect("hotline_rs::win32::error: call to SetActiveWindow failed");
            BringWindowToTop(hwnd).expect("hotline_rs::win32::error: call to BringWindowToTop failed");
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
    }

    fn show(&self, show: bool, activate: bool) {
        let mut cmd = SW_HIDE;
        if show {
            cmd = SW_SHOWNA;
            if activate {
                cmd = SW_SHOW;
            }
        }
        unsafe {
            let hwnd = as_hwnd(self.hwnd);
            let _ = ShowWindow(hwnd, cmd);
        }
    }

    fn close(&mut self) {
        unsafe {
            let hwnd = as_hwnd(self.hwnd);
            DestroyWindow(hwnd).expect("hotline_rs::win32::error: call to DestroyWindow failed");
        }
    }

    fn update(&mut self, app: &mut App) {
        // take events
        if let Some(window_events) = app.events.get_mut(&self.hwnd) {
            self.events = *window_events;
            *window_events = super::WindowEventFlags::NONE;
        }
    }

    fn update_style(&mut self, flags: super::WindowStyleFlags, rect: super::Rect<i32>) {
        let ws = to_win32_dw_style(&flags);
        let wsex = to_win32_dw_ex_style(&flags);
        if ws != self.ws || wsex != self.wsex {
            let top_most_changed = (wsex & WS_EX_TOPMOST) != (self.wsex & WS_EX_TOPMOST);
            let swp_flag = if top_most_changed {
                SET_WINDOW_POS_FLAGS(0)
            } else {
                SWP_NOZORDER
            };

            let insert_after =
                if flags.contains(super::WindowStyleFlags::TOPMOST) && top_most_changed {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };

            self.ws = ws;
            self.wsex = wsex;
            unsafe {
                let hwnd = as_hwnd(self.hwnd);
                SetWindowLongA(hwnd, GWL_STYLE, ws.0 as i32);
                SetWindowLongA(hwnd, GWL_EXSTYLE, wsex.0 as i32);

                let mut rect = RECT {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                };
                AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex).expect("hotline_rs::win32::error: call to AdjustWindowRectEx failed");

                SetWindowPos(
                    hwnd,
                    insert_after,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    swp_flag | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                ).expect("hotline_rs::win32::error: call to SetWindowPos failed");

                let _ = ShowWindow(hwnd, SW_SHOWNA);
                self.events |= super::WindowEventFlags::MOVE;
                self.events |= super::WindowEventFlags::SIZE;
            }
        }
    }

    fn is_focused(&self) -> bool {
        unsafe { GetForegroundWindow() == as_hwnd(self.hwnd) }
    }

    fn is_minimised(&self) -> bool {
        unsafe { IsIconic(as_hwnd(self.hwnd)) == true }
    }

    fn set_focused(&self) {
        unsafe {
            let hwnd = as_hwnd(self.hwnd);
            BringWindowToTop(hwnd).expect("hotline_rs::win32::error: call to BringWindowToTop failed");
            let _ = SetForegroundWindow(hwnd);
            SetFocus(hwnd).expect("hotline_rs::win32::error: call to SetFocus failed");
        }
    }

    fn is_mouse_hovered(&self) -> bool {
        unsafe {
            let mut mouse_pos = POINT::default();
            GetCursorPos(&mut mouse_pos).expect("hotline_rs::win32::error: call to GetCursorPos failed");
            as_hwnd(self.hwnd) == WindowFromPoint(mouse_pos)
        }
    }

    fn set_title(&self, title: String) {
        unsafe {
            let mb = string_to_wide(title);
            SetWindowTextW(as_hwnd(self.hwnd), PCWSTR(mb.as_ptr() as _)).expect("hotline_rs::win32::error: call to SetWindowTextW failed");
        }
    }

    fn set_pos(&self, pos: super::Point<i32>) {
        let mut rect = RECT {
            left: pos.x,
            top: pos.y,
            right: pos.x,
            bottom: pos.y,
        };
        unsafe {
            AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex).expect("hotline_rs::win32::error: call to AdjustWindowRectEx failed");
            SetWindowPos(
                as_hwnd(self.hwnd),
                HWND::default(),
                rect.left,
                rect.top,
                0,
                0,
                SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE,
            ).expect("hotline_rs::win32::error: call to SetWindowPos failed");
        }
    }

    fn set_size(&self, size: super::Point<i32>) {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: size.x,
            bottom: size.y,
        };
        unsafe {
            AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex).expect("hotline_rs::win32::error: call to AdjustWindowRectEx failed");
            SetWindowPos(
                as_hwnd(self.hwnd),
                HWND::default(),
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOMOVE | SWP_NOACTIVATE,
            ).expect("hotline_rs::win32::error: call to SetWindowPos failed");
        }
    }

    fn get_pos(&self) -> super::Point<i32> {
        unsafe {
            let mut pos = POINT { x: 0, y: 0 };
            let _ = ClientToScreen(as_hwnd(self.hwnd), &mut pos);
            super::Point { x: pos.x, y: pos.y }
        }
    }

    fn get_size(&self) -> super::Size<i32> {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(as_hwnd(self.hwnd), &mut rect).expect("hotline_rs::win32::error: call to GetClientRect failed");
            super::Size {
                x: rect.right - rect.left,
                y: rect.bottom - rect.top,
            }
        }
    }

    fn get_viewport_rect(&self) -> super::Rect<i32> {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(as_hwnd(self.hwnd), &mut rect).expect("hotline_rs::win32::error: call to GetClientRect failed");
            super::Rect::<i32> {
                x: 0,
                y: 0,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            }
        }
    }

    fn get_window_rect(&self) -> super::Rect<i32> {
        unsafe {
            let mut rect = RECT::default();
            GetWindowRect(as_hwnd(self.hwnd), &mut rect).expect("hotline_rs::win32::error: call to GetWindowRect failed");

            // this is adjusted to compensate for AdjustWindowRectEx on create
            let mut adj = RECT::default();
            AdjustWindowRectEx(&mut adj, self.ws, BOOL::from(false), self.wsex).expect("hotline_rs::win32::error: call to AdjustWindowRectEx failed");
            let adj_rect = RECT {
                left: rect.left - adj.left,
                right: rect.right - adj.right,
                top: rect.top - adj.top,
                bottom: rect.bottom - adj.bottom,
            };

            super::Rect::<i32> {
                x: adj_rect.left,
                y: adj_rect.top,
                width: adj_rect.right - adj_rect.left,
                height: adj_rect.bottom - adj_rect.top,
            }
        }
    }

    fn get_mouse_client_pos(&self, mouse_pos: super::Point<i32>) -> super::Point<i32> {
        unsafe {
            let mut mp = POINT {
                x: mouse_pos.x,
                y: mouse_pos.y,
            };
            ScreenToClient(as_hwnd(self.hwnd), &mut mp).expect("hotline_rs::win32::error: call to ScreenToClient failed");
            super::Point { x: mp.x, y: mp.y }
        }
    }

    fn get_dpi_scale(&self) -> f32 {
        unsafe {
            let monitor = MonitorFromWindow(as_hwnd(self.hwnd), MONITOR_DEFAULTTONEAREST);
            let mut xdpi: u32 = 0;
            let mut ydpi: u32 = 0;
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut xdpi, &mut ydpi).is_err() {
                println!("hotline_rs::os::win32: GetDpiForMonitor failed");
                return 1.0;
            }
            (xdpi as f32) / 96.0
        }
    }

    fn get_native_handle(&self) -> NativeHandle {
        NativeHandle { hwnd: self.hwnd }
    }

    fn get_events(&self) -> super::WindowEventFlags {
        self.events
    }

    fn clear_events(&mut self) {
        self.events = super::WindowEventFlags::NONE
    }

    fn as_ptr(&self) -> *const Self {
        self as *const Self
    }

    fn as_mut_ptr(&mut self) -> *mut Self {
        self as *mut Self
    }
}

/*
TODO: wndproc
WM_SETFOCUS => LRESULT(0),
WM_KILLFOCUS => LRESULT(0),
WM_DEVICECHANGE => LRESULT(0),
WM_DISPLAYCHANGE => LRESULT(0),
*/

extern "system" fn main_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_DESTROY => {
            unsafe {
                // PostQuitMessage must happen here, not in the member wnd proc function
                println!("hotline_rs::os::win32:: WM_DESTROY with code ({})", 0);
                PostQuitMessage(0);
                LRESULT(0)
            }
        }
        WM_SYSCOMMAND => {
            if (wparam.0 & 0xfff0) == SC_KEYMENU as usize {
                // Disable ALT application menu
                LRESULT(0)
            } else {
                wndproc(window, message, wparam, lparam)
            }
        }
        _ => wndproc(window, message, wparam, lparam),
    }
}

extern "system" fn imgui_wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_CLOSE | WM_MOVE | WM_SIZE | WM_MOUSEACTIVATE => {
            LRESULT(0)
        }
        _ => wndproc(window, message, wparam, lparam),
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_MOUSEMOVE => {
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                LRESULT(0)
            }
            WM_NCLBUTTONDOWN | WM_NCLBUTTONUP => {
                LRESULT(0)
            }
            WM_NCMBUTTONDOWN | WM_NCMBUTTONUP => {
                LRESULT(0)
            }
            WM_NCRBUTTONDOWN | WM_NCRBUTTONUP => {
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                LRESULT(0)
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                LRESULT(0)
            }
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                LRESULT(0)
            }
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                LRESULT(0)
            }
            WM_MBUTTONUP => {
                LRESULT(0)
            }
            WM_XBUTTONUP => {
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                LRESULT(0)
            }
            WM_MOUSEHWHEEL => {
                LRESULT(0)
            }
            WM_PAINT => {
                LRESULT(0)
            }
            WM_CHAR => {
                LRESULT(0)
            }
            WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
                LRESULT(0)
            }
            WM_SIZE => {
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}

static mut MONITOR_ENUM: Vec<super::MonitorInfo> = Vec::new();

extern "system" fn enum_func(
    monitor: HMONITOR,
    _hdc: HDC,
    _lprect: *mut RECT,
    _lparam: LPARAM,
) -> BOOL {
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoA(monitor, &mut info) == false {
            return BOOL::from(false);
        }

        // get dpi from monitor
        let mut xdpi: u32 = 0;
        let mut ydpi: u32 = 0;
        let dpi_scale =
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut xdpi, &mut ydpi).is_ok() {
                (xdpi as f32) / 96.0
            }
            else {
                1.0
            };

        static_ref_mut!(MONITOR_ENUM).push(super::MonitorInfo {
            rect: super::Rect {
                x: info.rcMonitor.left,
                y: info.rcMonitor.top,
                width: info.rcMonitor.right - info.rcMonitor.left,
                height: info.rcMonitor.bottom - info.rcMonitor.top,
            },
            client_rect: super::Rect {
                x: info.rcWork.left,
                y: info.rcWork.top,
                width: info.rcWork.right - info.rcWork.left,
                height: info.rcWork.bottom - info.rcWork.top,
            },
            dpi_scale,
            primary: (info.dwFlags & MONITORINFOF_PRIMARY) != 0,
        });
        BOOL::from(true)
    }
}
