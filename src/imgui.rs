use imgui_sys::*;

use crate::gfx::Heap;
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

use maths_rs::Vec4f;

use crate::static_ref;
use crate::static_ref_mut;
use crate::static_ref_array_mut;

fn to_im_vec4(v: Vec4f) -> ImVec4 {
    unsafe {
        std::mem::transmute(v)
    }
}

const DEFAULT_VB_SIZE: i32 = 5000;
const DEFAULT_IB_SIZE: i32 = 10000;
const MAX_RANGES: usize = 32;

const MAIN_DOCKSPACE_FLAGS : u32 = ImGuiWindowFlags_NoTitleBar |
    ImGuiWindowFlags_NoCollapse |
    ImGuiWindowFlags_NoResize |
    ImGuiWindowFlags_NoMove |
    ImGuiWindowFlags_NoBringToFrontOnFocus |
    ImGuiWindowFlags_NoNavFocus |
    ImGuiWindowFlags_MenuBar |
    ImGuiWindowFlags_NoScrollbar;

const IMVEC2_ZERO : ImVec2 = ImVec2 {x: 0.0, y: 0.0 };
const MAIN_DOCK_NAME : *const i8 = "main_dock\0".as_ptr() as *const i8;
const STATUS_BAR_NAME : *const i8 = "status_bar\0".as_ptr() as *const i8;

/// Info to supply fonts from .ttf files for use with imgui
pub struct FontInfo {
    /// filepath to a .ttf file
    pub filepath: String,
    /// optional to specify ranges, which are wide char u32 code points. defaults to basic latin & extended latin
    pub glyph_ranges: Option<Vec<[u32; 2]>>
}

/// Info required to create an instance of imgui
pub struct ImGuiInfo<'stack, D: Device, A: App> {
    pub device: &'stack mut D,
    pub swap_chain: &'stack mut D::SwapChain,
    pub main_window: &'stack A::Window,
    pub fonts: Vec<FontInfo>,
}

/// The concrete `ImGui` instance itself
pub struct ImGui<D: Device, A: App> {
    _native_handle: A::NativeHandle,
    _font_texture: D::Texture,
    pipeline: D::RenderPipeline,
    buffers: Vec<RenderBuffers<D>>,
    last_cursor: os::Cursor
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
    image_heaps: &'a Vec<&'a D::Heap>
}

/// Trait for hooking into imgui ui calls into other modules
pub trait UserInterface<D: gfx::Device, A: os::App> {
    fn show_ui(&mut self, imgui: &mut ImGui<D, A>, open: bool) -> bool;
}

bitflags! {
    pub struct WindowFlags : i32 {
        const NONE = 0;
        const NO_TITLE_BAR = 1 << 0;
        const NO_RESIZE = 1 << 1;
        const NO_MOVE = 1 << 2;
        const NO_SCROLLBAR = 1 << 3;
        const NO_SCROLL_WITH_MOUSE = 1 << 4;
        const NO_COLLAPSE = 1 << 5;
        const ALWAYS_AUTO_RESIZE = 1 << 6;
        const NO_BACKGROUND = 1 << 7;
        const NO_SAVED_SETTINGS = 1 << 8;
        const NO_MOUSE_INPUTS = 1 << 9;
        const MENU_BAR = 1 << 10;
        const HORIZONTAL_SCROLLBAR = 1 << 11;
        const NO_FOCUS_ON_APPEARING = 1 << 12;
        const NO_BRING_TO_FRONT_ON_FOCUS = 1 << 13;
        const ALWAYS_VERTICAL_SCROLLBAR = 1 << 14;
        const ALWAYS_HORIZONTAL_SCROLLBAR = 1 << 15;
        const ALWAYS_USE_WINDOW_PADDING = 1 << 16;
        const NO_NAV_INPUTS = 1 << 18;
        const NO_NAV_FOCUS = 1 << 19;
        const UNSAVED_DOCUMENT = 1 << 20;
        const NO_DOCKING = 1 << 21;
        const NO_NAV = (1 << 18 | 1 << 19);
        const NO_DECORATION = (1 << 0 | 1 << 1 | 1 << 3 | 1 << 5);
        const NO_INPUTS = (1 << 9 | 1 << 18 | 1 << 19);
    }
}

impl From<WindowFlags> for i32 {
    fn from(mask: WindowFlags) -> i32 {
        mask.bits
    }
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

fn new_ranges() -> *mut [u32; (MAX_RANGES * 2) + 1] {
    unsafe {
        let layout = std::alloc::Layout::from_size_align(std::mem::size_of::<[u32; (MAX_RANGES * 2) + 1]>(), 8).unwrap();
        std::alloc::alloc_zeroed(layout) as *mut [u32; (MAX_RANGES * 2) + 1]
    }
}

fn to_imgui_texture_id<D: Device>(tex: &D::Texture) -> ImTextureID {
    unsafe {
        let srv = tex.get_srv_index().unwrap() as u64;
        let heap_id = tex.get_shader_heap_id().unwrap() as u64;

        // packs them in so the first 48bits are for srv
        // top 16 bits are heap id

        // mask off 48 bits 0x
        let mask = 0x0000ffffffffffff;
        let combined = (srv & mask) | ((heap_id << 48) & !mask);
        let tex_id : *mut cty::c_void = std::ptr::null_mut();
        tex_id.add(combined as usize)
    }
}

fn to_srv_heap_id(tex_id: *mut cty::c_void) -> (usize, u16) {
    let mask = 0x0000ffffffffffff;
    let srv_id = (tex_id as u64) & mask;
    let heap_id = ((tex_id as u64) & !mask) >> 48;
    (srv_id as usize, heap_id as u16)
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
            array_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: gfx::TextureUsage::SHADER_RESOURCE,
            initial_state: gfx::ResourceState::ShaderResource,
        };

