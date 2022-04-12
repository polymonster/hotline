use imgui_sys::*;

use crate::os;
use crate::os::App;
use crate::os::Window;
use crate::os::NativeHandle;

use crate::gfx;
use crate::gfx::Buffer;
use crate::gfx::CmdBuf;
use crate::gfx::Device;
use crate::gfx::SwapChain;
use crate::gfx::Texture;

use std::ffi::CStr;
use std::ffi::CString;

const DEFAULT_VB_SIZE: i32 = 5000;
const DEFAULT_IB_SIZE: i32 = 10000;

pub struct ImGuiInfo<'a, D: Device, A: App> {
    pub device: &'a mut D,
    pub swap_chain: &'a mut D::SwapChain,
    pub main_window: &'a A::Window,
    pub fonts: Vec<String>,
}

pub struct ImGui<D: Device, A: App> {
    _native_handle: A::NativeHandle,
    font_texture: D::Texture,
    pipeline: D::RenderPipeline,
    buffers: Vec<RenderBuffers<D>>,
    last_cursor: os::Cursor,
}

#[derive(Clone)]
struct RenderBuffers<D: Device> {
    vb: D::Buffer,
    ib: D::Buffer,
    vb_size: i32,
    ib_size: i32,
}

struct ViewportData<D: Device, A: App> {
    /// if viewport is main, we get the window from UserData and the rest of this struct is null
    main_viewport: bool,
    window: Vec<A::Window>,
    swap_chain: Vec<D::SwapChain>,
    cmd: Vec<D::CmdBuf>,
    buffers: Vec<RenderBuffers<D>>,
}

struct UserData<'a, D: Device, A: App> {
    app: &'a mut A,
    device: &'a mut D,
    main_window: &'a mut A::Window,
    pipeline: &'a D::RenderPipeline,
    font_texture: &'a D::Texture,
}

fn new_viewport_data<D: Device, A: App>() -> *mut ViewportData<D, A> {
    unsafe {
        let layout =
            std::alloc::Layout::from_size_align(std::mem::size_of::<ViewportData<D, A>>(), 8).unwrap();
        std::alloc::alloc_zeroed(layout) as *mut ViewportData<D, A>
    }
}

fn new_native_handle<A: App>(handle: A::NativeHandle) -> *mut A::NativeHandle {
    unsafe {
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<A::NativeHandle>(),
            8,
        )
        .unwrap();
        let nh = std::alloc::alloc_zeroed(layout) as *mut A::NativeHandle;
        *nh = handle;
        nh
    }
}

fn new_monitors(monitors: &Vec<ImGuiPlatformMonitor>) -> *mut ImGuiPlatformMonitor {
    unsafe {
        let size_bytes = std::mem::size_of::<ImGuiPlatformMonitor>() * monitors.len();
        let layout = std::alloc::Layout::from_size_align(size_bytes, 8).unwrap();
        let ptr = std::alloc::alloc_zeroed(layout) as *mut ImGuiPlatformMonitor;
        std::ptr::copy_nonoverlapping(monitors.as_ptr(), ptr, monitors.len());
        ptr
    }
}

fn create_fonts_texture<D: Device>(
    device: &mut D,
) -> Result<D::Texture, super::Error> {
    unsafe {
        let io = &*igGetIO();
        let mut out_pixels: *mut u8 = std::ptr::null_mut();
        let mut out_width = 0;
        let mut out_height = 0;
        let mut out_bytes_per_pixel = 0;
        ImFontAtlas_GetTexDataAsRGBA32(
            io.Fonts,
            &mut out_pixels,
            &mut out_width,
            &mut out_height,
            &mut out_bytes_per_pixel,
        );

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
            initial_state: gfx::ResourceState::ShaderResource,
        };

        Ok(device.create_texture(&tex_info, Some(data_slice))?)
    }
}

fn create_render_pipeline<D: Device, A: App>(info: &ImGuiInfo<D, A>) -> Result<D::RenderPipeline, super::Error> {
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

    Ok(device.create_render_pipeline(&gfx::RenderPipelineInfo {
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
            bindings: Some(vec![gfx::DescriptorBinding {
                visibility: gfx::ShaderVisibility::Fragment,
                binding_type: gfx::DescriptorType::ShaderResource,
                num_descriptors: Some(1),
                shader_register: 0,
                register_space: 0,
            }]),
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
            render_target: vec![gfx::RenderTargetBlendInfo {
                blend_enabled: true,
                logic_op_enabled: false,
                src_blend: gfx::BlendFactor::SrcAlpha,
                dst_blend: gfx::BlendFactor::InvSrcAlpha,
                blend_op: gfx::BlendOp::Add,
                src_blend_alpha: gfx::BlendFactor::One,
                dst_blend_alpha: gfx::BlendFactor::InvSrcAlpha,
                blend_op_alpha: gfx::BlendOp::Add,
                logic_op: gfx::LogicOp::Clear,
                write_mask: gfx::WriteMask::ALL,
            }],
            ..Default::default()
        },
        topology: gfx::Topology::TriangleList,
        patch_index: 0,
        pass: swap_chain.get_backbuffer_pass(),
    })?)
}

