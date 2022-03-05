use imgui_sys::*;

#[cfg(target_os = "windows")]
use crate::os::win32 as os_platform;
use crate::gfx::d3d12 as gfx_platform;

use crate::os;
use crate::os::App;
use crate::os::Window;

use crate::gfx;
use crate::gfx::Device;
use crate::gfx::SwapChain;
use crate::gfx::CmdBuf;
use crate::gfx::Buffer;
use crate::gfx::Texture;

use std::ffi::CString;

pub struct ImGuiInfo<'a> {
    pub device: &'a mut gfx_platform::Device,
    pub swap_chain: &'a mut gfx_platform::SwapChain,
    pub fonts: Vec<String>
}

pub struct ImGui {
    fonts: Vec<String>,
    font_texture: gfx_platform::Texture,
    pipeline: gfx_platform::RenderPipeline,
    buffers: Vec<RenderBuffers>,
    viewports: Vec<ViewportData>,
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

fn create_vertex_buffer(
    device: &mut gfx_platform::Device, 
    size: i32, ) -> Result<gfx_platform::Buffer, gfx::Error> {
        Ok(device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Vertex,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: size as usize, 
        }, None)?)
}

fn create_index_buffer(
    device: &mut gfx_platform::Device, 
    size: i32, ) -> Result<gfx_platform::Buffer, gfx::Error> {
        Ok(device.create_buffer::<u8>(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<ImDrawIdx>(),
            num_elements: size as usize, 
        }, None)?)
}

