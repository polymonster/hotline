use imgui_sys::*;

#[cfg(target_os = "windows")]
use crate::os::win32 as os_platform;
use crate::gfx::d3d12 as gfx_platform;

use crate::os::Window;

use crate::gfx;
use crate::gfx::Device;
use crate::gfx::SwapChain;

use std::ffi::CString;

pub struct ImGuiInfo {
    pub main_window: *mut os_platform::Window,
    pub device: *mut gfx_platform::Device,
    pub swap_chain: *mut gfx_platform::SwapChain,
    pub fonts: Vec<String>
}

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

#[derive(Clone)]
struct DrawBuffers {
    vb: gfx_platform::Buffer,
    ib: gfx_platform::Buffer,
    vb_size: i32,
    ib_size: i32,
}

#[derive(Clone)]
struct ImGuiViewport {
    device: *mut gfx_platform::Device,
    window: Option<os_platform::Window>,
    swap_chain: Option<gfx_platform::SwapChain>,
    cmd: gfx_platform::CmdBuf,
    buffers: DrawBuffers,
    magic: u32
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

fn create_fonts_texture(device: &mut gfx_platform::Device) -> Result<gfx_platform::Texture, gfx::Error> {
    unsafe {
        let io = &*igGetIO();
        let mut out_pixels : *mut u8 = std::ptr::null_mut();
        let mut out_width = 0;
        let mut out_height = 0;
        let mut out_bytes_per_pixel = 0;
        ImFontAtlas_GetTexDataAsRGBA32(io.Fonts, &mut out_pixels, &mut out_width, &mut out_height, &mut out_bytes_per_pixel);

        let data_size = out_bytes_per_pixel * out_width * out_height;
        let data_slice = std::slice::from_raw_parts(out_pixels, data_size as usize);

        let tex_info = gfx::TextureInfo {
            format: gfx::Format::RGBA8n,
            tex_type: gfx::TextureType::Texture2D,
            width: out_width as u64,
            height: out_height as u64,
            depth: 1,
            array_levels: 1,
            mip_levels: 1,
            samples: 1,
            usage: gfx::TextureUsage::SHADER_RESOURCE,
            initial_state: gfx::ResourceState::ShaderResource
        };

        Ok(device.create_texture(&tex_info, Some(data_slice))?)
    }
}

fn create_render_pipeline(info: &ImGuiInfo) -> Result<gfx_platform::RenderPipeline, gfx::Error> {
    unsafe {
        let device = &*info.device;
        let swap_chain = &*info.swap_chain;
    
        // temp: compile shaders
        let src = "
            cbuffer vertexBuffer : register(b0)
            {
                float4x4 ProjectionMatrix;
            };
            struct VS_INPUT
            {
                float2 pos : POSITION;
                float4 col : COLOR0;
                float2 uv  : TEXCOORD0;
            };
            
            struct PS_INPUT
            {
                float4 pos : SV_POSITION;
                float4 col : COLOR0;
                float2 uv  : TEXCOORD0;
            };
            
            PS_INPUT VSMain(VS_INPUT input)
            {
                PS_INPUT output;
                output.pos = mul( ProjectionMatrix, float4(input.pos.xy, 0.f, 1.f));
                output.col = input.col;
                output.uv  = input.uv;
                return output;
            }
            struct PS_INPUT
            {
              float4 pos : SV_POSITION;
              float4 col : COLOR0;
              float2 uv  : TEXCOORD0;
            };
            SamplerState sampler0 : register(s0);
            Texture2D texture0 : register(t0);
            
            float4 PSMain(PS_INPUT input) : SV_Target
            {
              float4 out_col = input.col * texture0.Sample(sampler0, input.uv);
              return out_col;
            }";
    
        let vs_info = gfx::ShaderInfo {
            shader_type: gfx::ShaderType::Vertex,
            compile_info: Some(gfx::ShaderCompileInfo {
                entry_point: String::from("VSMain"),
                target: String::from("vs_5_0"),
                flags: gfx::ShaderCompileFlags::NONE,
            }),
        };
    
        let fs_info = gfx::ShaderInfo {
            shader_type: gfx::ShaderType::Fragment,
            compile_info: Some(gfx::ShaderCompileInfo {
                entry_point: String::from("PSMain"),
                target: String::from("ps_5_0"),
                flags: gfx::ShaderCompileFlags::NONE,
            }),
        };
    
        let vs = device.create_shader(&vs_info, src.as_bytes())?;
        let fs = device.create_shader(&fs_info, src.as_bytes())?;
    
        Ok( device.create_render_pipeline(&gfx::RenderPipelineInfo {
                vs: Some(vs),
                fs: Some(fs),
                input_layout: vec![
                    gfx::InputElementInfo {
                        semantic: String::from("POSITION"),
                        index: 0,
                        format: gfx::Format::RG32f,
                        input_slot: 0,
                        aligned_byte_offset: 0,
                        input_slot_class: gfx::InputSlotClass::PerVertex,
                        step_rate: 0,
                    },
                    gfx::InputElementInfo {
                        semantic: String::from("TEXCOORD"),
                        index: 0,
                        format: gfx::Format::RG32f,
                        input_slot: 0,
                        aligned_byte_offset: 8,
                        input_slot_class: gfx::InputSlotClass::PerVertex,
                        step_rate: 0,
                    },
                    gfx::InputElementInfo {
                        semantic: String::from("COLOR"),
                        index: 0,
                        format: gfx::Format::RGBA8n,
                        input_slot: 0,
                        aligned_byte_offset: 16,
                        input_slot_class: gfx::InputSlotClass::PerVertex,
                        step_rate: 0,
                    },
                ],
                descriptor_layout: gfx::DescriptorLayout {
                    push_constants: Some(vec![gfx::PushConstantInfo {
                        visibility: gfx::ShaderVisibility::Vertex,
                        num_values: 16,
                        shader_register: 0,
                        register_space: 0,
                    }]),
                    bindings: Some(vec![
                        gfx::DescriptorBinding {
                            visibility: gfx::ShaderVisibility::Fragment,
                            binding_type: gfx::DescriptorType::ShaderResource,
                            num_descriptors: Some(1),
                            shader_register: 0,
                            register_space: 0,
                        }
                    ]),
                    static_samplers: Some(vec![gfx::SamplerInfo {
                        visibility: gfx::ShaderVisibility::Fragment,
                        filter: gfx::SamplerFilter::Linear,
                        address_u: gfx::SamplerAddressMode::Wrap,
                        address_v: gfx::SamplerAddressMode::Wrap,
                        address_w: gfx::SamplerAddressMode::Wrap,
                        comparison: None,
                        border_colour: None,
                        mip_lod_bias: 0.0,
                        max_aniso: 0,
                        min_lod: -1.0,
                        max_lod: -1.0,
                        shader_register: 0,
                        register_space: 0,
                    }]),
                },
                raster_info: gfx::RasterInfo::default(),
                depth_stencil_info: gfx::DepthStencilInfo::default(),
                blend_info: gfx::BlendInfo {
                    render_target: vec![
                        gfx::RenderTargetBlendInfo::default()
                    ],
                    ..Default::default() 
                },
                topology: gfx::Topology::TriangleList,
                patch_index: 0,
                pass: swap_chain.get_backbuffer_pass()
        })?)
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

fn create_or_resize_buffers(
    device: &mut gfx_platform::Device, 
    vb_size: i32, 
    ib_size: i32,
    buffers: Option<DrawBuffers>) -> Result<DrawBuffers, gfx::Error> {

    let vb = if buffers.is_none() || buffers.as_ref().unwrap().vb_size < vb_size {
        device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Vertex,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: vb_size as usize, 
        }, None)?
    }
    else {
        buffers.as_ref().unwrap().vb.clone()
    };

    let ib = if buffers.is_none() || buffers.as_ref().unwrap().vb_size < vb_size {
        device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            format: gfx::Format::R16i,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: ib_size as usize, 
        }, None)?
    }
    else {
        buffers.as_ref().unwrap().ib.clone()
    };
        
    Ok(DrawBuffers{
        vb: vb,
        ib: ib,
        vb_size: vb_size,
        ib_size: ib_size
    })
}

