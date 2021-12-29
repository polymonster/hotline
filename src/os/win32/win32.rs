use windows::{
    Win32::Foundation::*, 
    Win32::System::LibraryLoader::*, 
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::ValidateRect
};

pub struct Instance {
    window_class: String,
    hinstance: HINSTANCE
}

pub struct Window {
    info: os::WindowInfo,
    hwnd : HWND
}

impl Window {
    pub fn get_native_handle(&self) -> HWND {
        self.hwnd
    }
}

impl os::Instance<Platform> for Instance {
    fn create() -> Self {
        unsafe {
            let window_class = "window";
            let instance = GetModuleHandleA(None);
            debug_assert!(instance.0 != 0);
    
            let wc = WNDCLASSA {
                hCursor: LoadCursorW(None, IDC_ARROW),
                hInstance: instance,
                lpszClassName: PSTR(b"window\0".as_ptr() as _),
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                ..Default::default()
            };
    
            let atom = RegisterClassA(&wc);
            debug_assert!(atom != 0);
    
            Instance {
                window_class: String::from(window_class),
                hinstance: instance
            }
        }
    }
    fn create_window(&self, info: os::WindowInfo) -> Window {
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
            println!("creating window {}", self.window_class);
            Window {
                hwnd: hwnd,
                info: info
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
                }
                else
                {
                    break;
                }
            }
            !quit
        }
    }
}

impl os::Window<Platform> for Window {
    fn set_rect(&mut self, rect : os::Rect<i32>) {
        unsafe {
            SetWindowPos(self.hwnd, HWND(0), rect.x, rect.y, rect.width, rect.height, SWP_ASYNCWINDOWPOS);
        }
        self.info.rect = rect;
    }

    fn get_rect(&self) -> os::Rect<i32> {
        self.info.rect
    }

    fn set_size(&mut self, width : i32, height : i32) {
        let mut rect = self.info.rect;
        rect.width = width;
        rect.height = height;
        unsafe {
            SetWindowPos(self.hwnd, HWND(0), rect.x, rect.y, rect.width, rect.height, SWP_ASYNCWINDOWPOS);
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
}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message as u32 {
            WM_PAINT => {
                println!("WM_PAINT");
                ValidateRect(window, std::ptr::null());
                LRESULT(0)
            }
            WM_DESTROY => {
                println!("WM_DESTROY");
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}

pub enum Platform {}
impl os::Platform for Platform {
    type Instance = Instance;
    type Window = Window;
}