impl ImGui {
    pub fn create(info: &mut ImGuiInfo) -> Result<Self, gfx::Error> {
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
            for _i in 0..num_buffers {
                buffers.push(
                    RenderBuffers {
                        vb: create_vertex_buffer(&mut info.device, 5000)?,
                        vb_size: 5000,
                        ib: create_index_buffer(&mut info.device, 10000)?,
                        ib_size: 10000,
                    }
                )
            }

            let font_tex = create_fonts_texture(&mut info.device)?;
            let pipeline = create_render_pipeline(info)?;
    
            //
            let imgui = ImGui {
                fonts: info.fonts.clone(),
                font_texture: font_tex,
                pipeline: pipeline,
                buffers: buffers,
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
        unsafe {
            let platform_io = &mut *igGetPlatformIO(); 
            platform_io.Platform_CreateWindow = Some(platform_create_window);
            platform_io.Platform_DestroyWindow = Some(platform_destroy_window);
            platform_io.Platform_ShowWindow = Some(platform_show_window);
            platform_io.Platform_SetWindowPos = Some(platform_set_window_pos);
            platform_io.Platform_GetWindowPos = Some(platform_get_window_pos);
            platform_io.Platform_SetWindowSize = Some(platform_set_window_size);
            platform_io.Platform_GetWindowSize = Some(platform_get_window_size);
            platform_io.Platform_SetWindowFocus = Some(platform_set_window_focus);
            platform_io.Platform_GetWindowFocus = Some(platform_get_window_focus);
            platform_io.Platform_GetWindowMinimized = Some(platform_get_window_minimised);
            platform_io.Platform_SetWindowTitle = Some(platform_set_window_title);
            platform_io.Platform_SetWindowAlpha = Some(platform_set_window_alpha);
        }
        //platform_io.Platform_UpdateWindow = ImGui_ImplWin32_UpdateWindow;
        //platform_io.Platform_GetWindowDpiScale = ImGui_ImplWin32_GetWindowDpiScale; // FIXME-DPI
        //platform_io.Platform_OnChangedViewport = ImGui_ImplWin32_OnChangedViewport; // FIXME-DPI
    }//

    fn setup_renderer_interface(&self) {
        //platform_io.Renderer_CreateWindow = ImGui_ImplDX12_CreateWindow;
        //platform_io.Renderer_DestroyWindow = ImGui_ImplDX12_DestroyWindow;
        //platform_io.Renderer_SetWindowSize = ImGui_ImplDX12_SetWindowSize;
        //platform_io.Renderer_RenderWindow = ImGui_ImplDX12_RenderWindow;
        //platform_io.Renderer_SwapBuffers = ImGui_ImplDX12_SwapBuffers;
    }

    pub fn new_frame(&self, app: &os_platform::App, main_window: &mut os_platform::Window) {
        let (window_width, window_height) = main_window.get_size();
        unsafe {
            let io = &mut *igGetIO(); 

            // update display
            io.DisplaySize = ImVec2 {
                x: window_width as f32,
                y: window_height as f32,
            };

            // update mouse
            if io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32 == 0 {
                // non viewports mouse coords are in client space
                let client_mouse = main_window.get_mouse_client_pos(&app.get_mouse_pos());
                io.MousePos = ImVec2::from(client_mouse);
            }
            else {
                // viewports mouse coords are in screen space
                io.MousePos = ImVec2::from(app.get_mouse_pos());
            }

            io.MouseWheel =app.get_mouse_wheel();
            io.MouseWheelH =app.get_mouse_wheel();
            io.MouseDown = app.get_mouse_buttons();

            igNewFrame();
        }
    }

    pub fn render(&mut self, device: &mut gfx_platform::Device, cmd: &mut gfx_platform::CmdBuf) {
        unsafe {
            let io = &mut *igGetIO(); 
            igRender();
            self.render_draw_data(&*igGetDrawData(), device, cmd).unwrap();
            if(io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
                //update_platform_windows();
                //render_platform_windows();
            }
            //swap_renderer(); 
        }
    }

    fn render_draw_data(&mut self, draw_data: &ImDrawData, device: &mut gfx_platform::Device, cmd: &mut gfx_platform::CmdBuf) -> Result<(), gfx::Error> {
        unsafe {    
            let font_tex_index = self.font_texture.get_srv_index().unwrap();
            let bb = cmd.get_backbuffer_index() as usize;
    
            let mut buffers = &mut self.buffers[bb];

            // resize vb
            if draw_data.TotalVtxCount > buffers.vb_size {
                // todo release
                buffers.vb = create_vertex_buffer(device, draw_data.TotalVtxCount)?;
                buffers.vb_size = draw_data.TotalVtxCount;
            }

            // resize ib
            if draw_data.TotalIdxCount > buffers.ib_size {
                // todo release
                buffers.ib = create_index_buffer(device, draw_data.TotalIdxCount)?;
                buffers.ib_size = draw_data.TotalIdxCount;
            }
    
            // update buffers
            let imgui_cmd_lists = std::slice::from_raw_parts(draw_data.CmdLists, draw_data.CmdListsCount as usize);
            let mut vertex_write_offset = 0;
            let mut index_write_offset = 0;

            for imgui_cmd_list in imgui_cmd_lists {
                // vertex
                let draw_vert = &(*(*imgui_cmd_list)).VtxBuffer;
                let vb_size_bytes = draw_vert.Size as usize * std::mem::size_of::<ImDrawVert>();
                let vb_slice = std::slice::from_raw_parts(draw_vert.Data, draw_vert.Size as usize);
                buffers.vb.update(vertex_write_offset, vb_slice)?;
                vertex_write_offset += vb_size_bytes as isize;
                // index
                let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
                let ib_size_bytes = draw_index.Size as usize * std::mem::size_of::<ImDrawIdx>();
                let ib_slice = std::slice::from_raw_parts(draw_index.Data, draw_index.Size as usize);
                buffers.ib.update(index_write_offset, ib_slice)?;
                index_write_offset += ib_size_bytes as isize;
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
            cmd.set_render_pipeline(&self.pipeline);
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
    
                        cmd.set_render_heap(1, device.get_shader_heap(), font_tex_index);
                        cmd.set_scissor_rect(&scissor);
                        cmd.draw_indexed_instanced(
                            imgui_cmd.ElemCount, 1, 
                            imgui_cmd.IdxOffset + global_idx_offset, (imgui_cmd.VtxOffset + global_vtx_offset) as i32, 0);
                    }
                }
                global_idx_offset += draw_index.Size as u32;
                global_vtx_offset += draw_vert.Size as u32;
            }
    
            Ok(())
        }
    }

    pub fn demo(&self) {
        unsafe {
            let mut open = true;
            igShowDemoWindow(&mut open);

            let io = &mut *igGetIO(); 
            igText("%f, %f : %f %f\0".as_ptr() as *const i8, 
                io.MousePos.x as f64, io.MousePos.y as f64,
                io.DisplaySize.x as f64, io.DisplaySize.y as f64
            );
        }
    }
}

