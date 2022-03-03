use imgui_sys::*;

#[cfg(target_os = "windows")]
use crate::os::win32 as os_platform;
use crate::gfx::d3d12 as gfx_platform;

use crate::os::Window;

use crate::gfx;
use crate::gfx::Device;
use crate::gfx::SwapChain;
use crate::gfx::CmdBuf;
use crate::gfx::Buffer;
use crate::gfx::Texture;

use std::ffi::CString;

pub struct ImGuiInfo {
    pub main_window: *mut os_platform::Window,
    pub device: *mut gfx_platform::Device,
    pub swap_chain: *mut gfx_platform::SwapChain,
    pub fonts: Vec<String>
}

pub struct ImGuiInfo2 {
    pub device: &'static mut gfx_platform::Device,
    pub main_window: &'static mut os_platform::Window,
    pub swap_chain: &'static mut gfx_platform::SwapChain,
    pub fonts: Vec<String>
}

pub struct ImGui {
    device: &'static mut gfx_platform::Device,
    main_window: &'static mut os_platform::Window,
    swap_chain: &'static mut gfx_platform::SwapChain,
    fonts: Vec<String>,
    font_texture: gfx_platform::Texture,
    pipeline: gfx_platform::RenderPipeline,
    cmd: gfx_platform::CmdBuf,
    buffers: Vec<RenderBuffers>,
    viewports: Vec<ViewportData>,
}

#[derive(Clone)]
struct RenderData {
    main_window: *mut os_platform::Window,
    device: *mut gfx_platform::Device,
    swap_chain: *mut gfx_platform::SwapChain,
    font_texture: Option<gfx_platform::Texture>,
    pipeline: Option<gfx_platform::RenderPipeline>
}


#[derive(Clone)]
struct RenderBuffers {
    vb: gfx_platform::Buffer,
    ib: gfx_platform::Buffer,
    vb_size: i32,
    ib_size: i32,
}

#[derive(Clone)]
struct ViewportData {
    device: *mut gfx_platform::Device,
    window: Option<os_platform::Window>,
    swap_chain: Option<gfx_platform::SwapChain>,
    cmd: gfx_platform::CmdBuf,
    buffers: Vec<RenderBuffers>,
}

static mut render_data : RenderData = RenderData {
    main_window: std::ptr::null_mut(),
    device: std::ptr::null_mut(),
    swap_chain: std::ptr::null_mut(),
    font_texture: None,
    pipeline: None
};

fn get_main_viewport_data<'a>() -> &'a mut ViewportData {
    unsafe {
        let main_viewport = &mut *igGetMainViewport();
        &mut *(main_viewport.RendererUserData as *mut ViewportData)
    }
}

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

fn create_render_pipeline(info: &ImGuiInfo2) -> Result<gfx_platform::RenderPipeline, gfx::Error> {
    unsafe {
        let device = &info.device;
        let swap_chain = &info.swap_chain;
    
        // TODO: temp: compile shaders
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
                output.pos = mul(ProjectionMatrix, float4(input.pos.xy, 0.0, 1.0));
                output.col = input.col;
                output.uv  = input.uv;
                return output;
            }

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
                    independent_blend_enabled: true,
                    render_target: vec![
                        gfx::RenderTargetBlendInfo {
                            blend_enabled: true,
                            logic_op_enabled: false,
                            src_blend: gfx::BlendFactor::SrcAlpha,
                            dst_blend: gfx::BlendFactor::InvSrcAlpha,
                            blend_op: gfx::BlendOp::Add,
                            src_blend_alpha: gfx::BlendFactor::One,
                            dst_blend_alpha: gfx::BlendFactor::InvSrcAlpha,
                            blend_op_alpha: gfx::BlendOp::Add,
                            logic_op: gfx::LogicOp::Clear,
                            write_mask: gfx::WriteMask::ALL
                        }
                    ],
                    ..Default::default() 
                },
                topology: gfx::Topology::TriangleList,
                patch_index: 0,
                pass: swap_chain.get_backbuffer_pass()
        })?)
    }
}