fn create_vertex_buffer<D: Device>(
    device: &mut D,
    size: i32,
) -> Result<D::Buffer, super::Error> {
    Ok(device.create_buffer::<u8>(
        &gfx::BufferInfo {
            usage: gfx::BufferUsage::Vertex,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: size as usize,
        },
        None,
    )?)
}

fn create_index_buffer<D: Device>(
    device: &mut D,
    size: i32,
) -> Result<D::Buffer, super::Error> {
    Ok(device.create_buffer::<u8>(
        &gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<ImDrawIdx>(),
            num_elements: size as usize,
        },
        None,
    )?)
}

fn render_draw_data<D: Device>(
    draw_data: &ImDrawData,
    device: &mut D,
    cmd: &mut D::CmdBuf,
    buffers: &mut Vec<RenderBuffers<D>>,
    pipeline: &D::RenderPipeline,
    font_texture: &D::Texture,
) -> Result<(), super::Error> {
    unsafe {
        let font_tex_index = font_texture.get_srv_index().unwrap();
        let bb = cmd.get_backbuffer_index() as usize;

        let mut buffers = &mut buffers[bb];

        // resize vb
        if draw_data.TotalVtxCount > buffers.vb_size {
            buffers.vb = create_vertex_buffer::<D>(device, draw_data.TotalVtxCount)?;
            buffers.vb_size = draw_data.TotalVtxCount;
        }

        // resize ib
        if draw_data.TotalIdxCount > buffers.ib_size {
            buffers.ib = create_index_buffer::<D>(device, draw_data.TotalIdxCount)?;
            buffers.ib_size = draw_data.TotalIdxCount;
        }

        // update buffers
        let imgui_cmd_lists =
            std::slice::from_raw_parts(draw_data.CmdLists, draw_data.CmdListsCount as usize);
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

        let mvp: [[f32; 4]; 4] = [
            [2.0 / (r - l), 0.0, 0.0, 0.0],
            [0.0, 2.0 / (t - b), 0.0, 0.0],
            [0.0, 0.0, 0.5, 0.0],
            [(r + l) / (l - r), (t + b) / (b - t), 0.0, 1.0],
        ];

        cmd.set_marker(0xff00ffff, "ImGui");

        let viewport = gfx::Viewport {
            x: 0.0,
            y: 0.0,
            width: draw_data.DisplaySize.x,
            height: draw_data.DisplaySize.y,
            min_depth: 0.0,
            max_depth: 1.0,
        };

        cmd.set_viewport(&viewport);
        cmd.set_vertex_buffer(&buffers.vb, 0);
        cmd.set_index_buffer(&buffers.ib);
        cmd.set_render_pipeline(&pipeline);
        cmd.push_constants(0, 16, 0, &mvp);

        let clip_off = draw_data.DisplayPos;
        let mut global_vtx_offset = 0;
        let mut global_idx_offset = 0;
        for imgui_cmd_list in imgui_cmd_lists {
            let imgui_cmd_buffer = (**imgui_cmd_list).CmdBuffer;
            let imgui_cmd_data =
                std::slice::from_raw_parts(imgui_cmd_buffer.Data, imgui_cmd_buffer.Size as usize);
            let draw_vert = &(*(*imgui_cmd_list)).VtxBuffer;
            let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
            for i in 0..imgui_cmd_buffer.Size as usize {
                let imgui_cmd = &imgui_cmd_data[i];
                if imgui_cmd.UserCallback.is_some() {
                } else {
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
                        bottom: clip_max_y as i32,
                    };

                    cmd.set_render_heap(1, device.get_shader_heap(), font_tex_index);
                    cmd.set_scissor_rect(&scissor);
                    cmd.draw_indexed_instanced(
                        imgui_cmd.ElemCount,
                        1,
                        imgui_cmd.IdxOffset + global_idx_offset,
                        (imgui_cmd.VtxOffset + global_vtx_offset) as i32,
                        0,
                    );
                }
            }
            global_idx_offset += draw_index.Size as u32;
            global_vtx_offset += draw_vert.Size as u32;
        }
        Ok(())
    }
}

