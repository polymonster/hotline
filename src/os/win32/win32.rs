use windows::{
    core::*, Win32::Foundation::*, 
    Win32::Graphics::Direct3D::Fxc::*, 
    Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, 
    Win32::Graphics::Dxgi::Common::*, 
    Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, 
    Win32::System::Threading::*,
    Win32::System::WindowsProgramming::*, 
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::ValidateRect
};

pub struct Instance {
    window_class: String,
    wc: WNDCLASSA,
    hinstance: HINSTANCE,
}

pub struct Window {
    hwnd : HWND
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
                wc: wc,
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
            Window {
                hwnd: hwnd
            }
        }
    }
    fn run(&self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            let mut quit = false;
            loop {
                if PeekMessageA(&mut msg, HWND(0), 0, 0, PM_REMOVE).into() {
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
    fn set_rect(&self, rect : os::Rect<i32>) {
        println!("setting rect on win32 window {} {} {} {}", rect.x, rect.y, rect.width, rect.height);
    }

    fn resize(&self, width : i32, height : i32) {

    }

    fn close(&self) {

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