fn create_or_resize_buffers(
    device: &mut gfx_platform::Device, 
    vb_size: i32, 
    ib_size: i32,
    buffers: Option<&RenderBuffers>) -> Result<RenderBuffers, gfx::Error> {

    // TODO: fix double init

    let vb = if let Some(existing) = buffers {
        if existing.vb_size < vb_size {
            device.create_buffer::<u8>(&gfx::BufferInfo {
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::WRITE,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<ImDrawVert>(),
                num_elements: vb_size as usize, 
            }, None)?
        }
        else {
            (&existing.vb).clone()
        }
    }
    else {
        device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Vertex,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: vb_size as usize, 
        }, None)?
    };

    let ib = if let Some(existing) = buffers {
        if existing.ib_size < ib_size {
            device.create_buffer::<u8>(&gfx::BufferInfo {
                usage: gfx::BufferUsage::Index,
                cpu_access: gfx::CpuAccessFlags::WRITE,
                format: gfx::Format::R16u,
                stride: std::mem::size_of::<ImDrawIdx>(),
                num_elements: ib_size as usize, 
            }, None)?
        }
        else {
            (&existing.ib).clone()
        }
    }
    else {
        device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<ImDrawIdx>(),
            num_elements: ib_size as usize, 
        }, None)?
    };

    Ok(RenderBuffers{
        vb: vb,
        ib: ib,
        vb_size: vb_size,
        ib_size: ib_size
    })
}

fn setup_renderer(info: &ImGuiInfo) -> std::result::Result<(), gfx::Error> {
    unsafe {
        let mut io = &mut *igGetIO();
        io.BackendRendererUserData = std::mem::transmute(&render_data.clone());
        io.BackendRendererName = "imgui_impl_hotline".as_ptr() as *const i8;
        io.BackendFlags |= ImGuiBackendFlags_RendererHasVtxOffset as i32; 
        io.BackendFlags |= ImGuiBackendFlags_RendererHasViewports as i32; 

        render_data = RenderData {
            main_window: info.main_window,
            device: info.device,
            swap_chain: info.swap_chain,
            font_texture: Some(create_fonts_texture(&mut *info.device)?),
            pipeline: None, //Some(create_render_pipeline(info)?)
        };

        let mut buffers : Vec<RenderBuffers> = Vec::new();
        let num_buffers = (*info.swap_chain).get_num_buffers();
        for i in 0..num_buffers {
            buffers.push(
                create_or_resize_buffers(&mut *info.device, 5000, 10000, None)?
            )
        }

        let main_viewport_data = ViewportData{
            device: info.device,
            window: None,
            swap_chain: None,
            cmd: (*info.device).create_cmd_buf(2),
            buffers: buffers,
        };
        
        let layout = std::alloc::Layout::new::<ViewportData>(); 
        let ptr = std::alloc::alloc(layout);
        std::ptr::copy_nonoverlapping((&main_viewport_data as *const ViewportData) as *const u8, 
            ptr, std::mem::size_of::<ViewportData>());

        std::mem::forget(main_viewport_data);

        let mut main_viewport = &mut *igGetMainViewport();
        main_viewport.RendererUserData = ptr as *mut cty::c_void;
                        
        Ok(())
    }
}