impl<D, A> ImGui<D, A> where D: Device, A: App {
    pub fn create(info: &mut ImGuiInfo<D, A>) -> Result<Self, super::Error> {
        unsafe {
            igCreateContext(std::ptr::null_mut());
            let mut io = &mut *igGetIO();

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
                    io.Fonts,
                    null_font_name.as_ptr() as *const i8,
                    16.0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }

            io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard as i32;
            io.KeyMap[ImGuiKey_Tab as usize] = A::get_key_code(os::Key::Tab);
            io.KeyMap[ImGuiKey_LeftArrow as usize] = A::get_key_code(os::Key::Left);
            io.KeyMap[ImGuiKey_RightArrow as usize] =
                A::get_key_code(os::Key::Right);
            io.KeyMap[ImGuiKey_UpArrow as usize] = A::get_key_code(os::Key::Up);
            io.KeyMap[ImGuiKey_DownArrow as usize] = A::get_key_code(os::Key::Down);
            io.KeyMap[ImGuiKey_PageUp as usize] = A::get_key_code(os::Key::PageUp);
            io.KeyMap[ImGuiKey_PageDown as usize] =
                A::get_key_code(os::Key::PageDown);
            io.KeyMap[ImGuiKey_Home as usize] = A::get_key_code(os::Key::Home);
            io.KeyMap[ImGuiKey_End as usize] = A::get_key_code(os::Key::End);
            io.KeyMap[ImGuiKey_Insert as usize] = A::get_key_code(os::Key::Insert);
            io.KeyMap[ImGuiKey_Delete as usize] = A::get_key_code(os::Key::Delete);
            io.KeyMap[ImGuiKey_Backspace as usize] =
                A::get_key_code(os::Key::Backspace);
            io.KeyMap[ImGuiKey_Space as usize] = A::get_key_code(os::Key::Space);
            io.KeyMap[ImGuiKey_Enter as usize] = A::get_key_code(os::Key::Enter);
            io.KeyMap[ImGuiKey_Escape as usize] = A::get_key_code(os::Key::Escape);
            io.KeyMap[ImGuiKey_KeyPadEnter as usize] =
                A::get_key_code(os::Key::KeyPadEnter);
            io.KeyMap[ImGuiKey_A as usize] = 'A' as i32;
            io.KeyMap[ImGuiKey_C as usize] = 'C' as i32;
            io.KeyMap[ImGuiKey_V as usize] = 'V' as i32;
            io.KeyMap[ImGuiKey_X as usize] = 'X' as i32;
            io.KeyMap[ImGuiKey_Y as usize] = 'Y' as i32;
            io.KeyMap[ImGuiKey_Z as usize] = 'Z' as i32;

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
            let mut buffers: Vec<RenderBuffers<D>> = Vec::new();
            let num_buffers = (*info.swap_chain).get_num_buffers();

            let font_tex = create_fonts_texture::<D>(&mut info.device)?;
            let pipeline = create_render_pipeline(info)?;

            for _i in 0..num_buffers {
                buffers.push(RenderBuffers {
                    vb: create_vertex_buffer::<D>(&mut info.device, DEFAULT_VB_SIZE)?,
                    vb_size: DEFAULT_VB_SIZE,
                    ib: create_index_buffer::<D>(&mut info.device, DEFAULT_IB_SIZE)?,
                    ib_size: DEFAULT_IB_SIZE,
                })
            }

            // enum monitors
            let mut monitors: Vec<ImGuiPlatformMonitor> = Vec::new();

            let platform_io = &mut *igGetPlatformIO();
            let os_monitors = A::enumerate_display_monitors();
            for monitor in os_monitors {
                let ig_mon = ImGuiPlatformMonitor {
                    MainPos: ImVec2 {
                        x: monitor.rect.x as f32,
                        y: monitor.rect.y as f32,
                    },
                    MainSize: ImVec2 {
                        x: monitor.rect.width as f32,
                        y: monitor.rect.height as f32,
                    },
                    WorkPos: ImVec2 {
                        x: monitor.client_rect.x as f32,
                        y: monitor.client_rect.y as f32,
                    },
                    WorkSize: ImVec2 {
                        x: monitor.client_rect.width as f32,
                        y: monitor.client_rect.height as f32,
                    },
                    DpiScale: monitor.dpi_scale,
                };
                if monitor.primary {
                    monitors.push(ig_mon)
                } else {
                    monitors.insert(0, ig_mon)
                }
            }

            platform_io.Monitors.Size = monitors.len() as i32;
            platform_io.Monitors.Capacity = monitors.len() as i32;
            platform_io.Monitors.Data = new_monitors(&monitors);

            let vps = &mut *igGetMainViewport();

            // alloc main viewport ViewportData (we obtain from UserData in callbacks)
            let vp = new_viewport_data::<D, A>();
            (*vp).main_viewport = true;
            vps.PlatformUserData = vp as _;
            vps.PlatformHandle = new_native_handle::<A>(info.main_window.get_native_handle()) as _;

            //
            let imgui = ImGui {
                _native_handle: info.main_window.get_native_handle(),
                font_texture: font_tex,
                pipeline: pipeline,
                buffers: buffers,
                last_cursor: os::Cursor::None,
            };

            if (io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
                imgui.setup_platform_interface();
            }

            Ok(imgui)
        }
    }

    fn setup_platform_interface(&self) {
        unsafe {
            let platform_io = &mut *igGetPlatformIO();
            // platform hooks
            platform_io.Platform_CreateWindow = Some(platform_create_window::<D, A>);
            platform_io.Platform_DestroyWindow = Some(platform_destroy_window::<D, A>);
            platform_io.Platform_ShowWindow = Some(platform_show_window::<D, A>);
            platform_io.Platform_SetWindowPos = Some(platform_set_window_pos::<D, A>);
            platform_io.Platform_SetWindowSize = Some(platform_set_window_size::<D, A>);
            platform_io.Platform_SetWindowFocus = Some(platform_set_window_focus::<D, A>);
            platform_io.Platform_GetWindowFocus = Some(platform_get_window_focus::<D, A>);
            platform_io.Platform_GetWindowMinimized = Some(platform_get_window_minimised::<D, A>);
            platform_io.Platform_SetWindowTitle = Some(platform_set_window_title::<D, A>);
            platform_io.Platform_GetWindowDpiScale = Some(platform_get_window_dpi_scale::<D, A>);
            platform_io.Platform_UpdateWindow = Some(platform_update_window::<D, A>);

            // render hooks
            platform_io.Renderer_RenderWindow = Some(renderer_render_window::<D, A>);
            platform_io.Renderer_SwapBuffers = Some(renderer_swap_buffers::<D, A>);

            // need to hook these c-compatible getter funtions due to complex return types
            ImGuiPlatformIO_Set_Platform_GetWindowPos(platform_io, platform_get_window_pos::<D, A>);
            ImGuiPlatformIO_Set_Platform_GetWindowSize(platform_io, platform_get_window_size::<D, A>)
        }
    }

    pub fn new_frame(
        &mut self,
        app: &mut A,
        main_window: &mut A::Window,
        device: &mut D,
    ) {
        let size = main_window.get_size();
        unsafe {
            let io = &mut *igGetIO();

            // gotta pack the refs into a pointer and into UserData for callbacks
            let mut ud = UserData {
                device: device,
                app: app,
                main_window: main_window,
                pipeline: &self.pipeline,
                font_texture: &self.font_texture,
            };
            io.UserData = (&mut ud as *mut UserData<D, A>) as _;

            // update display
            io.DisplaySize = ImVec2 {
                x: size.x as f32,
                y: size.y as f32,
            };

            // update mouse
            if io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32 == 0 {
                // non viewports mouse coords are in client space
                let client_mouse = main_window.get_mouse_client_pos(app.get_mouse_pos());
                io.MousePos = ImVec2::from(client_mouse);
            } else {
                // viewports mouse coords are in screen space
                io.MousePos = ImVec2::from(app.get_mouse_pos());
            }

            // set ImGui mouse hovered viewport
            let platform_io = &mut *igGetPlatformIO();
            let num_vp = platform_io.Viewports.Size;
            let viewports = std::slice::from_raw_parts(platform_io.Viewports.Data, num_vp as usize);

            // find / reset hovered
            io.MouseHoveredViewport = 0;
            for vp in viewports {
                let p_vp = *vp;
                let vp_ref = &*p_vp;
                let win = get_viewport_window::<D, A>(p_vp);
                if win.is_mouse_hovered() {
                    if (vp_ref.Flags & ImGuiViewportFlags_NoInputs as i32) == 0 {
                        io.MouseHoveredViewport = vp_ref.ID;
                    }
                }
            }

            // update mouse
            io.MouseWheel = app.get_mouse_wheel();
            io.MouseWheelH = app.get_mouse_wheel();
            io.MouseDown = app.get_mouse_buttons();

            // update char inputs
            let utf16 = app.get_utf16_input();
            for u in utf16 {
                ImGuiIO_AddInputCharacterUTF16(io, u);
            }

            // update keyboard
            let keys_down = app.get_keys_down();
            std::ptr::copy_nonoverlapping(
                &keys_down as *const bool,
                &mut io.KeysDown as *mut bool,
                256,
            );
            io.KeyCtrl = app.is_sys_key_down(os::SysKey::Ctrl);
            io.KeyShift = app.is_sys_key_down(os::SysKey::Shift);
            io.KeyAlt = app.is_sys_key_down(os::SysKey::Alt);

            // update mouse cursor

            // Update OS mouse cursor with the cursor requested by imgui
            let cursor = if io.MouseDrawCursor {
                to_os_cursor(igGetMouseCursor())
            } else {
                os::Cursor::None
            };

            if self.last_cursor != cursor {
                self.last_cursor = cursor;
                app.set_cursor(&self.last_cursor);
            }

            igNewFrame();

            io.UserData = std::ptr::null_mut();
        }
    }

    pub fn render(
        &mut self,
        app: &mut A,
        main_window: &mut A::Window,
        device: &mut D,
        cmd: &mut D::CmdBuf,
    ) {
        unsafe {
            let io = &mut *igGetIO();
            igRender();

            render_draw_data::<D>(
                &*igGetDrawData(),
                device,
                cmd,
                &mut self.buffers,
                &self.pipeline,
                &self.font_texture,
            )
            .unwrap();

            if (io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
                // gotta pack the refs into a pointer and into io.UserData
                let mut ud = UserData {
                    device: device,
                    app: app,
                    main_window: main_window,
                    pipeline: &self.pipeline,
                    font_texture: &self.font_texture,
                };
                io.UserData = (&mut ud as *mut UserData<D, A>) as _;

                igUpdatePlatformWindows();
                igRenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());

                io.UserData = std::ptr::null_mut();
            }
        }
    }

    pub fn demo(&self) {
        unsafe {
            static mut SHOW_DEMO_WINDOW: bool = true;
            static mut SHOW_ANOTHER_WINDOW: bool = true;
            static mut CLEAR_COLOUR: [f32; 3] = [0.0, 0.0, 0.0];

            let io = &mut *igGetIO();

            if SHOW_DEMO_WINDOW {
                igShowDemoWindow(&mut SHOW_DEMO_WINDOW);
            }

            // 2. Show a simple window that we create ourselves. We use a Begin/End pair to created a named window.
            {
                static mut SLIDER_FLOAT: f32 = 0.0;
                static mut COUNTER: i32 = 0;

                let mut open = true;

                // Create a window called "Hello, world!" and append into it.
                igBegin(
                    "Hello, world!\0".as_ptr() as *const i8,
                    &mut open,
                    ImGuiWindowFlags_None as i32,
                );

                igText(
                    "%s\0".as_ptr() as *const i8,
                    "This is some useful text.\0".as_ptr() as *const i8,
                );

                igCheckbox("Demo Window\0".as_ptr() as *const i8, &mut SHOW_DEMO_WINDOW);
                igCheckbox("Another Window\0".as_ptr() as *const i8, &mut SHOW_ANOTHER_WINDOW);

                igText(
                    "%f, %f : %f %f\0".as_ptr() as *const i8,
                    io.MousePos.x as f64,
                    io.MousePos.y as f64,
                    io.DisplaySize.x as f64,
                    io.DisplaySize.y as f64,
                );

                igSliderFloat(
                    "float\0".as_ptr() as _,
                    &mut SLIDER_FLOAT,
                    0.0,
                    1.0,
                    "%.3f\0".as_ptr() as _,
                    0,
                );

                if igButton("Button\0".as_ptr() as _, ImVec2 { x: 0.0, y: 0.0 }) {
                    COUNTER += 1;
                }

                igSameLine(0.0, -1.0);

                igText(
                    "counter = %i\0".as_ptr() as *const i8,
                    "This is some useful text.\0".as_ptr() as *const i8,
                );

                igColorEdit3("clear color\0".as_ptr() as _, CLEAR_COLOUR.as_mut_ptr(), 0); // Edit 3 floats representing a color

                igText(
                    "Application average %.3f ms/frame (%.1f FPS)\0".as_ptr() as _,
                    1000.0 / io.Framerate as f64,
                    io.Framerate as f64,
                );

                igEnd();
            }

            // 3. Show another simple window.
            if SHOW_ANOTHER_WINDOW {
                igBegin(
                    "Another Window\0".as_ptr() as *const i8,
                    &mut SHOW_ANOTHER_WINDOW,
                    ImGuiWindowFlags_None as i32,
                );

                igText(
                    "%s\0".as_ptr() as *const i8,
                    "Hello from another window!\0".as_ptr() as *const i8,
                );

                if igButton("Close Me\0".as_ptr() as _, ImVec2 { x: 0.0, y: 0.0 }) {
                    SHOW_ANOTHER_WINDOW = false;
                }

                igEnd();
            }
        }
    }
}

