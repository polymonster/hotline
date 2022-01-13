use windows::{
    Win32::Foundation::*, Win32::Graphics::Gdi::ValidateRect, Win32::System::LibraryLoader::*,
    Win32::UI::Input::KeyboardAndMouse, Win32::UI::WindowsAndMessaging::*,
};

pub struct App {
    window_class: String,
    hinstance: HINSTANCE,
    atom: u16,
}

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

impl super::App for App {
    // types
    type Window = Window;

    fn create() -> Self {
        unsafe {
            let window_class = "window\0";
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

            let atom = RegisterClassA(&wc);
            if atom == 0 {
                println!("win32 class {} already registered", window_class);
            }

            App {
                window_class: String::from(window_class),
                hinstance: instance,
                atom: atom,
            }
        }
    }

    fn create_window(&self, info: super::WindowInfo) -> Window {
        unsafe {
            let hwnd = CreateWindowExA(
                Default::default(),
                self.window_class.clone(),
                info.title.clone(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                info.rect.x,
                info.rect.y,
                info.rect.width,
                info.rect.height,
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

    fn run(&self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut quit = false;
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
}

impl super::Window<App> for Window {
    fn bring_to_front(&self) {
        unsafe {
            SetForegroundWindow(self.hwnd);
            KeyboardAndMouse::SetFocus(self.hwnd);
            KeyboardAndMouse::SetActiveWindow(self.hwnd);
            BringWindowToTop(self.hwnd);
            ShowWindow(self.hwnd, SW_RESTORE);
        }
    }

    fn set_rect(&mut self, rect: super::Rect<i32>) {
        unsafe {
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

    fn set_size(&mut self, width: i32, height: i32) {
        let mut rect = self.info.rect;
        rect.width = width;
        rect.height = height;
        unsafe {
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
            GetWindowRect(self.hwnd, &mut win_rect);
            self.info.rect.width = win_rect.right - win_rect.left;
            self.info.rect.height = win_rect.bottom - win_rect.top;
            self.info.rect.x = win_rect.left;
            self.info.rect.y = win_rect.top;
        }
    }
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as u32 {
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
