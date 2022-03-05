use windows::{
    Win32::Foundation::*, 
    Win32::Graphics::Gdi::ValidateRect, 
    Win32::Graphics::Gdi::ScreenToClient,
    Win32::Graphics::Gdi::EnumDisplayMonitors,
    Win32::Graphics::Gdi::HDC,
    Win32::Graphics::Gdi::HMONITOR,
    Win32::Graphics::Gdi::MONITORINFO,
    Win32::Graphics::Gdi::GetMonitorInfoA,
    Win32::System::LibraryLoader::*,
    Win32::UI::Input::KeyboardAndMouse::*, 
    Win32::UI::WindowsAndMessaging::*,
    Win32::UI::Controls::*,
};

pub struct App {
    window_class: String,
    hinstance: HINSTANCE,
    mouse_pos: super::Point<i32>
}

#[derive(Clone)]
pub struct Window {
    info: super::WindowInfo,
    hwnd: HWND,
}

impl Window {
    pub fn get_native_handle(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
            self.hwnd = HWND(0);
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe {
            UnregisterClassA(PSTR(self.window_class.as_ptr() as _), self.hinstance);
        }
    }
}

fn adjust_window_rect(rect: &super::Rect::<i32>) -> super::Rect::<i32>{
    let mut rc = RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height
    };
    unsafe {
        AdjustWindowRect(&mut rc, WS_OVERLAPPEDWINDOW, BOOL::from(false));
    }
    super::Rect::<i32> {
        x: rc.left,
        y: rc.top,
        width: rc.right - rc.left,
        height: rc.bottom - rc.top,
    }
}

struct ProcData {
    mouse_hwnd: HWND,
    mouse_tracked: bool,
    mouse_down: [bool; 5],
    mouse_wheel: f32,
    mouse_hwheel: f32
}

static mut PROC_DATA : ProcData = ProcData {
    mouse_hwnd: HWND(0),
    mouse_tracked: false,
    mouse_down: [false; 5],
    mouse_wheel: 0.0,
    mouse_hwheel: 0.0
};

static mut MONITOR_ENUM : Vec<super::MonitorInfo> = Vec::new();

impl App {
    fn update_mouse(&mut self) {
        unsafe {
            let mut mouse_pos = POINT::default();
            GetCursorPos(&mut mouse_pos);
            self.mouse_pos = super::Point {
                x: mouse_pos.x,
                y: mouse_pos.y
            }
        }
    }
}

impl super::App for App {
    type Window = Window;

    fn create(info: super::AppInfo) -> Self {
        unsafe {
            let window_class = info.name + "\0";
            let instance = GetModuleHandleA(None);
            debug_assert!(instance.0 != 0);

            let wc = WNDCLASSA {
                hCursor: LoadCursorW(None, IDC_ARROW),
                hInstance: instance,
                lpszClassName: PSTR(window_class.as_ptr() as _),
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                ..Default::default()
            };

            if RegisterClassA(&wc) == 0 {
                panic!("hotline::os::win32: class already registered!");
            }

            App {
                window_class: String::from(window_class),
                hinstance: instance,
                mouse_pos: super::Point::default()
            }
        }
    }