impl<D, A> Drop for ImGui<D, A> where D: Device, A: App {
    fn drop(&mut self) {
        unsafe {
            igDestroyPlatformWindows();
            let platform_io = &mut *igGetPlatformIO();
            std::ptr::drop_in_place(platform_io.Monitors.Data as *mut ImGuiPlatformMonitor);
            platform_io.Monitors.Data = std::ptr::null_mut();
        }
    }
}

impl From<os::Point<i32>> for ImVec2 {
    fn from(point: os::Point<i32>) -> ImVec2 {
        ImVec2 {
            x: point.x as f32,
            y: point.y as f32,
        }
    }
}

impl From<ImVec2> for os::Point<i32> {
    fn from(vec2: ImVec2) -> os::Point<i32> {
        os::Point {
            x: vec2.x as i32,
            y: vec2.y as i32,
        }
    }
}

/// handles the case where we can return an imgui created window from ViewportData, or borrow the main window
/// from UserData
fn get_viewport_window<'a, D: Device, A: App>(vp: *mut ImGuiViewport) -> &'a mut A::Window {
    unsafe {
        let vp_ref = &mut *vp;
        let vd = &mut *(vp_ref.PlatformUserData as *mut ViewportData<D, A>);
        if vd.main_viewport {
            let io = &mut *igGetIO();
            let ud = &mut *(io.UserData as *mut UserData<D, A>);
            return ud.main_window;
        }
        return &mut vd.window[0];
    }
}

