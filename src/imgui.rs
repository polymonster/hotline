use imgui_sys::*;

#[cfg(target_os = "windows")]
use crate::os::win32 as os_platform;
use crate::gfx::d3d12 as gfx_platform;

use crate::os::Window;

use std::ffi::CStr;
use std::ffi::CString;

#[derive(Clone)]
struct ImGuiPlatform {
    window: *mut os_platform::Window,
    mouse_window: *mut os_platform::Window,
    time: u64,
    ticks_per_second: u64,
    last_mouse_cursor: ImGuiMouseCursor,
    mouse_tracked: bool,
    has_gamepad: bool,
    want_update_has_gamepad: bool,
    want_update_monitors: bool 
}

#[derive(Clone)]
struct ImGuiRenderer {
    main_window: *mut os_platform::Window,
    device: *mut gfx_platform::Device,
    swap_chain: *mut gfx_platform::SwapChain,
    font_texture: Option<gfx_platform::Texture>,
    pipeline: Option<gfx_platform::RenderPipeline>
}

struct ImGuiViewport {
    device: *mut gfx_platform::Device,
    window: os_platform::Window,
    swap_chain: gfx_platform::SwapChain,
    cmd: gfx_platform::CmdBuf
}

pub struct ImGuiInfo {
    pub main_window: *mut os_platform::Window,
    pub device: *mut gfx_platform::Device,
    pub swap_chain: *mut gfx_platform::SwapChain,
    pub fonts: Vec<String>
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

static mut renderer_data : ImGuiRenderer = ImGuiRenderer {
    main_window: std::ptr::null_mut(),
    device: std::ptr::null_mut(),
    swap_chain: std::ptr::null_mut(),
    font_texture: None,
    pipeline: None
};

fn create_fonts_texture() {
    unsafe {
        let io = &*igGetIO();
        let mut out_pixels : *mut u8 = std::ptr::null_mut();
        let mut out_width = 0;
        let mut out_height = 0;
        let mut out_bytes_per_pixel = 0;
        ImFontAtlas_GetTexDataAsRGBA32(io.Fonts, &mut out_pixels, &mut out_width, &mut out_height, &mut out_bytes_per_pixel);
    }
}

fn setup_renderer_interface() {
    unsafe {
        /*
        platform_io.Renderer_CreateWindow = ImGui_ImplDX12_CreateWindow;
        platform_io.Renderer_DestroyWindow = ImGui_ImplDX12_DestroyWindow;
        platform_io.Renderer_SetWindowSize = ImGui_ImplDX12_SetWindowSize;
        platform_io.Renderer_RenderWindow = ImGui_ImplDX12_RenderWindow;
        platform_io.Renderer_SwapBuffers = ImGui_ImplDX12_SwapBuffers;
        */
    }
}

fn setup_renderer(info: &ImGuiInfo) {
    unsafe {
        let mut io = &mut *igGetIO();
        io.BackendRendererUserData = std::mem::transmute(&renderer_data.clone());
        io.BackendRendererName = "imgui_impl_hotline".as_ptr() as *const i8;
        io.BackendFlags |= ImGuiBackendFlags_RendererHasVtxOffset as i32; 
        io.BackendFlags |= ImGuiBackendFlags_RendererHasViewports as i32; 

        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            setup_platform_interface();
        }

        create_fonts_texture();
    }
}

fn new_frame_renderer() {

}

fn render_platform_windows() {
    
}

fn render_draw_data() {
    
}

fn render_renderer() {

}

fn swap_renderer() {

}

fn setup_platform_interface() {
    unsafe {
        let mut platform_io = &mut *igGetPlatformIO();
    }

    /*
    platform_io.Platform_CreateWindow = ImGui_ImplWin32_CreateWindow;
    platform_io.Platform_DestroyWindow = ImGui_ImplWin32_DestroyWindow;
    platform_io.Platform_ShowWindow = ImGui_ImplWin32_ShowWindow;
    platform_io.Platform_SetWindowPos = ImGui_ImplWin32_SetWindowPos;
    platform_io.Platform_GetWindowPos = ImGui_ImplWin32_GetWindowPos;
    platform_io.Platform_SetWindowSize = ImGui_ImplWin32_SetWindowSize;
    platform_io.Platform_GetWindowSize = ImGui_ImplWin32_GetWindowSize;
    platform_io.Platform_SetWindowFocus = ImGui_ImplWin32_SetWindowFocus;
    platform_io.Platform_GetWindowFocus = ImGui_ImplWin32_GetWindowFocus;
    platform_io.Platform_GetWindowMinimized = ImGui_ImplWin32_GetWindowMinimized;
    platform_io.Platform_SetWindowTitle = ImGui_ImplWin32_SetWindowTitle;
    platform_io.Platform_SetWindowAlpha = ImGui_ImplWin32_SetWindowAlpha;
    platform_io.Platform_UpdateWindow = ImGui_ImplWin32_UpdateWindow;
    platform_io.Platform_GetWindowDpiScale = ImGui_ImplWin32_GetWindowDpiScale; // FIXME-DPI
    platform_io.Platform_OnChangedViewport = ImGui_ImplWin32_OnChangedViewport; // FIXME-DPI
    */
}

fn setup_platform(info: &ImGuiInfo) {
    unsafe {
        let mut io = &mut *igGetIO();
        
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
        };

        // TODO: mouse update

        // TODO: keyboard mappings

        // TODO: gamepads

        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            setup_platform_interface();
        }
    }
}

fn new_frame_platform() {
    unsafe {
        let mut io = &mut *igGetIO();
        let window = &*platform_data.window;
        let (window_width, window_height) = window.get_size();
        io.DisplaySize = ImVec2 {
            x: window_width as f32,
            y: window_height as f32,
        }
    }
}

fn update_platform_windows() {

}

pub fn setup(info: &ImGuiInfo) {
    unsafe {
        igCreateContext(std::ptr::null_mut());
        let mut io = &mut *igGetIO();

        io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard as i32;
        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable as i32;
        io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable as i32;

        igStyleColorsLight(std::ptr::null_mut());

        let mut style = &mut *igGetStyle();
        style.WindowRounding = 0.0; 
        style.Colors[imgui_sys::ImGuiCol_WindowBg as usize].w = 1.0;

        // add fonts
        for font_name in &info.fonts {
            let null_font_name = CString::new(font_name.clone()).unwrap();
            ImFontAtlas_AddFontFromFileTTF(
                io.Fonts, null_font_name.as_ptr() as *const i8, 16.0, std::ptr::null_mut(), std::ptr::null_mut());
        }

        setup_platform(info);
        setup_renderer(info);
    }
}

pub fn new_frame() {
    new_frame_platform();
    new_frame_renderer();
    unsafe { igNewFrame(); }
}

pub fn render() {
    unsafe { igRender(); }
    render_renderer();
    update_platform_windows();
    render_platform_windows();
    swap_renderer();
}

pub fn demo() {

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