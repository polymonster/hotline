use imgui_sys;

pub fn setup() {
    unsafe {
        imgui_sys::igCreateContext(std::ptr::null_mut());
        let mut io = *imgui_sys::igGetIO();

        io.ConfigFlags |= imgui_sys::ImGuiConfigFlags_NavEnableKeyboard as i32;
        io.ConfigFlags |= imgui_sys::ImGuiConfigFlags_DockingEnable as i32;
        io.ConfigFlags |= imgui_sys::ImGuiConfigFlags_ViewportsEnable as i32;

        imgui_sys::igStyleColorsLight(std::ptr::null_mut());

        let mut style = *imgui_sys::igGetStyle();
        style.WindowRounding = 0.0; 
        style.Colors[imgui_sys::ImGuiCol_WindowBg as usize].w = 1.0;
    }
}