/// get a hotline::imgui::ViewportData mutable reference from an ImGuiViewport
fn get_viewport_data<'a, D: Device, A: App>(vp: *mut ImGuiViewport) -> &'a mut ViewportData<D, A> {
    unsafe {
        let vp_ref = &mut *vp;
        let vd = &mut *(vp_ref.PlatformUserData as *mut ViewportData<D, A>);
        vd
    }
}

/// Get the UserData packed inside ImGui.io to access Device, App and main Window
fn get_user_data<'a, D: Device, A: App>() -> &'a mut UserData<'a, D, A> {
    unsafe {
        let io = &mut *igGetIO();
        let ud = &mut *(io.UserData as *mut UserData<D, A>);
        ud
    }
}

unsafe extern "C" fn platform_create_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let io = &mut *igGetIO();
    let ud = &mut *(io.UserData as *mut UserData<D, A>);
    let mut device = &mut ud.device;
    let mut vp_ref = &mut *vp;

    // alloc viewport data
    let p_vd = new_viewport_data::<D, A>();
    let vd = &mut *p_vd;

    // find parent
    let mut parent_handle = None;
    if vp_ref.ParentViewportId != 0 {
        let parent = &*igFindViewportByID(vp_ref.ParentViewportId);
        let nh = &*(parent.PlatformHandle as *mut A::NativeHandle);
        parent_handle = Some(nh.copy());
    }

    // create a window
    vd.window = vec![ud.app.create_window(os::WindowInfo {
        title: String::from("Utitled"),
        rect: os::Rect {
            x: vp_ref.Pos.x as i32,
            y: vp_ref.Pos.y as i32,
            width: vp_ref.Size.x as i32,
            height: vp_ref.Size.y as i32,
        },
        style: os::WindowStyleFlags::from(vp_ref.Flags),
        parent_handle: parent_handle,
    })];

    // create cmd buffer
    vd.cmd = vec![device.create_cmd_buf(2)];

    // create swap chain and bind to window
    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };
    vd.swap_chain = vec![device.create_swap_chain::<A>(&swap_chain_info, &vd.window[0])];

    // create render buffers
    let mut buffers: Vec<RenderBuffers<D>> = Vec::new();
    let num_buffers = vd.swap_chain[0].get_num_buffers();
    for _i in 0..num_buffers {
        buffers.push(RenderBuffers {
            vb: create_vertex_buffer::<D>(&mut device, DEFAULT_VB_SIZE).unwrap(),
            vb_size: DEFAULT_VB_SIZE,
            ib: create_index_buffer::<D>(&mut device, DEFAULT_IB_SIZE).unwrap(),
            ib_size: DEFAULT_IB_SIZE,
        })
    }
    vd.buffers = buffers;

    // track the viewport user data pointer
    vp_ref.PlatformUserData = p_vd as *mut _;
    vp_ref.PlatformRequestResize = false;
    vp_ref.PlatformHandle = new_native_handle::<A>(vd.window[0].get_native_handle()) as _;
}

