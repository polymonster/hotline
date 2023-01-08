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
    Win32::UI::Shell::*, Win32::UI::Shell::Common::COMDLG_FILTERSPEC
};

use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::result;

use std::collections::HashMap;
use std::ffi::CString;

#[derive(Clone)]
pub struct App {
    window_class: String,
    window_class_imgui: String,
    hinstance: HINSTANCE,
    mouse_pos: super::Point<i32>,
    mouse_pos_delta: super::Point<i32>,
    proc_data: ProcData,
    events: HashMap<isize, super::WindowEventFlags>,
    hwnd_flags: HashMap<isize, super::WindowStyleFlags>
}

#[derive(Clone)]
pub struct Window {
    hwnd: HWND,
    ws: WINDOW_STYLE,
    wsex: WINDOW_EX_STYLE,
    events: super::WindowEventFlags,
}

unsafe impl Send for Window {}
unsafe impl Sync for Window {}

#[derive(Clone, Copy)]
pub struct NativeHandle {
    pub hwnd: HWND,
}

#[derive(Clone)]
struct ProcData {
    mouse_hwnd: HWND,
    mouse_tracked: bool,
    mouse_down: [bool; super::MouseButton::Count as usize],
    mouse_wheel: f32,
    mouse_hwheel: f32,
    utf16_inputs: Vec<u16>,
    key_down: [bool; 256],
    key_ctrl: bool,
    key_shift: bool,
    key_alt: bool,
}

impl super::NativeHandle<App> for NativeHandle {
    fn get_isize(&self) -> isize {
        self.hwnd.0
    }
    fn copy(&self) -> NativeHandle {
        *self
    }
}

impl Window {
    pub fn get_hwnd(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            if GetCapture() == self.hwnd {
                ReleaseCapture();
            }
            DestroyWindow(self.hwnd);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            UnregisterClassA(PCSTR(self.window_class.as_ptr() as _), self.hinstance);
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
        AdjustWindowRectEx(&mut rc, ws, BOOL::from(false), wsex);
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
            vx.as_mut_slice(),
        );
        let mut v: Vec<u16> = vec![0; n as usize];
        MultiByteToWideChar(
            windows::Win32::Globalization::CP_UTF8,
            windows::Win32::Globalization::MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
            null_string.as_bytes(),
            v.as_mut_slice(),
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
            GetCursorPos(&mut mouse_pos);
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
    }

