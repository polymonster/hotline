use imgui_sys::*;

#[cfg(target_os = "windows")]
use crate::os::win32 as platform;

#[derive(Clone)]
struct ImGuiPlatform {
    window: *mut platform::Window,
    mouse_window: *mut platform::Window,
    time: u64,
    ticks_per_second: u64,
    last_mouse_cursor: ImGuiMouseCursor,
    mouse_tracked: bool,
    has_gamepad: bool,
    want_update_has_gamepad: bool,
    want_update_monitors: bool 
}

pub struct ImGuiInfo {
    pub main_window: *mut platform::Window,
}

static mut platform_data : ImGuiPlatform = ImGuiPlatform {
    window: std::ptr::null_mut(),
    mouse_window: std::ptr::null_mut(),
    time: 0,
    ticks_per_second: 0,
    last_mouse_cursor: ImGuiMouseCursor_Arrow,
    mouse_tracked: false,
    has_gamepad: false,
    want_update_has_gamepad: false,
    want_update_monitors: false 
};

pub fn setup_platform(info: &ImGuiInfo) {
    unsafe {
        let mut io = *imgui_sys::igGetIO();
        
        // io setup
        io.BackendPlatformUserData = std::mem::transmute(&platform_data.clone());
        io.BackendPlatformName = "imgui_impl_hotline".as_ptr() as *const i8;
        io.BackendFlags |= ImGuiBackendFlags_HasMouseCursors as i32;
        io.BackendFlags |= ImGuiBackendFlags_HasSetMousePos as i32;
        io.BackendFlags |= ImGuiBackendFlags_PlatformHasViewports as i32;
        io.BackendFlags |= ImGuiBackendFlags_HasMouseHoveredViewport as i32;

        // platform backend setup
        platform_data = ImGuiPlatform {
            window: std::mem::transmute(info.main_window),
            want_update_has_gamepad: true,
            want_update_monitors: true,
            ticks_per_second: 0, // TODO:
            time: 0, // TODO:
            last_mouse_cursor: ImGuiMouseCursor_COUNT,
            ..Default::default()
        }

        // TODO: mouse update fun

        // TODO: keyboard mappings

        // TODO: gamepads
    }
}

pub fn setup(info: &ImGuiInfo) {
    unsafe {
        igCreateContext(std::ptr::null_mut());
        let mut io = *igGetIO();

        io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard as i32;
        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable as i32;
        io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable as i32;

        igStyleColorsLight(std::ptr::null_mut());

        let mut style = *igGetStyle();
        style.WindowRounding = 0.0; 
        style.Colors[imgui_sys::ImGuiCol_WindowBg as usize].w = 1.0;

        setup_platform(info);
    }
}

impl Default for ImGuiPlatform {
    fn default() -> Self { 
        ImGuiPlatform {
            window: std::ptr::null_mut(),
            mouse_window: std::ptr::null_mut(),
            time: 0,
            ticks_per_second: 0,
            last_mouse_cursor: ImGuiMouseCursor_Arrow,
            mouse_tracked: false,
            has_gamepad: false,
            want_update_has_gamepad: false,
            want_update_monitors: false 
        }
    }
}