unsafe extern "C" fn platform_destroy_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let vd = get_viewport_data::<D, A>(vp);
    let mut vp_ref = &mut *vp;

    if vd.swap_chain.len() > 0 {
        vd.swap_chain[0].wait_for_last_frame();
    }

    vd.swap_chain.clear();
    vd.cmd.clear();
    vd.buffers.clear();
    vd.window.clear();

    // drop and null allocated data
    std::ptr::drop_in_place(vp_ref.PlatformUserData as *mut ViewportData<D, A>);
    vp_ref.PlatformUserData = std::ptr::null_mut();
    std::ptr::drop_in_place(vp_ref.PlatformHandle as *mut A::NativeHandle);
    vp_ref.PlatformHandle = std::ptr::null_mut();
}

unsafe extern "C" fn platform_update_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let window = get_viewport_window::<D, A>(vp);
    let mut vp_ref = &mut *vp;
    window.update(get_user_data::<D, A>().app);
    window.update_style(
        os::WindowStyleFlags::from(vp_ref.Flags),
        os::Rect {
            x: vp_ref.Pos.x as i32,
            y: vp_ref.Pos.y as i32,
            width: vp_ref.Size.x as i32,
            height: vp_ref.Size.y as i32,
        },
    );
    let events = window.get_events();
    if events.contains(os::WindowEventFlags::CLOSE) {
        vp_ref.PlatformRequestClose = true;
    }
    if events.contains(os::WindowEventFlags::MOVE) {
        vp_ref.PlatformRequestMove = true;
    }
    if events.contains(os::WindowEventFlags::SIZE) {
        vp_ref.PlatformRequestResize = true;
    }
    window.clear_events();
}