        device.create_texture(&tex_info, Some(data_slice))
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

    device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: Some(&vs),
        fs: Some(&fs),
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
        pipeline_layout: gfx::PipelineLayout {
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
            static_samplers: Some(vec![gfx::SamplerBinding {
                visibility: gfx::ShaderVisibility::Fragment,
                shader_register: 0,
                register_space: 0,
                sampler_info: gfx::SamplerInfo {
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
            }}]),
        },
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
        pass: Some(swap_chain.get_backbuffer_pass()),
        ..Default::default()
    })
}

fn create_vertex_buffer<D: Device>(
    device: &mut D,
    size: i32,
) -> Result<D::Buffer, super::Error> {
    device.create_buffer::<u8>(
        &gfx::BufferInfo {
            usage: gfx::BufferUsage::VERTEX,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVert>(),
            num_elements: size as usize,
            initial_state: gfx::ResourceState::VertexConstantBuffer
        },
        None,
    )
}

fn create_index_buffer<D: Device>(
    device: &mut D,
    size: i32,
) -> Result<D::Buffer, super::Error> {
    device.create_buffer::<u8>(
        &gfx::BufferInfo {
            usage: gfx::BufferUsage::INDEX,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<ImDrawIdx>(),
            num_elements: size as usize,
            initial_state: gfx::ResourceState::IndexBuffer
        },
        None,
    )
}