    fn create_window(&self, info: super::WindowInfo) -> Window {
        unsafe {
            let rect = adjust_window_rect(&info.rect);
            let hwnd = CreateWindowExA(
                Default::default(),
                self.window_class.clone(),
                info.title.clone(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                None,
                None,
                self.hinstance,
                std::ptr::null_mut(),
            );
            Window {
                hwnd: hwnd,
                info: info,
            }
        }
    }

    fn run(&mut self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut quit = false;
            self.update_mouse();
            loop {
                if PeekMessageA(&mut msg, None, 0, 0, PM_REMOVE).into() {
                    TranslateMessage(&mut msg);
                    DispatchMessageA(&mut msg);
                    if msg.message == WM_QUIT {
                        quit = true;
                        break;
                    }
                } else {
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
        unsafe {
            PROC_DATA.mouse_wheel
        }
    }

    fn get_mouse_hwheel(&self) -> f32 {
        unsafe {
            PROC_DATA.mouse_hwheel
        }
    }

    fn get_mouse_buttons(&self) -> [bool; super::MouseButton::Count as usize] {
        unsafe {
            PROC_DATA.mouse_down
        }
    }

    fn enumerate_display_monitors() -> Vec<super::MonitorInfo> {
        unsafe {
            MONITOR_ENUM.clear();
            EnumDisplayMonitors(HDC(0), std::ptr::null_mut(), Some(enum_func), LPARAM(0));
            let mut monitors : Vec<super::MonitorInfo> = Vec::new();
            for m in &MONITOR_ENUM {
                monitors.push(m.clone());
            }
            monitors
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

    fn set_rect(&mut self, rect: super::Rect<i32>) {
        unsafe {
            let rect = adjust_window_rect(&rect);
            SetWindowPos(
                self.hwnd,
                HWND(0),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_ASYNCWINDOWPOS,
            );
        }
        self.info.rect = rect;
    }

    fn get_rect(&self) -> super::Rect<i32> {
        self.info.rect
    }

    fn get_viewport_rect(&self) -> super::Rect<i32> {
        super::Rect::<i32> {
            x: 0,
            y: 0,
            width: self.info.rect.width,
            height: self.info.rect.height,
        }
    }

    fn get_mouse_client_pos(&self, mouse_pos: &super::Point<i32>) -> super::Point<i32> {
        unsafe {
            let mut mp = POINT {
                x: mouse_pos.x,
                y: mouse_pos.y
            };
            ScreenToClient(self.hwnd, &mut mp);
            super::Point {
                x: mp.x,
                y: mp.y
            }
        }
    }

    fn set_size(&mut self, width: i32, height: i32) {
        let mut rect = self.info.rect;
        rect.width = width;
        rect.height = height;
        unsafe {
            let rect = adjust_window_rect(&rect);
            SetWindowPos(
                self.hwnd,
                HWND(0),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                SWP_ASYNCWINDOWPOS,
            );
        }
        self.info.rect = rect;
    }

    fn get_size(&self) -> (i32, i32) {
        (self.info.rect.width, self.info.rect.height)
    }

    fn close(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }

    fn update(&mut self) {
        unsafe {
            let mut win_rect = RECT::default();
            GetClientRect(self.hwnd, &mut win_rect);
            self.info.rect.width = win_rect.right - win_rect.left;
            self.info.rect.height = win_rect.bottom - win_rect.top;
            self.info.rect.x = win_rect.left;
            self.info.rect.y = win_rect.top;
        }
    }

    fn as_ptr(&self) -> *const Self {
        unsafe {
            std::mem::transmute(self)
        }
    }

    fn as_mut_ptr(&mut self) -> *mut Self {
        unsafe {
            std::mem::transmute(self)
        }
    }
}

fn set_capture(window: HWND) {
    unsafe {
        let any_down = PROC_DATA.mouse_down.iter().any(|v| v == &true);
        if !any_down && GetCapture() == HWND(0){
            SetCapture(window);
        }
    }   
}

fn release_capture(window: HWND) {
    unsafe {
        let any_down = PROC_DATA.mouse_down.iter().any(|v| v == &true);
        if !any_down && GetCapture() == window {
            ReleaseCapture();
        }
    }   
}


/*
TODO: wndproc
WM_KEYDOWN => LRESULT(0),
WM_KEYUP => LRESULT(0),
WM_SYSKEYDOWN => LRESULT(0),
WM_SYSKEYUP => LRESULT(0),
WM_SETFOCUS => LRESULT(0),
WM_KILLFOCUS => LRESULT(0),
WM_CHAR => LRESULT(0),
WM_SETCURSOR => LRESULT(0),
WM_DEVICECHANGE => LRESULT(0),
WM_DISPLAYCHANGE => LRESULT(0),
*/


extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as u32 {
            WM_MOUSEMOVE => {
                PROC_DATA.mouse_hwnd = window;
                if !PROC_DATA.mouse_tracked {
                    // We need to call TrackMouseEvent in order to receive WM_MOUSELEAVE events 
                    TrackMouseEvent(&mut TRACKMOUSEEVENT{
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: window,
                        dwHoverTime: 0
                    });
                    PROC_DATA.mouse_tracked = true;
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                PROC_DATA.mouse_hwnd = HWND(0);
                PROC_DATA.mouse_tracked = false;
                LRESULT(0)
            }
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
                PROC_DATA.mouse_down[0] = true;
                set_capture(window);
                LRESULT(0)
            }
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
                PROC_DATA.mouse_down[1] = true;
                set_capture(window);
                LRESULT(0)
            }
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                PROC_DATA.mouse_down[2] = true;
                set_capture(window);
                LRESULT(0)
            }
            WM_XBUTTONDOWN | WM_XBUTTONDBLCLK => {
                let button = ((wparam.0 >> 16) & 0xffff) + 1;
                PROC_DATA.mouse_down[button] = true;
                set_capture(window);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                PROC_DATA.mouse_down[0] = false;
                release_capture(window);
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                PROC_DATA.mouse_down[1] = false;
                release_capture(window);
                LRESULT(0)
            }
            WM_MBUTTONUP => {
                PROC_DATA.mouse_down[2] = false;
                release_capture(window);
                LRESULT(0)
            }
            WM_XBUTTONUP => {
                let button = ((wparam.0 >> 16) & 0xffff) + 1;
                PROC_DATA.mouse_down[button] = true;
                release_capture(window);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                PROC_DATA.mouse_wheel += ((wparam.0 >> 16) & 0xffff) as f32 / WHEEL_DELTA as f32;
                LRESULT(0)
            }
            WM_MOUSEHWHEEL => {
                PROC_DATA.mouse_hwheel += ((wparam.0 >> 16) & 0xffff) as f32 / WHEEL_DELTA as f32;
                LRESULT(0)
            }
            WM_PAINT => {
                ValidateRect(window, std::ptr::null());
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}

extern "system" fn enum_func(monitor: HMONITOR, _hdc: HDC, _lprect: *mut RECT, _lparam: LPARAM) -> BOOL {
    unsafe {
        let mut info : MONITORINFO = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoA(monitor, &mut info) == BOOL::from(false) {
            return BOOL::from(false);
        }
        MONITOR_ENUM.push(super::MonitorInfo {
            rect: super::Rect {
                x: info.rcMonitor.left,
                y: info.rcMonitor.top,
                width: info.rcMonitor.right - info.rcMonitor.left,
                height: info.rcMonitor.bottom - info.rcMonitor.top
            },
            client_rect: super::Rect {
                x: info.rcWork.left,
                y: info.rcWork.top,
                width: info.rcWork.right - info.rcWork.left,
                height: info.rcWork.bottom - info.rcWork.top
            },
            dpi_scale: 1.0 // TODO:
        });
        BOOL::from(true)
    }
}