fn render_draw_data(draw_data: &ImDrawData, cmd: &mut gfx_platform::CmdBuf) -> Result<(), gfx::Error> {
    unsafe {
        let vp = get_main_viewport_data();
        let swap_chain = &(*render_data.swap_chain);
        let dev = &*vp.device;

        let font_tex = render_data.font_texture.as_ref().unwrap();
        let font_tex_index = font_tex.get_srv_index().unwrap();

        let bb = swap_chain.get_backbuffer_index() as usize;

        // resize buffers
        vp.buffers[bb] = create_or_resize_buffers(
            &mut *vp.device, draw_data.TotalVtxCount, draw_data.TotalIdxCount, Some(&vp.buffers[bb]))?;

        // update buffers
        let buffers = &vp.buffers[bb];
        let imgui_cmd_lists = std::slice::from_raw_parts(draw_data.CmdLists, draw_data.CmdListsCount as usize);
        let mut vertex_write_offset = 0;
        let mut index_write_offset = 0;
        for imgui_cmd_list in imgui_cmd_lists {
            // vertex
            let draw_vert = &(*(*imgui_cmd_list)).VtxBuffer;
            let vb_size_bytes = draw_vert.Size as usize * std::mem::size_of::<ImDrawVert>();
            let vb_slice = std::slice::from_raw_parts(draw_vert.Data, vb_size_bytes);
            buffers.vb.update(vertex_write_offset, vb_slice)?;
            vertex_write_offset += draw_vert.Size as isize;
            // index
            let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
            let ib_size_bytes = draw_index.Size as usize * std::mem::size_of::<ImDrawIdx>();
            let ib_slice = std::slice::from_raw_parts(draw_index.Data, ib_size_bytes);
            buffers.ib.update(index_write_offset, ib_slice)?;
            index_write_offset += draw_index.Size as isize;
        }

        // update push constants
        let l = draw_data.DisplayPos.x;
        let r = draw_data.DisplayPos.x + draw_data.DisplaySize.x;
        let t = draw_data.DisplayPos.y;
        let b = draw_data.DisplayPos.y + draw_data.DisplaySize.y;

        let mvp : [[f32;4];4] = [
            [2.0/(r-l), 0.0, 0.0, 0.0],
            [0.0, 2.0/(t-b), 0.0, 0.0],
            [0.0, 0.0, 0.5, 0.0],
            [(r+l)/(l-r), (t+b)/(b-t), 0.0, 1.0]
        ];
        
        // let cmd = &mut vp.cmd;
        // cmd.reset(swap_chain);

        cmd.set_marker(0xff00ffff, "ImGui");

        let viewport = gfx::Viewport {
            x: 0.0,
            y: 0.0,
            width: draw_data.DisplaySize.x,
            height: draw_data.DisplaySize.y,
            min_depth: 0.0,
            max_depth: 1.0
        };

        cmd.set_viewport(&viewport);
        cmd.set_vertex_buffer(&buffers.vb, 0);
        cmd.set_index_buffer(&buffers.ib);
        cmd.set_render_pipeline(&render_data.pipeline.as_ref().unwrap());
        cmd.push_constants(0, 16, 0, &mvp);

        let clip_off = draw_data.DisplayPos;
        let mut global_vtx_offset = 0;
        let mut global_idx_offset = 0;
        for imgui_cmd_list in imgui_cmd_lists {
            let imgui_cmd_buffer = (**imgui_cmd_list).CmdBuffer;
            let imgui_cmd_data = std::slice::from_raw_parts(imgui_cmd_buffer.Data, imgui_cmd_buffer.Size as usize);
            let draw_vert = &(*(*imgui_cmd_list)).VtxBuffer;
            let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
            for i in 0..imgui_cmd_buffer.Size as usize {
                let imgui_cmd = &imgui_cmd_data[i];
                if imgui_cmd.UserCallback.is_some() {

                }
                else {
                    let clip_min_x = imgui_cmd.ClipRect.x - clip_off.x;
                    let clip_min_y = imgui_cmd.ClipRect.y - clip_off.y;
                    let clip_max_x = imgui_cmd.ClipRect.z - clip_off.x;
                    let clip_max_y = imgui_cmd.ClipRect.w - clip_off.y;
                    if clip_max_x < clip_min_x || clip_max_y < clip_min_y {
                        continue;
                    }

                    let scissor = gfx::ScissorRect {
                        left: clip_min_x as i32,
                        top: clip_min_y as i32,
                        right: clip_max_x as i32,
                        bottom: clip_max_y as i32
                    };

                    cmd.set_render_heap(1, dev.get_shader_heap(), font_tex_index);
                    cmd.set_scissor_rect(&scissor);
                    cmd.draw_indexed_instanced(
                        imgui_cmd.ElemCount, 1, 
                        imgui_cmd.IdxOffset + global_idx_offset, (imgui_cmd.VtxOffset + global_vtx_offset) as i32, 0);
                }
            }
            global_idx_offset += draw_vert.Size as u32;
            global_vtx_offset += draw_index.Size as u32;
        }

        Ok(())
    }
}