unsafe extern "C" fn platform_get_window_pos<D: Device, A: App>(vp: *mut ImGuiViewport, out_pos: *mut ImVec2) {
    let window = get_viewport_window::<D, A>(vp);
    let pos = window.get_pos();
    (*out_pos).x = pos.x as f32;
    (*out_pos).y = pos.y as f32;
}

unsafe extern "C" fn platform_get_window_size<D: Device, A: App>(vp: *mut ImGuiViewport, out_size: *mut ImVec2) {
    let window = get_viewport_window::<D, A>(vp);
    let size = window.get_size();
    (*out_size).x = size.x as f32;
    (*out_size).y = size.y as f32;
}

unsafe extern "C" fn platform_show_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let window = get_viewport_window::<D, A>(vp);
    let activate = if (*vp).Flags & ImGuiViewportFlags_NoFocusOnAppearing as i32 != 0 {
        false
    } else {
        true
    };
    window.show(true, activate);
}

unsafe extern "C" fn platform_set_window_title<D: Device, A: App>(vp: *mut ImGuiViewport, str_: *const cty::c_char) {
    let win = get_viewport_window::<D, A>(vp);
    let cstr = CStr::from_ptr(str_);
    win.set_title(String::from(cstr.to_str().unwrap()));
}

unsafe extern "C" fn platform_set_window_focus<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let window = get_viewport_window::<D, A>(vp);
    window.set_focused();
}

unsafe extern "C" fn platform_get_window_focus<D: Device, A: App>(vp: *mut ImGuiViewport) -> bool {
    let window = get_viewport_window::<D, A>(vp);
    window.is_focused()
}

unsafe extern "C" fn platform_set_window_pos<D: Device, A: App>(vp: *mut ImGuiViewport, pos: ImVec2) {
    let window = get_viewport_window::<D, A>(vp);
    window.set_pos(os::Point::from(pos));
}

unsafe extern "C" fn platform_set_window_size<D: Device, A: App>(vp: *mut ImGuiViewport, size: ImVec2) {
    let window = get_viewport_window::<D, A>(vp);
    window.set_size(os::Size::from(size));
}

unsafe extern "C" fn platform_get_window_minimised<D: Device, A: App>(vp: *mut ImGuiViewport) -> bool {
    let window = get_viewport_window::<D, A>(vp);
    window.is_minimised()
}

unsafe extern "C" fn platform_get_window_dpi_scale<D: Device, A: App>(vp: *mut ImGuiViewport) -> f32 {
    let window = get_viewport_window::<D, A>(vp);
    window.get_dpi_scale()
}