fn setup_renderer(info: &ImGuiInfo) -> std::result::Result<(), gfx::Error> {
    unsafe {
        let mut io = &mut *igGetIO();
        io.BackendRendererUserData = std::mem::transmute(&renderer_data.clone());
        io.BackendRendererName = "imgui_impl_hotline".as_ptr() as *const i8;
        io.BackendFlags |= ImGuiBackendFlags_RendererHasVtxOffset as i32; 
        io.BackendFlags |= ImGuiBackendFlags_RendererHasViewports as i32; 

        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            setup_platform_interface();
        }

        renderer_data = ImGuiRenderer {
            main_window: info.main_window,
            device: info.device,
            swap_chain: info.swap_chain,
            font_texture: Some(create_fonts_texture(&mut *info.device)?),
            pipeline: Some(create_render_pipeline(info)?)
        };

        let main_viewport_data = std::boxed::Box::new(ImGuiViewport{
            device: info.device,
            window: None,
            swap_chain: None,
            cmd: (*info.device).create_cmd_buf(2),
            buffers: create_or_resize_buffers(&mut *info.device, 5000, 10000, None)?,
            magic: 696969
        });

        let mut main_viewport = &mut *igGetMainViewport();
        main_viewport.RendererUserData = std::mem::transmute(&mut main_viewport_data.clone());

        Ok(())
    }
}

fn new_frame_renderer() {

}

fn render_platform_windows() {
    
}

fn render_draw_data(draw_data: &ImDrawData, cmd_buf: &gfx_platform::CmdBuf) {
    unsafe {
        let a = 0;
    }
}

fn render_renderer() {
    unsafe {
        let ig_main_viewport = &*igGetMainViewport();
        let main_viewport : &ImGuiViewport = std::mem::transmute(ig_main_viewport.RendererUserData);
        render_draw_data(&*igGetDrawData(), &main_viewport.cmd);
    }
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

        io.ConfigFlags |= ImGuiConfigFlags_DockingEnable as i32;
        //io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard as i32;
        //io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable as i32;

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
    unsafe {
        let io = &mut *igGetIO(); 
        igRender();
        render_renderer();
        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            update_platform_windows();
            render_platform_windows();
        }
        swap_renderer(); 
    }
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