fn setup_platform(info: &ImGuiInfo) {
    unsafe {
        let mut io = &mut *igGetIO();
        
        // io setup
        io.BackendPlatformName = "imgui_impl_hotline".as_ptr() as *const i8;
        io.BackendFlags |= ImGuiBackendFlags_HasMouseCursors as i32;
        io.BackendFlags |= ImGuiBackendFlags_HasSetMousePos as i32;
        io.BackendFlags |= ImGuiBackendFlags_PlatformHasViewports as i32;
        io.BackendFlags |= ImGuiBackendFlags_HasMouseHoveredViewport as i32;

        // TODO: mouse update

        // TODO: keyboard mappings

        // TODO: gamepads

        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            // setup_platform_interface();
        }
    }
}

fn new_frame_platform() {
    unsafe {
        let mut io = &mut *igGetIO();

        /*
        let window = &*platform_data.window;
        let (window_width, window_height) = window.get_size();
        io.DisplaySize = ImVec2 {
            x: window_width as f32,
            y: window_height as f32,
        }
        */
    }
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
        setup_renderer(info).unwrap();
    }
}

pub fn new_frame() {
    new_frame_platform();
    unsafe { igNewFrame(); }
}

pub fn render(cmd: &mut gfx_platform::CmdBuf) {
    unsafe {
        let io = &mut *igGetIO(); 
        igRender();
        render_draw_data(&*igGetDrawData(), cmd).unwrap();
        if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
            //update_platform_windows();
            //render_platform_windows();
        }
        //swap_renderer(); 
    }
}

pub fn demo() {
    unsafe {
        let mut open = true;
        igShowDemoWindow(&mut open);
    }
}

impl ImGui {
    pub fn create(info: &'static mut ImGuiInfo2) -> Result<Self, gfx::Error> {
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
    
            // io setup
            io.BackendPlatformName = "imgui_impl_hotline".as_ptr() as *const i8;
            io.BackendFlags |= ImGuiBackendFlags_HasMouseCursors as i32;
            io.BackendFlags |= ImGuiBackendFlags_HasSetMousePos as i32;
            io.BackendFlags |= ImGuiBackendFlags_PlatformHasViewports as i32;
            io.BackendFlags |= ImGuiBackendFlags_HasMouseHoveredViewport as i32;

            // renderer setup
            io.BackendRendererName = "imgui_impl_hotline".as_ptr() as *const i8;
            io.BackendFlags |= ImGuiBackendFlags_RendererHasVtxOffset as i32; 
            io.BackendFlags |= ImGuiBackendFlags_RendererHasViewports as i32; 

            // create render buffers
            let mut buffers : Vec<RenderBuffers> = Vec::new();
            let num_buffers = (*info.swap_chain).get_num_buffers();
            for i in 0..num_buffers {
                buffers.push(
                    create_or_resize_buffers(&mut info.device, 5000, 10000, None)?
                )
            }

            let font_tex = create_fonts_texture(&mut info.device)?;
            let pipeline = create_render_pipeline(info)?;
            let cmd = info.device.create_cmd_buf(2);
    
            //
            let imgui = ImGui {
                device: info.device,
                main_window: info.main_window,
                swap_chain: info.swap_chain,
                fonts: info.fonts.clone(),
                font_texture: font_tex,
                pipeline: pipeline,
                buffers: buffers,
                cmd: cmd,
                viewports: Vec::new()
            };
    
            if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
                imgui.setup_platform_interface();
            }

            imgui.setup_renderer_interface();

            Ok(imgui)
        }
    }

    fn setup_platform_interface(&self) {
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

    fn setup_renderer_interface(&self) {
        /*
        platform_io.Renderer_CreateWindow = ImGui_ImplDX12_CreateWindow;
        platform_io.Renderer_DestroyWindow = ImGui_ImplDX12_DestroyWindow;
        platform_io.Renderer_SetWindowSize = ImGui_ImplDX12_SetWindowSize;
        platform_io.Renderer_RenderWindow = ImGui_ImplDX12_RenderWindow;
        platform_io.Renderer_SwapBuffers = ImGui_ImplDX12_SwapBuffers;
        */
    }
}