    fn wndproc(&mut self, window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        unsafe {
            let mut proc_data = &mut self.proc_data;
            match message {
                WM_MOUSEMOVE => {
                    proc_data.mouse_hwnd = window;
                    if !proc_data.mouse_tracked {
                        // We need to call TrackMouseEvent in order to receive WM_MOUSELEAVE events
                        TrackMouseEvent(&mut TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: window,
                            dwHoverTime: 0,
                        });
                        proc_data.mouse_tracked = true;
                    }
                    LRESULT(0)
                }
                WM_MOUSELEAVE => {
                    proc_data.mouse_hwnd = HWND(0);
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
                    ValidateRect(window, std::ptr::null());
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
                            proc_data.key_ctrl = down;
                        }
                        VK_SHIFT => {
                            proc_data.key_shift = down;
                        }
                        VK_MENU => {
                            proc_data.key_alt = down;
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
        if let Some(window_events) = self.events.get_mut(&window.0) {
            // or into existsing key
            *window_events |= flags;
        } else {
            // create new key
            self.events.insert(window.0, flags);
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
            if !any_down && GetCapture() == HWND(0) {
                SetCapture(window);
            }
        }
    }
    
    fn release_capture(&mut self, window: HWND) {
        unsafe {
            let any_down = self.proc_data.mouse_down.iter().any(|v| v == &true);
            if !any_down && GetCapture() == window {
                ReleaseCapture();
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
            CoInitialize(std::ptr::null_mut()).unwrap();

            let window_class = info.name.to_string() + "\0";
            let window_class_imgui = info.name.to_string() + "_imgui\0";

            let instance = GetModuleHandleA(None);
            debug_assert!(instance.0 != 0);

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
                hinstance: instance,
                mouse_pos: super::Point::default(),
                mouse_pos_delta: super::Point::default(),
                proc_data: ProcData {
                    mouse_hwnd: HWND(0),
                    mouse_tracked: false,
                    mouse_down: [false; 5],
                    mouse_wheel: 0.0,
                    mouse_hwheel: 0.0,
                    utf16_inputs: Vec::new(),
                    key_down: [false; 256],
                    key_ctrl: false,
                    key_shift: false,
                    key_alt: false,
                },
                events: HashMap::new(),
                hwnd_flags: HashMap::new()
            }
        }
    }

    fn create_window(&mut self, info: super::WindowInfo<Self>) -> Window {
        unsafe {
            let ws = to_win32_dw_style(&info.style);
            let wsex = to_win32_dw_ex_style(&info.style);
            let rect = adjust_window_rect(&info.rect, ws, wsex);

            let parent_hwnd = if info.parent_handle.is_some() {
                Some(info.parent_handle.unwrap().hwnd)
            } else {
                None
            };

            let class = if info.style.contains(super::WindowStyleFlags::IMGUI) {
                self.window_class_imgui.clone()
            } else {
                self.window_class.clone()
            };

            let hwnd = CreateWindowExA(
                wsex,
                class,
                info.title.clone(),
                ws,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                parent_hwnd,
                None,
                self.hinstance,
                std::ptr::null_mut(),
            );

            // track window style to send to correct wnd proc
            self.hwnd_flags.insert(hwnd.0, info.style);

            Window {
                hwnd,
                ws,
                wsex,
                events: super::WindowEventFlags::NONE,
            }
        }
    }

    fn destroy_window(&mut self, window: &Window) {
        self.hwnd_flags.remove(&window.hwnd.0);
    }

    fn run(&mut self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut quit = false;
            
            self.update_input();
            loop {
                if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE).into() {
                    TranslateMessage(&msg);
                    DispatchMessageA(&msg);
                    
                    // handle wnd proc on self functions, to avoid need for static mutable state
                    if let Some(hwnd_flags) = self.hwnd_flags.get(&msg.hwnd.0) {
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
        match key {
            super::SysKey::Ctrl => self.proc_data.key_ctrl,
            super::SysKey::Shift => self.proc_data.key_shift,
            super::SysKey::Alt => self.proc_data.key_alt,
        }
    }

    fn get_key_code(key: super::Key) -> i32 {
        to_win32_key_code(key)
    }

    fn enumerate_display_monitors() -> Vec<super::MonitorInfo> {
        unsafe {
            MONITOR_ENUM.clear();
            EnumDisplayMonitors(HDC(0), std::ptr::null_mut(), Some(enum_func), LPARAM(0));
            let mut monitors: Vec<super::MonitorInfo> = Vec::new();
            for m in &MONITOR_ENUM {
                monitors.push(m.clone());
            }
            monitors
        }
    }

    fn set_cursor(&self, cursor: &super::Cursor) {
        unsafe {
            match cursor {
                super::Cursor::None => SetCursor(HCURSOR(0)),
                super::Cursor::Arrow => SetCursor(LoadCursorW(self.hinstance, IDC_ARROW).unwrap()),
                super::Cursor::TextInput => SetCursor(LoadCursorW(self.hinstance, IDC_IBEAM).unwrap()),
                super::Cursor::ResizeAll => SetCursor(LoadCursorW(self.hinstance, IDC_SIZEALL).unwrap()),
                super::Cursor::ResizeEW => SetCursor(LoadCursorW(self.hinstance, IDC_SIZEWE).unwrap()),
                super::Cursor::ResizeNS => SetCursor(LoadCursorW(self.hinstance, IDC_SIZENS).unwrap()),
                super::Cursor::ResizeNESW => SetCursor(LoadCursorW(self.hinstance, IDC_SIZENESW).unwrap()),
                super::Cursor::ResizeNWSE => SetCursor(LoadCursorW(self.hinstance, IDC_SIZENWSE).unwrap()),
                super::Cursor::Hand => SetCursor(LoadCursorW(self.hinstance, IDC_HAND).unwrap()),
                super::Cursor::NotAllowed => SetCursor(LoadCursorW(self.hinstance, IDC_NO).unwrap()),
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
            }
            
            open_dialog.SetFileTypes(&specs)?;
            open_dialog.Show(HWND(0))?;
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
}

impl super::Window<App> for Window {
    fn bring_to_front(&self) {
        unsafe {
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
            SetActiveWindow(self.hwnd);
            BringWindowToTop(self.hwnd);
            ShowWindow(self.hwnd, SW_RESTORE);
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
            ShowWindow(self.hwnd, cmd);
        }
    }

    fn close(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }

    fn update(&mut self, app: &mut App) {
        // take events
        if let Some(window_events) = app.events.get_mut(&self.hwnd.0) {
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
                SetWindowLongA(self.hwnd, GWL_STYLE, ws.0 as i32);
                SetWindowLongA(self.hwnd, GWL_EXSTYLE, wsex.0 as i32);

                let mut rect = RECT {
                    left: rect.x,
                    top: rect.y,
                    right: rect.x + rect.width,
                    bottom: rect.y + rect.height,
                };
                AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex);

                SetWindowPos(
                    self.hwnd,
                    insert_after,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    swp_flag | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );

                ShowWindow(self.hwnd, SW_SHOWNA);
                self.events |= super::WindowEventFlags::MOVE;
                self.events |= super::WindowEventFlags::SIZE;
            }
        }
    }

    fn is_focused(&self) -> bool {
        unsafe { GetForegroundWindow() == self.hwnd }
    }

    fn is_minimised(&self) -> bool {
        unsafe { IsIconic(self.hwnd) == true }
    }

    fn set_focused(&self) {
        unsafe {
            BringWindowToTop(self.hwnd);
            SetForegroundWindow(self.hwnd);
            SetFocus(self.hwnd);
        }
    }

    fn is_mouse_hovered(&self) -> bool {
        unsafe {
            let mut mouse_pos = POINT::default();
            GetCursorPos(&mut mouse_pos);
            self.hwnd == WindowFromPoint(mouse_pos)
        }
    }

    fn set_title(&self, title: String) {
        unsafe {
            let mb = string_to_wide(title);
            SetWindowTextW(self.hwnd, PCWSTR(mb.as_ptr() as _));
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
            AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex);
            SetWindowPos(
                self.hwnd,
                HWND(0),
                rect.left,
                rect.top,
                0,
                0,
                SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE,
            );
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
            AdjustWindowRectEx(&mut rect, self.ws, BOOL::from(false), self.wsex);
            SetWindowPos(
                self.hwnd,
                HWND(0),
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOZORDER | SWP_NOMOVE | SWP_NOACTIVATE,
            );
        }
    }

    fn get_pos(&self) -> super::Point<i32> {
        unsafe {
            let mut pos = POINT { x: 0, y: 0 };
            ClientToScreen(self.hwnd, &mut pos);
            super::Point { x: pos.x, y: pos.y }
        }
    }

    fn get_size(&self) -> super::Size<i32> {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect);
            super::Size {
                x: rect.right - rect.left,
                y: rect.bottom - rect.top,
            }
        }
    }

    fn get_viewport_rect(&self) -> super::Rect<i32> {
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(self.hwnd, &mut rect);
            super::Rect::<i32> {
                x: 0,
                y: 0,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            }
        }
    }

    fn get_mouse_client_pos(&self, mouse_pos: super::Point<i32>) -> super::Point<i32> {
        unsafe {
            let mut mp = POINT {
                x: mouse_pos.x,
                y: mouse_pos.y,
            };
            ScreenToClient(self.hwnd, &mut mp);
            super::Point { x: mp.x, y: mp.y }
        }
    }

    fn get_dpi_scale(&self) -> f32 {
        unsafe {
            let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
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

        MONITOR_ENUM.push(super::MonitorInfo {
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