impl From<os::Point<i32>> for ImVec2 {
    fn from(point: os::Point<i32>) -> ImVec2 {
        ImVec2 {
            x: point.x as f32,
            y: point.y as f32
        }
    }
}

unsafe extern "C" fn platform_create_window(vp: *mut ImGuiViewport) {
    let a = 0;
}

unsafe extern "C" fn platform_destroy_window(vp: *mut ImGuiViewport) {
    let a = 0;
}

unsafe extern "C" fn platform_show_window(vp: *mut ImGuiViewport) {
    let a = 0;
}

unsafe extern "C" fn platform_set_window_pos(vp: *mut ImGuiViewport, pos: ImVec2) {
    let a = 0;
}

unsafe extern "C" fn platform_get_window_pos(vp: *mut ImGuiViewport) -> ImVec2 {
    ImVec2::default()
}

unsafe extern "C" fn platform_set_window_size(vp: *mut ImGuiViewport, pos: ImVec2) {
    let a = 0;
}

unsafe extern "C" fn platform_get_window_size(vp: *mut ImGuiViewport) -> ImVec2 {
    ImVec2::default()
}

unsafe extern "C" fn platform_set_window_focus(vp: *mut ImGuiViewport) {
    let a = 0;
}

unsafe extern "C" fn platform_get_window_focus(vp: *mut ImGuiViewport) -> bool {
    false
}

unsafe extern "C" fn platform_get_window_minimised(vp: *mut ImGuiViewport) -> bool {
    false
}

unsafe extern "C" fn platform_set_window_title(vp: *mut ImGuiViewport, str_: *const cty::c_char) {
    let a = 0;
}

unsafe extern "C" fn platform_set_window_alpha(vp: *mut ImGuiViewport, alpha: f32) {
    let a = 0;
}

/*
pub Platform_UpdateWindow: ::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport)>,
pub Platform_RenderWindow: ::core::option::Option<
unsafe extern "C" fn(vp: *mut ImGuiViewport, render_arg: *mut cty::c_void),
>,
pub Platform_SwapBuffers: ::core::option::Option<
unsafe extern "C" fn(vp: *mut ImGuiViewport, render_arg: *mut cty::c_void),
>,
pub Platform_GetWindowDpiScale:
::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport) -> f32>,
pub Platform_OnChangedViewport:
::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport)>,
pub Platform_SetImeInputPos:
::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, pos: ImVec2)>,
*/

/*
pub Renderer_CreateWindow: ::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport)>,
pub Renderer_DestroyWindow:
::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport)>,
pub Renderer_SetWindowSize:
::core::option::Option<unsafe extern "C" fn(vp: *mut ImGuiViewport, size: ImVec2)>,
pub Renderer_RenderWindow: ::core::option::Option<
unsafe extern "C" fn(vp: *mut ImGuiViewport, render_arg: *mut cty::c_void),
>,
pub Renderer_SwapBuffers: ::core::option::Option<
unsafe extern "C" fn(vp: *mut ImGuiViewport, render_arg: *mut cty::c_void),
>,
*/