fn render_draw_data<D: Device>(
    draw_data: &ImDrawData,
    device: &mut D,
    cmd: &mut D::CmdBuf,
    image_heaps: &Vec<&D::Heap>,
    buffers: &mut [RenderBuffers<D>],
    pipeline: &D::RenderPipeline,
) -> Result<(), super::Error> where D::RenderPipeline: gfx::Pipeline {
    unsafe {
        if draw_data.CmdListsCount > 0 && draw_data.TotalVtxCount > 0 {
            let bb = cmd.get_backbuffer_index() as usize;

            let buffers = &mut buffers[bb];

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
                vertex_write_offset += vb_size_bytes;
                // index
                let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
                let ib_size_bytes = draw_index.Size as usize * std::mem::size_of::<ImDrawIdx>();
                let ib_slice = std::slice::from_raw_parts(draw_index.Data, draw_index.Size as usize);
                buffers.ib.update(index_write_offset, ib_slice)?;
                index_write_offset += ib_size_bytes;
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

            let viewport = gfx::Viewport {
                x: 0.0,
                y: 0.0,
                width: draw_data.DisplaySize.x,
                height: draw_data.DisplaySize.y,
                min_depth: 0.0,
                max_depth: 1.0,
            };

            cmd.begin_event(0xff1fb6c4, "imgui");
            cmd.set_viewport(&viewport);
            cmd.set_vertex_buffer(&buffers.vb, 0);
            cmd.set_index_buffer(&buffers.ib);
            cmd.set_render_pipeline(pipeline);
            cmd.push_render_constants(0, 16, 0, &mvp);

            let clip_off = draw_data.DisplayPos;
            let mut global_vtx_offset = 0;
            let mut global_idx_offset = 0;
            for imgui_cmd_list in imgui_cmd_lists {
                let imgui_cmd_buffer = (**imgui_cmd_list).CmdBuffer;
                let imgui_cmd_data =
                    std::slice::from_raw_parts(imgui_cmd_buffer.Data, imgui_cmd_buffer.Size as usize);
                let draw_vert = &(*(*imgui_cmd_list)).VtxBuffer;
                let draw_index = &(*(*imgui_cmd_list)).IdxBuffer;
                for cmd_data in imgui_cmd_data.iter().take(imgui_cmd_buffer.Size as usize) {
                    let imgui_cmd = &cmd_data;
                    if imgui_cmd.UserCallback.is_some() {
                        // TODO:
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
                            bottom: clip_max_y as i32,
                        };

                        let (srv, heap_id) = to_srv_heap_id(imgui_cmd.TextureId);
                        if heap_id == device.get_shader_heap().get_heap_id() {
                            // bind the device heap
                            cmd.set_binding(pipeline, device.get_shader_heap(), 1, srv);
                        }
                        else {
                            // bund srv in another heap
                            for heap in image_heaps {
                                if heap.get_heap_id() == heap_id {
                                    cmd.set_binding(pipeline, heap, 1, srv);
                                    break;
                                }
                            }
                        }

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
            cmd.end_event();
        }
        Ok(())
    }
}

impl<D, A> ImGui<D, A> where D: Device, A: App, D::RenderPipeline: gfx::Pipeline {
    /// Internal utility for styling hotline colours in a hardcoded fashion
    fn style_colours_hotline() {
        unsafe {
            igStyleColorsDark(std::ptr::null_mut());

            let style = &mut *igGetStyle();
            style.Colors[ImGuiCol_WindowBg as usize].w = 1.0;
            style.WindowRounding = 2.0;
            style.TabRounding = 2.0;

            let colors = &mut style.Colors;

            let hl = [
                // sand
                ImVec4{x: 251.0/255.0, y: 211.0/255.0, z: 122.0/255.0, w: 1.0},
                ImVec4{x: 191.0/255.0, y: 161.0/255.0, z: 93.0/255.0, w: 1.0},

                // mauve
                ImVec4{x: 179.0/255.0, y:  85.0/255.0, z: 149.0/255.0, w: 1.0},
                ImVec4{x: 131.0/255.0, y:  61.0/255.0, z: 109.0/255.0, w: 1.0},
                ImVec4{x:  93.0/255.0, y:  50.0/255.0, z:  79.0/255.0, w: 1.0},

                // darker sand
                ImVec4{x: 251.0/255.0, y: 180.0/255.0, z: 93.0/255.0, w: 1.0},
            ];

            let bg = [
                // warm grey
                ImVec4{x: 0.31, y: 0.305, z: 0.30, w: 1.0},
                ImVec4{x: 0.21, y: 0.205, z: 0.21, w: 1.0},
                ImVec4{x: 0.15, y: 0.150, z: 0.15, w: 1.0},
                ImVec4{x: 0.11, y: 0.105, z: 0.10, w: 1.0},
            ];

            colors[ImGuiCol_WindowBg as usize] = bg[3];
            colors[ImGuiCol_Header as usize] = hl[3];
            colors[ImGuiCol_HeaderActive as usize] = hl[1];
            colors[ImGuiCol_Button as usize] = hl[4];
            colors[ImGuiCol_ButtonHovered as usize] = hl[1];
            colors[ImGuiCol_ButtonActive as usize] = hl[0];
            colors[ImGuiCol_FrameBg as usize] = bg[1];
            colors[ImGuiCol_FrameBgHovered as usize] = bg[0];
            colors[ImGuiCol_FrameBgActive as usize] = bg[2];
            colors[ImGuiCol_Tab as usize] = hl[3];
            colors[ImGuiCol_TabHovered as usize] = hl[5];
            colors[ImGuiCol_TabActive as usize] = hl[1];
            colors[ImGuiCol_TabUnfocused as usize] = bg[2];
            colors[ImGuiCol_TabUnfocusedActive as usize] = hl[3];
            colors[ImGuiCol_TitleBg as usize] = bg[2];
            colors[ImGuiCol_TitleBgActive as usize] = hl[4];
            colors[ImGuiCol_TitleBgCollapsed as usize] = bg[2];

            //
            colors[ImGuiCol_CheckMark as usize] = hl[1];
            colors[ImGuiCol_SliderGrab as usize] = hl[1];
            colors[ImGuiCol_SliderGrabActive as usize] = hl[0];
            colors[ImGuiCol_HeaderHovered as usize] = hl[1];
            colors[ImGuiCol_ResizeGrip as usize] = bg[0];
            colors[ImGuiCol_ResizeGripActive as usize] = hl[0];
            colors[ImGuiCol_ResizeGripHovered as usize] = hl[1];
       }
    }

    /// Create a new `ImGui` instance from `ImGuiInfo`
    pub fn create(info: &mut ImGuiInfo<D, A>) -> Result<Self, super::Error> {
        unsafe {
            igCreateContext(std::ptr::null_mut());
            let io = &mut *igGetIO();

            io.ConfigFlags |= ImGuiConfigFlags_DockingEnable as i32;
            io.ConfigFlags |= ImGuiConfigFlags_ViewportsEnable as i32;

            // construct path for ini to be along side the exe
            let exe_path = std::env::current_exe().ok().unwrap();
            if let Some(parent) = exe_path.parent() {
                // create static here, for persistent pointer
                let ini_file = parent.join("imgui.ini");
                static mut NULL_INI_FILE : Option<CString> = None;
                NULL_INI_FILE = Some(CString::new(ini_file.to_str().unwrap().to_string()).unwrap());
                if let Some(i) = static_ref!(NULL_INI_FILE) {
                    io.IniFilename = i.as_ptr() as _;
                }
            };
        
            Self::style_colours_hotline();

            let style = &mut *igGetStyle();
            style.WindowRounding = 0.0;
            style.Colors[imgui_sys::ImGuiCol_WindowBg as usize].w = 1.0;

            // add fonts
            let mut merge = false;

            let mut font_ranges = Vec::new();
            let mut font_ranges_alloc = Vec::new();
            let mut font_names = Vec::new();

            for font in &info.fonts {
                let names_back = font_names.len();
                font_names.push(CString::new(font.filepath.clone()).unwrap());

                let config = ImFontConfig_ImFontConfig();
                (*config).MergeMode = merge;

                // copy over the font ranges          
                let null_term_ranges = new_ranges();
                let mut itr = 0;
                
                if let Some(ranges) = &font.glyph_ranges {
                    // we alloc a fixed sized array on the heap for imgui glyph ranges
                    // if you have more ranges that size will need increasing
                    assert!(ranges.len() < MAX_RANGES);
                    for range in ranges {
                        (*null_term_ranges)[itr] = range[0];
                        (*null_term_ranges)[itr+1] = range[1];
                        itr += 2;
                    }
                }
                font_ranges.push(null_term_ranges);
                
                // pass ranges or null
                let p_ranges = if font.glyph_ranges.is_some() {
                    null_term_ranges as *mut u32
                }
                else {
                    std::ptr::null_mut()
                };
                font_ranges_alloc.push(p_ranges);

                ImFontAtlas_AddFontFromFileTTF(
                    io.Fonts,
                    font_names[names_back].as_ptr() as *const i8,
                    16.0,
                    config,
                    p_ranges
                );

                // subsequent fonts are merged
                merge = true;
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

            let font_tex = create_fonts_texture::<D>(info.device)?;

            let font_tex_id = to_imgui_texture_id::<D>(&font_tex);
            ImFontAtlas_SetTexID(io.Fonts, font_tex_id);

            let pipeline = create_render_pipeline(info)?;

            for _i in 0..num_buffers {
                buffers.push(RenderBuffers {
                    vb: create_vertex_buffer::<D>(info.device, DEFAULT_VB_SIZE)?,
                    vb_size: DEFAULT_VB_SIZE,
                    ib: create_index_buffer::<D>(info.device, DEFAULT_IB_SIZE)?,
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
                _font_texture: font_tex,
                pipeline,
                buffers,
                last_cursor: os::Cursor::None
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

    /// Call this each frame before you want to start making `imgui.` UI calls
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
                device,
                app,
                main_window,
                pipeline: &self.pipeline,
                image_heaps: &Vec::new()
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

            assert_ne!(platform_io.Viewports.Data, std::ptr::null_mut());

            let viewports = std::slice::from_raw_parts(platform_io.Viewports.Data, num_vp as usize);

            // find / reset hovered
            io.MouseHoveredViewport = 0;
            for vp in viewports {
                let p_vp = *vp;
                let vp_ref = &*p_vp;
                if !vp_ref.PlatformUserData.is_null() {
                    let win = get_viewport_window::<D, A>(p_vp);
                    if win.is_mouse_hovered() && (vp_ref.Flags & ImGuiViewportFlags_NoInputs as i32) == 0 {
                        io.MouseHoveredViewport = vp_ref.ID;
                    }
                }
            }

            // update mouse
            io.MouseWheel = app.get_mouse_wheel();
            //io.MouseWheelH = app.get_mouse_wheel();
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

    /// Adds a dockspace to the main window starting at 0,0 running to width, height-status_bar_height
    /// if status bar height is 0.0 then the whole window will be filled
    pub fn add_main_dock(&mut self, status_bar_height: f32) {
        unsafe {
            // set main window as dockspace
            let main_viewport = &*igGetMainViewport();

            let size = ImVec2 {
                x: main_viewport.Size.x,
                y: main_viewport.Size.y - status_bar_height
            };

            igSetNextWindowPos(main_viewport.Pos, 0, IMVEC2_ZERO);
            igSetNextWindowSize(size, 0);
            igSetNextWindowViewport(main_viewport.ID);

            igPushStyleVarFloat(ImGuiStyleVar_WindowRounding as i32, 0.0);
            igPushStyleVarFloat(ImGuiStyleVar_WindowBorderSize as i32, 0.0);
            igPushStyleVarVec2(ImGuiStyleVar_WindowPadding as i32, IMVEC2_ZERO);

            let dockspace_name = "main_window_dockspace\0".as_ptr() as *const i8;

            let mut open = true;
            igBegin(dockspace_name, &mut open, MAIN_DOCKSPACE_FLAGS as i32);

            let id = igGetIDStr(dockspace_name);

            igDockSpace(id, ImVec2{x: 0.0, y: 0.0}, ImGuiDockNodeFlags_PassthruCentralNode as i32, std::ptr::null_mut());

            igPopStyleVar(3);
            igEnd();

            // create main dock area
            igPushStyleVarFloat(ImGuiStyleVar_ChildRounding as i32, 0.0);
            igPushStyleVarFloat(ImGuiStyleVar_ChildBorderSize as i32, 0.0);
            igPushStyleVarFloat(ImGuiStyleVar_WindowRounding as i32, 0.0);
            igPushStyleVarFloat(ImGuiStyleVar_WindowBorderSize as i32, 0.0);
            igPushStyleVarVec2(ImGuiStyleVar_WindowPadding as i32, IMVEC2_ZERO);

            let window_class = ImGuiWindowClass {
                DockNodeFlagsOverrideSet: ImGuiDockNodeFlags_NoTabBar,
                ..Default::default()
            };

            /*
            igSetNextWindowClass(&window_class);

            igBegin(MAIN_DOCK_NAME, std::ptr::null_mut(), 0);

            let mut avail = IMVEC2_ZERO;
            igGetContentRegionAvail(&mut avail);

            igEnd();
            */

            igPopStyleVar(5);
        }
    }

    /// Add an optional status bar which appears at the bottom of the main window at fixed size and position
    /// you push items into the status bar by using `imgui.begin("status_bar")` 
    /// when passing `height` the true size may actually differ, the actual size is returned from this function
    pub fn add_status_bar(&mut self, height: f32) -> f32 {
        unsafe {
            let status_bar_flags = ImGuiWindowFlags_NoDocking | 
                ImGuiWindowFlags_NoResize | 
                ImGuiWindowFlags_NoTitleBar |
                ImGuiWindowFlags_NoMove | 
                ImGuiWindowFlags_NoScrollbar | 
                ImGuiWindowFlags_NoSavedSettings;

            let vp = &*igGetMainViewport();
            let style = &*igGetStyle();

            igSetNextWindowPos(ImVec2 {x: vp.Pos.x, y: vp.Pos.y + vp.Size.y - height}, 0, IMVEC2_ZERO);
            igSetNextWindowSize(ImVec2 {x: vp.Size.x, y: height}, 0);
            igPushStyleVarFloat(ImGuiStyleVar_WindowRounding as i32, 0.0);
            igPushStyleVarFloat(ImGuiStyleVar_WindowBorderSize as i32, 0.0);
            igPushStyleColorVec4(ImGuiCol_WindowBg as i32, style.Colors[ImGuiCol_MenuBarBg as usize]);

            igBegin(STATUS_BAR_NAME, std::ptr::null_mut(), status_bar_flags as i32);
            igPopStyleColor(1);
            igPopStyleVar(2);

            let mut actual_size = IMVEC2_ZERO;
            igGetWindowSize(&mut actual_size);
            let actual_height = actual_size.y;

            igEnd();

            actual_height
        }
    }

    /// Return the size of the main dockspace viewport which is the background of the main window
    pub fn get_main_dock_size(&self) -> (f32, f32) {
        unsafe {
            igBegin(MAIN_DOCK_NAME, std::ptr::null_mut(), 0);
            let mut avail = IMVEC2_ZERO;
            igGetContentRegionAvail(&mut avail);
            igEnd();
            (avail.x, avail.y)
        }
    }

    /// Call this each frame to render the `ImGui` data, `image_heaps` can be empty if you are only
    /// using the default device heap via `device.create_texture`
    pub fn render(
        &mut self,
        app: &mut A,
        main_window: &mut A::Window,
        device: &mut D,
        cmd: &mut D::CmdBuf,
        image_heaps: &Vec<&D::Heap>,
    ) {
        unsafe {
            let io = &mut *igGetIO();
            igRender();

            render_draw_data::<D>(
                &*igGetDrawData(),
                device,
                cmd,
                image_heaps,
                &mut self.buffers,
                &self.pipeline,
            )
            .unwrap();

            if (io.ConfigFlags & ImGuiConfigFlags_ViewportsEnable as i32) != 0 {
                // gotta pack the refs into a pointer and into io.UserData
                let mut ud = UserData {
                    device,
                    app,
                    main_window,
                    pipeline: &self.pipeline,
                    image_heaps
                };
                io.UserData = (&mut ud as *mut UserData<D, A>) as _;

                igUpdatePlatformWindows();
                igRenderPlatformWindowsDefault(std::ptr::null_mut(), std::ptr::null_mut());

                io.UserData = std::ptr::null_mut();
            }
        }
    }

    /// This is the native `ImGui` demo window which showcases all features present in `ImGui`
    pub fn demo(&self) {
        unsafe {
            static mut SHOW_DEMO_WINDOW: bool = true;
            static mut SHOW_ANOTHER_WINDOW: bool = true;
            static mut CLEAR_COLOUR: [f32; 3] = [0.0, 0.0, 0.0];

            let io = &mut *igGetIO();

            if SHOW_DEMO_WINDOW {
                igShowDemoWindow(static_ref_mut!(SHOW_DEMO_WINDOW));
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

                igCheckbox("Demo Window\0".as_ptr() as *const i8, static_ref_mut!(SHOW_DEMO_WINDOW));
                igCheckbox("Another Window\0".as_ptr() as *const i8, static_ref_mut!(SHOW_ANOTHER_WINDOW));

                igText(
                    "%f, %f : %f %f\0".as_ptr() as *const i8,
                    io.MousePos.x as f64,
                    io.MousePos.y as f64,
                    io.DisplaySize.x as f64,
                    io.DisplaySize.y as f64,
                );

                igSliderFloat(
                    "float\0".as_ptr() as _,
                    static_ref_mut!(SLIDER_FLOAT),
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

                igColorEdit3("color\0".as_ptr() as _, static_ref_array_mut!(CLEAR_COLOUR), 0); // Edit 3 floats representing a color

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
                    static_ref_mut!(SHOW_ANOTHER_WINDOW),
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

    /// Get the current imgui context so it can be passed to other plugins and libs, to be used woth `set_current_context`
    pub fn get_current_context(&self) ->*mut core::ffi::c_void {
        unsafe {
            igGetCurrentContext() as *mut core::ffi::c_void
        }
    }

    /// This function will make the `ImGuiContext` current, required when calling imgui from inside in a plugin dll or lib
    pub fn set_current_context(&mut self, context: *mut core::ffi::c_void) {
        unsafe {
            igSetCurrentContext(context as *mut ImGuiContext);
        }
    }

    /// Begin a new imgui window
    pub fn begin(&mut self, title: &str, open: &mut bool, flags: WindowFlags) -> bool {
        unsafe {
            let null_title = CString::new(title).unwrap();
            igBegin(
                null_title.as_ptr() as *const i8,
                open as *mut bool,
                i32::from(flags),
            )
        }
    }

    /// Begin a window with no settings, you can use this to re-use existing and setup windows
    pub fn begin_window(&mut self, name: &str) -> bool {
        unsafe {
            let null_name = CString::new(name).unwrap();
            igBegin(
                null_name.as_ptr() as *const i8,
                std::ptr::null_mut(),
                0
            )
        }
    }

    /// End imgui window must be called after a call to `begin` regardless of if `begin` returns true or false
    pub fn end(&mut self) {
        unsafe { 
            igEnd();
        };
    }

    /// Add imgui text widget, you can format text to pass in: `imgui.text(&format!("{}", values));`
    pub fn text(&mut self, text: &str) {
        let null_term_text = CString::new(text).unwrap();
        unsafe {
            igText(null_term_text.as_ptr() as *const i8);
        }
    }

    /// Add coloured text specified with rgba 0-1 range float.
    pub fn colour_text(&mut self, text: &str, col: Vec4f) {
        unsafe {
            igPushStyleColorVec4(ImGuiCol_Text as i32, to_im_vec4(col));
            self.text(text);
            igPopStyleColor(1);
        }
    }

    /// Push a style colour using ImGuiCol_ flags
    pub fn push_style_colour(&mut self, flags: ImGuiStyleVar, col: Vec4f) {
        unsafe {
            igPushStyleColorVec4(flags, to_im_vec4(col))
        }
    }

    /// Pop a single style colour ImGuiStyleVar from the stack
    pub fn pop_style_colour(&mut self) {
        unsafe {
            igPopStyleColor(1);
        }
    }
    
    /// Pop a style colour using ImGuiCol_ flags
    pub fn pop_style_colour_count(&mut self, count: i32) {
        unsafe {
            igPopStyleColor(count);
        }
    }

    /// Begin imgui main menu bar which appears at the top of the main window
    pub fn begin_main_menu_bar(&mut self) -> bool {
        unsafe {
            igBeginMainMenuBar()
        }
    }

    /// Ends main menu bar calls, this must be called after a call to `begin_main_menu_bar` returns true
    pub fn end_main_menu_bar(&mut self) {
        unsafe {
            igEndMainMenuBar()
        }
    }

    /// Begin new menu, populate it with `menu_item`
    pub fn begin_menu(&mut self, label: &str) -> bool {
        let null_term_label = CString::new(label).unwrap();
        unsafe {
            igBeginMenu(null_term_label.as_ptr() as *const i8, true)
        }
    }

    /// End menu on the top of the stack, pairs with `begin_menu`
    pub fn end_menu(&mut self) {
        unsafe {
            igEndMenu()
        }
    }

    /// Begin a combo box, populate it with `selectable`'s
    pub fn begin_combo(&mut self, label: &str, preview_item: &str, flags: ImGuiComboFlags) -> bool {
        unsafe {
            let null_term_label = CString::new(label).unwrap();
            let null_term_preview_item = CString::new(preview_item).unwrap();
            igBeginCombo(
                null_term_label.as_ptr() as *const i8, 
                null_term_preview_item.as_ptr() as *const i8,
                flags
            )
        }
    }

    /// End combo box on the top of the stack, pairs with `begin_combo`
    pub fn end_combo(&mut self) {
        unsafe {
            igEndCombo()
        }
    }

    /// A selectable item, can be used in menus, combo boxes, list etc. it highlights on hover
    pub fn selectable(&mut self, label: &str, selected: bool, flags: ImGuiSelectableFlags) -> bool {
        unsafe {
            let null_term_label = CString::new(label).unwrap();
            igSelectableBool(null_term_label.as_ptr() as *const i8, selected, flags, ImVec2 { x: 0.0, y: 0.0 })
        }
    }

    /// Simple combo box generated from a `Vec<String>`
    pub fn combo_list(&mut self, label: &str, items: &Vec<String>, selected: &str) -> (bool, String) {
        let mut result = selected.to_string();
        if self.begin_combo(label, selected, ImGuiComboFlags_None as i32) {
            for item in items {
                if self.selectable(item, item == selected, ImGuiSelectableFlags_None as i32) {
                    result = item.to_string(); 
                }
            }
            self.end_combo();
            (true, result)
        }
        else {
            (false, result)
        }
    }

    /// Menu items to populate menus between `begin_menu` and `end_menu`
    pub fn menu_item(&mut self, label: &str) -> bool {
        let null_term_label = CString::new(label).unwrap();
        unsafe {
            igMenuItemBool(
                null_term_label.as_ptr() as *const i8, 
                std::ptr::null(), 
                false, 
                true)
        }
    }

    /// Adds a horizontal line separating items
    pub fn separator(&mut self) {
        unsafe {
            igSeparator()
        }
    }

    /// Adds horizontal spacing
    pub fn spacing(&mut self) {
        unsafe {
            igSpacing()
        }
    }

    /// The next width will appear on the same line (horizontally) as the previous
    pub fn same_line(&mut self) {
        unsafe { 
            igSameLine(0.0, -1.0);
        };
    }

    /// Button with label, auto size
    pub fn button(&mut self, label: &str) -> bool {
        unsafe {
            let null_label = CString::new(label).unwrap();
            igButton(null_label.as_ptr() as *const i8, ImVec2{x: 0.0, y: 0.0})
        }
    }

    // Button with lable at specified size, supply 0.0 to `w` or `h` to auto size the dimension
    pub fn button_size(&mut self, label: &str, w: f32, h: f32) -> bool {
        unsafe {
            let null_label = CString::new(label).unwrap();
            igButton(null_label.as_ptr() as *const i8, ImVec2{x: w, y: h})
        }
    }

    /// Checkbox can be used on bools yes/no will display a tick or check when v is true and appear
    /// empty otherwise
    pub fn checkbox(&mut self, label: &str, v: &mut bool) -> bool {
        unsafe {    
            let null_label = CString::new(label).unwrap();
            igCheckbox(null_label.as_ptr() as *const i8, v)
        }
    }

    /// Modify float via keyboard input
    pub fn input_float(&mut self, label: &str, v: &mut f32) -> bool {
        unsafe {
            let null_label = CString::new(label).unwrap();
            let fmt = CString::new("%.3f").unwrap();
            igInputFloat(
                null_label.as_ptr() as *const i8, v, 0.0, 0.0, fmt.as_ptr() as *const i8, 0)
        }
    }

    /// Modify int via keyboard input
    pub fn input_int(&mut self, label: &str, v: &mut i32) -> bool {
        unsafe {
            let null_label = CString::new(label).unwrap();
            igInputInt(
                null_label.as_ptr() as *const i8, v, 0, 0, 0)
        }
    }

    /// Modify float via slider input
    pub fn slider_float(&mut self, label: &str, v: &mut f32, min: f32, max: f32) -> bool {
        unsafe {
            let null_label = CString::new(label).unwrap();
            let fmt = CString::new("%.3f").unwrap();
            igSliderFloat(
                null_label.as_ptr() as *const i8, v, min, max, fmt.as_ptr() as *const i8, 0)
        }
    }

    /// Adds a dummy widget for custom padding
    pub fn dummy(&mut self, w: f32, h: f32) {
        unsafe {
            igDummy(ImVec2 {x: w, y: h})
        }
    }

    /// Adds a dummy widget for custom padding horizontally
    pub fn hdummy(&mut self, size: f32) {
        unsafe {
            igDummy(ImVec2 {x: size, y: 0.0})
        }
    }

    /// Adds a dummy widget for custom padding vertically
    pub fn vdummy(&mut self, size: f32) {
        unsafe {
            igDummy(ImVec2 {x: 0.0, y: size})
        }
    }

    /// Dsiaply an image at the supplied width `w` and height `h` in pixels
    pub fn image(&mut self, tex: &D::Texture, w: f32, h: f32) {
        unsafe {
            let id = to_imgui_texture_id::<D>(tex);
            igImage(
                id, 
                ImVec2 {x: w, y: h},
                ImVec2 {x: 0.0, y: 0.0},
                ImVec2 {x: 1.0, y: 1.0},
                ImVec4 {x: 1.0, y: 1.0, z: 1.0, w: 1.0},
                ImVec4 {x: 0.0, y: 0.0, z: 0.0, w: 0.0},
            );
        }
    }

    /// Calculates the `x`, `y` size of the given text string, it can be used to aid layouts.
    pub fn calc_text_size(&self, text: &str) -> (f32, f32) {
        let null_text = CString::new(text).unwrap();
        let mut size = IMVEC2_ZERO;
        unsafe {
            igCalcTextSize(&mut size, null_text.as_ptr() as *const i8, std::ptr::null(), false, -1.0);
        }
        (size.x, size.y)
    }

    /// Align the next item to right of the context region subtarcted `offset` toward the left
    pub fn right_align(&mut self, offset: f32) {
        unsafe {
            let mut size = IMVEC2_ZERO;
            igGetWindowSize(&mut size);
            igSetCursorPosX(size.x - offset);
        }
    }

    /// Fills an entire content region identified by `label` with an image. Displ;aying no boarders or padding
    pub fn image_window(&mut self, label: &str, tex: &D::Texture) {
        unsafe {
            let null_label = CString::new(label).unwrap();

            let (w, h) = self.get_main_dock_size();

            let id = to_imgui_texture_id::<D>(tex);

            igBegin(null_label.as_ptr() as *const i8, std::ptr::null_mut(), 0);
            
            igImage(
                id, 
                ImVec2 {x: w, y: h},
                ImVec2 {x: 0.0, y: 0.0},
                ImVec2 {x: 1.0, y: 1.0},
                ImVec4 {x: 1.0, y: 1.0, z: 1.0, w: 1.0},
                ImVec4 {x: 0.0, y: 0.0, z: 0.0, w: 0.0},
            );

            igEnd();
        }
    }

    /// Sets the item of the next widget > 0.0 = width on pixels. < 0.0 = align to the right of window
    pub fn set_next_item_width(&mut self, item_width: f32) {
        unsafe {
            igSetNextItemWidth(item_width);
        }
    }

    /// Returns true if the main dock is hovered fals otherwise
    pub fn main_dock_hovered(&self) -> bool {
        unsafe {
            igBegin(MAIN_DOCK_NAME, std::ptr::null_mut(), 0);
            let result = igIsWindowHovered(0);
            igEnd();
            result
        }
    }

    /// Returns true if `ImGui` wants to intercept keyboard events, so they can be ignored elsewhere
    pub fn want_capture_keyboard(&self) -> bool {
        unsafe {
            let io = &mut *igGetIO();
            io.WantCaptureKeyboard
        }
    }

    /// Returns true if `ImGui` wants to intercept mouse events, so they can be ignored elsewhere
    pub fn want_capture_mouse(&self) -> bool {
        unsafe {
            let io = &mut *igGetIO();
            io.WantCaptureMouse
        }
    }

    /// Saves `imgui.ini` to the executable directory
    pub fn save_ini_settings(&self) {
        unsafe {
            let io = &mut *igGetIO();
            igSaveIniSettingsToDisk(io.IniFilename);
        }
    }

    /// Saves `imgui.ini` to the specified directory `path`
    pub fn save_ini_settings_to_location(&self, path: &str) {
        unsafe {
            let null_term_filename = CString::new(format!("{}/imgui.ini", path)).unwrap();
            igSaveIniSettingsToDisk(null_term_filename.as_ptr() as *const i8);
        }
    }

    /// Return the horizontal scroll position of the current window
    pub fn get_scroll_x(&self) -> f32 {
        unsafe {
            igGetScrollX()
        }
    }

    /// Set the horizontal scroll position of the current window
    pub fn set_scroll_x(&self, pos: f32) {
        unsafe {
            igSetScrollXFloat(pos)
        }
    }

    /// Return the vertical scroll position of the current window
    pub fn get_scroll_y(&self) -> f32 {
        unsafe {
            igGetScrollY()
        }
    }

    /// Set the vertical scroll position of the current window
    pub fn set_scroll_y(&self, pos: f32) {
        unsafe {
            igSetScrollYFloat(pos)
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
        &mut vd.window[0]
    }
}

/// get a hotline_rs::imgui::ViewportData mutable reference from an ImGuiViewport
fn get_viewport_data<'a, D: Device, A: App>(vp: *mut ImGuiViewport) -> &'a mut ViewportData<D, A> {
    unsafe {
        let vp_ref = &mut *vp;
        &mut *(vp_ref.PlatformUserData as *mut ViewportData<D, A>)
    }
}

/// Get the UserData packed inside ImGui.io to access Device, App and main Window
fn get_user_data<'a, D: Device, A: App>() -> &'a mut UserData<'a, D, A> {
    unsafe {
        let io = &mut *igGetIO();
        &mut *(io.UserData as *mut UserData<D, A>)
    }
}

unsafe extern "C" fn platform_create_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let io = &mut *igGetIO();
    let ud = &mut *(io.UserData as *mut UserData<D, A>);
    let device = &mut ud.device;
    let vp_ref = &mut *vp;

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
        parent_handle,
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
    vd.swap_chain = vec![device.create_swap_chain::<A>(&swap_chain_info, &vd.window[0]).unwrap()];

    // create render buffers
    let mut buffers: Vec<RenderBuffers<D>> = Vec::new();
    let num_buffers = vd.swap_chain[0].get_num_buffers();
    for _i in 0..num_buffers {
        buffers.push(RenderBuffers {
            vb: create_vertex_buffer::<D>(device, DEFAULT_VB_SIZE).unwrap(),
            vb_size: DEFAULT_VB_SIZE,
            ib: create_index_buffer::<D>(device, DEFAULT_IB_SIZE).unwrap(),
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
    let vp_ref = &mut *vp;

    if !vd.swap_chain.is_empty() {
        vd.swap_chain[0].wait_for_last_frame();
        vd.cmd[0].reset(&vd.swap_chain[0]);
    }

    // unregister window tracking... if UserData is null we are shutting down
    let io = &mut *igGetIO();
    if !io.UserData.is_null() {
        get_user_data::<D, A>().app.destroy_window(&vd.window[0]);
    }
    
    if !vd.swap_chain.is_empty() {
        vd.swap_chain.clear();
    }

    if !vd.cmd.is_empty() {
        vd.cmd.clear();
    }

    if !vd.buffers.is_empty() {
        vd.buffers.clear();
    }
    
    if !vd.window.is_empty() {
        vd.window.clear();
    }

    // drop and null allocated data
    std::ptr::drop_in_place(vp_ref.PlatformUserData as *mut ViewportData<D, A>);
    vp_ref.PlatformUserData = std::ptr::null_mut();
    std::ptr::drop_in_place(vp_ref.PlatformHandle as *mut A::NativeHandle);
    vp_ref.PlatformHandle = std::ptr::null_mut();
}

unsafe extern "C" fn platform_update_window<D: Device, A: App>(vp: *mut ImGuiViewport) {
    let window = get_viewport_window::<D, A>(vp);
    let vp_ref = &mut *vp;
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
    let activate = (*vp).Flags & ImGuiViewportFlags_NoFocusOnAppearing as i32 == 0;
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

unsafe extern "C" fn renderer_render_window<D: Device, A: App>(vp: *mut ImGuiViewport, _render_arg: *mut cty::c_void) where D::RenderPipeline: gfx::Pipeline {
    let ud = get_user_data::<D, A>();
    let vd = get_viewport_data::<D, A>(vp);
    let vp_ref = &*vp;

    // must be an imgui created window
    assert_ne!(vd.window.len(), 0);
    assert_ne!(vd.cmd.len(), 0);
    assert_ne!(vd.swap_chain.len(), 0);

    // unpack from vec
    let window = &mut vd.window[0];
    let cmd = &mut vd.cmd[0];
    let swap = &mut vd.swap_chain[0];
    let vp_rect = window.get_viewport_rect();

    // update
    window.update(ud.app);
    swap.update::<A>(ud.device, window, cmd);
    cmd.reset(swap);

    // render
    let viewport = gfx::Viewport::from(vp_rect);
    let scissor = gfx::ScissorRect::from(vp_rect);

    // TODO:
    cmd.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(swap.get_backbuffer_texture()),
        buffer: None,
        state_before: gfx::ResourceState::Present,
        state_after: gfx::ResourceState::RenderTarget,
    });

    let pass = swap.get_backbuffer_pass_mut();
    cmd.begin_render_pass(pass);

    cmd.set_viewport(&viewport);
    cmd.set_scissor_rect(&scissor);

    render_draw_data::<D>(
        &*vp_ref.DrawData,
        ud.device,
        cmd,
        ud.image_heaps,
        &mut vd.buffers,
        ud.pipeline,
    )
    .unwrap();

    cmd.end_render_pass();

    // TODO:
    cmd.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(swap.get_backbuffer_texture()),
        buffer: None,
        state_before: gfx::ResourceState::RenderTarget,
        state_after: gfx::ResourceState::Present,
    });

    cmd.close().unwrap();

    ud.device.execute(cmd);
}

unsafe extern "C" fn renderer_swap_buffers<D: Device, A: App>(vp: *mut ImGuiViewport, _render_arg: *mut cty::c_void) {
    let ud = get_user_data::<D, A>();
    let vd = get_viewport_data::<D, A>(vp);
    assert_ne!(vd.swap_chain.len(), 0);
    vd.swap_chain[0].swap(ud.device);
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
const fn to_os_cursor(cursor: ImGuiMouseCursor) -> os::Cursor {
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