unsafe extern "C" fn renderer_render_window<D: Device, A: App>(vp: *mut ImGuiViewport, _render_arg: *mut cty::c_void) {
    let ud = get_user_data::<D, A>();
    let vd = get_viewport_data::<D, A>(vp);
    let vp_ref = &*vp;

    // must be an imgui created window
    assert_ne!(vd.window.len(), 0);
    assert_ne!(vd.cmd.len(), 0);
    assert_ne!(vd.swap_chain.len(), 0);

    // unpack from vec
    let window = &mut vd.window[0];
    let mut cmd = &mut vd.cmd[0];
    let swap = &mut vd.swap_chain[0];
    let vp_rect = window.get_viewport_rect();

    // update
    window.update(ud.app);
    swap.update::<A>(&mut ud.device, &window, &mut cmd);
    cmd.reset(&swap);

    // render
    let viewport = gfx::Viewport::from(vp_rect);
    let scissor = gfx::ScissorRect::from(vp_rect);

    // TODO:
    cmd.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(swap.get_backbuffer_texture().clone_inner()),
        buffer: None,
        state_before: gfx::ResourceState::Present,
        state_after: gfx::ResourceState::RenderTarget,
    });

    let mut pass = swap.get_backbuffer_pass_mut();
    cmd.begin_render_pass(&mut pass);

    cmd.set_viewport(&viewport);
    cmd.set_scissor_rect(&scissor);

    render_draw_data::<D>(
        &*vp_ref.DrawData,
        &mut ud.device,
        &mut cmd,
        &mut vd.buffers,
        &ud.pipeline,
        &ud.font_texture,
    )
    .unwrap();

    cmd.end_render_pass();

    // TODO:
    cmd.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(swap.get_backbuffer_texture().clone_inner()),
        buffer: None,
        state_before: gfx::ResourceState::RenderTarget,
        state_after: gfx::ResourceState::Present,
    });

    cmd.close(&swap);

    ud.device.execute(cmd);
}

unsafe extern "C" fn renderer_swap_buffers<D: Device, A: App>(vp: *mut ImGuiViewport, _render_arg: *mut cty::c_void) {
    let ud = get_user_data::<D, A>();
    let vd = get_viewport_data::<D, A>(vp);
    assert_ne!(vd.swap_chain.len(), 0);
    vd.swap_chain[0].swap(&ud.device);
}

pub type WindowSizeCallback = unsafe extern "C" fn(vp: *mut ImGuiViewport, out_pos: *mut ImVec2);

extern "C" {
    pub fn ImGuiPlatformIO_Set_Platform_GetWindowPos(
        platform_io: *mut ImGuiPlatformIO,
        function: WindowSizeCallback,
    );

    pub fn ImGuiPlatformIO_Set_Platform_GetWindowSize(
        platform_io: *mut ImGuiPlatformIO,
        function: WindowSizeCallback,
    );
}

impl From<ImGuiViewportFlags> for os::WindowStyleFlags {
    fn from(flags: ImGuiViewportFlags) -> os::WindowStyleFlags {
        let mut style = os::WindowStyleFlags::IMGUI;
        if (flags & ImGuiViewportFlags_NoDecoration as i32) != 0 {
            style |= os::WindowStyleFlags::POPUP;
        } else {
            style |= os::WindowStyleFlags::OVERLAPPED_WINDOW;
        }
        if (flags & ImGuiViewportFlags_NoTaskBarIcon as i32) != 0 {
            style |= os::WindowStyleFlags::TOOL_WINDOW;
        } else {
            style |= os::WindowStyleFlags::APP_WINDOW;
        }
        if (flags & ImGuiViewportFlags_TopMost as i32) != 0 {
            style |= os::WindowStyleFlags::TOPMOST;
        }
        style
    }
}

#[allow(non_upper_case_globals)]
fn to_os_cursor(cursor: ImGuiMouseCursor) -> os::Cursor {
    match cursor {
        ImGuiMouseCursor_Arrow => os::Cursor::Arrow,
        ImGuiMouseCursor_TextInput => os::Cursor::TextInput,
        ImGuiMouseCursor_ResizeAll => os::Cursor::ResizeAll,
        ImGuiMouseCursor_ResizeEW => os::Cursor::ResizeEW,
        ImGuiMouseCursor_ResizeNS => os::Cursor::ResizeNS,
        ImGuiMouseCursor_ResizeNESW => os::Cursor::ResizeNESW,
        ImGuiMouseCursor_ResizeNWSE => os::Cursor::ResizeNWSE,
        ImGuiMouseCursor_Hand => os::Cursor::Hand,
        ImGuiMouseCursor_NotAllowed => os::Cursor::NotAllowed,
        _ => os::Cursor::None,
    }
}
