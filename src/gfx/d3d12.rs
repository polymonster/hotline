use crate::os::Window;

#[cfg(target_os = "windows")]
use crate::os::win32 as platform;

use super::*;
use super::Device as SuperDevice;

use std::ffi::CStr;
use std::ffi::CString;
use std::result;
use std::str;
use std::collections::HashMap;
use std::char::{decode_utf16, REPLACEMENT_CHARACTER};

use windows::{
    core::*, 
    Win32::Foundation::*, 
    Win32::Graphics::Direct3D::Fxc::*, 
    Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, 
    Win32::Graphics::Dxgi::Common::*, 
    Win32::Graphics::Dxgi::*,
    Win32::System::Threading::*, 
    Win32::System::WindowsProgramming::*,
    Win32::System::LibraryLoader::*
};

type BeginEventOnCommandList = extern "stdcall" fn(*const core::ffi::c_void, u64, PSTR) -> i32;
type EndEventOnCommandList = extern "stdcall" fn(*const core::ffi::c_void) -> i32;
type SetMarkerOnCommandList = extern "stdcall" fn(*const core::ffi::c_void, u64, PSTR) -> i32;

#[derive(Copy, Clone)]
struct WinPixEventRuntime {
    begin_event: BeginEventOnCommandList,
    end_event: EndEventOnCommandList,
    set_marker: SetMarkerOnCommandList,
}

impl WinPixEventRuntime {
    pub fn create() -> Option<WinPixEventRuntime> {
        unsafe {
            let module = LoadLibraryA("WinPixEventRuntime.dll\0");
            
            let p_begin_event = GetProcAddress(module, "PIXBeginEventOnCommandList\0");
            let p_end_event = GetProcAddress(module, "PIXEndEventOnCommandList\0");
            let p_set_marker = GetProcAddress(module, "PIXSetMarkerOnCommandList\0");

            if p_begin_event.is_some() && p_end_event.is_some() && p_set_marker.is_some() {
                return Some(WinPixEventRuntime {
                    begin_event: std::mem::transmute::<*const usize, BeginEventOnCommandList>(
                        p_begin_event.unwrap() as *const usize),
                    end_event: std::mem::transmute::<*const usize, EndEventOnCommandList>(
                        p_end_event.unwrap() as *const usize),
                    set_marker: std::mem::transmute::<*const usize, SetMarkerOnCommandList>(
                        p_set_marker.unwrap() as *const usize),
                });
            }
            None
        }
    }

    pub fn begin_event_on_command_list(&self, command_list: ID3D12GraphicsCommandList, color: u64, name: &str) {
        unsafe {
            let null_name = CString::new(name).unwrap();
            let fn_begin_event : BeginEventOnCommandList = self.begin_event;
            let p_cmd_list = std::mem::transmute::<IUnknown, *const core::ffi::c_void>(command_list.0.clone());
            fn_begin_event(p_cmd_list, color, PSTR(null_name.as_ptr() as _));
        }
    }

    pub fn end_event_on_command_list(&self, command_list: ID3D12GraphicsCommandList) {
        unsafe {
            let fn_end_event : EndEventOnCommandList = self.end_event;
            let p_cmd_list = std::mem::transmute::<IUnknown, *const core::ffi::c_void>(command_list.0.clone());
            fn_end_event(p_cmd_list);
        }
    }

    pub fn set_marker_on_command_list(&self, command_list: ID3D12GraphicsCommandList, color: u64, name: &str) {
        unsafe {
            let null_name = CString::new(name).unwrap();
            let fn_set_marker : SetMarkerOnCommandList = self.set_marker;
            let p_cmd_list = std::mem::transmute::<IUnknown, *const core::ffi::c_void>(command_list.0.clone());
            fn_set_marker(p_cmd_list, color, PSTR(null_name.as_ptr() as _));
        }
    }
}

pub struct Device {
    _adapter: IDXGIAdapter1,
    adapter_info: super::AdapterInfo,
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
    command_queue: ID3D12CommandQueue,
    pix: Option<WinPixEventRuntime>,
    shader_heap: Heap,
    rtv_heap: Heap,
    dsv_heap: Heap,
}

pub struct SwapChain {
    width: i32,
    height: i32,
    format: super::Format,
    num_bb: u32,
    flags: u32,
    frame_index: i32,
    bb_index: i32,
    swap_chain: IDXGISwapChain3,
    backbuffer_textures: Vec<Texture>,
    backbuffer_passes: Vec<RenderPass>,
    fence: ID3D12Fence,
    fence_last_signalled_value: u64,
    fence_event: HANDLE,
    frame_fence_value: Vec<u64>,
    readback_buffer: Option<ID3D12Resource>,
}

pub struct RenderPipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
    topology: D3D_PRIMITIVE_TOPOLOGY
}

pub struct CmdBuf {
    bb_index: usize,
    command_allocator: Vec<ID3D12CommandAllocator>,
    command_list: Vec<ID3D12GraphicsCommandList>,
    pix: Option<WinPixEventRuntime>,
    in_flight_barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>>,
}

pub struct Buffer {
    resource: ID3D12Resource,
    srv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    vbv: Option<D3D12_VERTEX_BUFFER_VIEW>,
    ibv: Option<D3D12_INDEX_BUFFER_VIEW>,
    srv_index: usize,
}

pub struct Shader {
    blob: ID3DBlob,
}

#[derive(Clone)]
pub struct Texture {
    resource: ID3D12Resource,
    rtv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    dsv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    srv: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    uav: Option<D3D12_CPU_DESCRIPTOR_HANDLE>,
    srv_index: usize
}

#[derive(Clone)]
pub struct ReadBackRequest {
    pub resource: Option<ID3D12Resource>,
    pub fence_value: u64,
    pub size: usize,
    pub row_pitch: usize,
    pub slice_pitch: usize,
}

pub struct RenderPass {
    rt: Vec<D3D12_RENDER_PASS_RENDER_TARGET_DESC>,
    rt_formats: Vec<DXGI_FORMAT>,
    ds: Option<D3D12_RENDER_PASS_DEPTH_STENCIL_DESC>,
    ds_format: DXGI_FORMAT,
    sample_count: u32
}

pub struct Heap {
    heap: ID3D12DescriptorHeap,
    base_address: usize,
    increment_size: usize,
    capacity: usize, // TODO: use and check when allocating
    offset: usize,
    free_list: Vec<usize>
}

pub struct ComputePipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
}

fn to_dxgi_format(format: super::Format) -> DXGI_FORMAT {
    match format {
        super::Format::Unknown => DXGI_FORMAT_UNKNOWN,
        super::Format::R16n => DXGI_FORMAT_R16_UNORM,
        super::Format::R16u => DXGI_FORMAT_R16_UINT,
        super::Format::R16i => DXGI_FORMAT_R16_SINT,
        super::Format::R16f => DXGI_FORMAT_R16_FLOAT,
        super::Format::R32u => DXGI_FORMAT_R32_UINT,
        super::Format::R32i => DXGI_FORMAT_R32_SINT,
        super::Format::R32f => DXGI_FORMAT_R32_FLOAT,
        super::Format::RG32u => DXGI_FORMAT_R32G32_UINT,
        super::Format::RG32i => DXGI_FORMAT_R32G32_SINT,
        super::Format::RG32f => DXGI_FORMAT_R32G32_FLOAT,
        super::Format::RGB32u => DXGI_FORMAT_R32G32B32_UINT,
        super::Format::RGB32i => DXGI_FORMAT_R32G32B32_SINT,
        super::Format::RGB32f => DXGI_FORMAT_R32G32B32_FLOAT,
        super::Format::RGBA8n => DXGI_FORMAT_R8G8B8A8_UNORM,
        super::Format::RGBA8u => DXGI_FORMAT_R8G8B8A8_UINT,
        super::Format::RGBA8i => DXGI_FORMAT_R8G8B8A8_SINT,
        super::Format::BGRA8n => DXGI_FORMAT_B8G8R8A8_UNORM,
        super::Format::RGBA16u => DXGI_FORMAT_R16G16B16A16_UINT,
        super::Format::RGBA16i => DXGI_FORMAT_R16G16B16A16_SINT,
        super::Format::RGBA16f => DXGI_FORMAT_R16G16B16A16_FLOAT,
        super::Format::RGBA32u => DXGI_FORMAT_R32G32B32A32_UINT,
        super::Format::RGBA32i => DXGI_FORMAT_R32G32B32A32_SINT,
        super::Format::RGBA32f => DXGI_FORMAT_R32G32B32A32_FLOAT,
        super::Format::D32fS8X24u => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        super::Format::D32f => DXGI_FORMAT_D32_FLOAT,
        super::Format::D24nS8u => DXGI_FORMAT_D24_UNORM_S8_UINT,
        super::Format::D16n => DXGI_FORMAT_D16_UNORM,
    }
}

fn to_d3d12_compile_flags(flags: &super::ShaderCompileFlags) -> u32 {
    let mut d3d12_flags = 0;
    if flags.contains(super::ShaderCompileFlags::SKIP_OPTIMIZATION) {
        d3d12_flags |= D3DCOMPILE_SKIP_OPTIMIZATION;
    }
    if flags.contains(super::ShaderCompileFlags::DEBUG) {
        d3d12_flags |= D3DCOMPILE_DEBUG;
    }
    d3d12_flags
}

fn to_d3d12_shader_visibility(visibility: &super::ShaderVisibility) -> D3D12_SHADER_VISIBILITY {
    match visibility {
        super::ShaderVisibility::All => D3D12_SHADER_VISIBILITY_ALL,
        super::ShaderVisibility::Vertex => D3D12_SHADER_VISIBILITY_VERTEX,
        super::ShaderVisibility::Fragment => D3D12_SHADER_VISIBILITY_PIXEL,
        super::ShaderVisibility::Compute => D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn to_d3d12_sampler_boarder_colour(col: Option<u32>) -> D3D12_STATIC_BORDER_COLOR {
    if col.is_some() {
        return D3D12_STATIC_BORDER_COLOR::from(col.unwrap() as i32);
    }
    D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK
}

fn to_d3d12_filter(filter: super::SamplerFilter) -> D3D12_FILTER {
    match filter {
        super::SamplerFilter::Point => D3D12_FILTER_MIN_MAG_MIP_POINT,
        super::SamplerFilter::Linear => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        super::SamplerFilter::Anisotropic => D3D12_FILTER_ANISOTROPIC,
    }
}

fn to_d3d12_address_mode(mode: super::SamplerAddressMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    match mode {
        super::SamplerAddressMode::Wrap => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        super::SamplerAddressMode::Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        super::SamplerAddressMode::Clamp => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        super::SamplerAddressMode::Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
        super::SamplerAddressMode::MirrorOnce => D3D12_TEXTURE_ADDRESS_MODE_MIRROR_ONCE,
    }
}

fn to_d3d12_comparison_func(func: super::ComparisonFunc) -> D3D12_COMPARISON_FUNC {
    match func {
        super::ComparisonFunc::Never => D3D12_COMPARISON_FUNC_NEVER,
        super::ComparisonFunc::Less => D3D12_COMPARISON_FUNC_LESS,
        super::ComparisonFunc::Equal => D3D12_COMPARISON_FUNC_EQUAL,
        super::ComparisonFunc::LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
        super::ComparisonFunc::Greater => D3D12_COMPARISON_FUNC_GREATER,
        super::ComparisonFunc::NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
        super::ComparisonFunc::GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
        super::ComparisonFunc::Always => D3D12_COMPARISON_FUNC_ALWAYS,
    }
}

fn to_d3d12_address_comparison_func(func: Option<super::ComparisonFunc>) -> D3D12_COMPARISON_FUNC {
    if func.is_some() {
        return to_d3d12_comparison_func(func.unwrap())
    }
    D3D12_COMPARISON_FUNC_ALWAYS
}

fn to_d3d12_resource_state(state: super::ResourceState) -> D3D12_RESOURCE_STATES {
    match state {
        super::ResourceState::RenderTarget => D3D12_RESOURCE_STATE_RENDER_TARGET,
        super::ResourceState::Present => D3D12_RESOURCE_STATE_PRESENT,
        super::ResourceState::UnorderedAccess => D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        super::ResourceState::ShaderResource => D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
        super::ResourceState::VertexConstantBuffer => D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER,
        super::ResourceState::IndexBuffer => D3D12_RESOURCE_STATE_INDEX_BUFFER,
        super::ResourceState::DepthStencil => D3D12_RESOURCE_STATE_DEPTH_WRITE,
        super::ResourceState::DepthStencilReadOnly => D3D12_RESOURCE_STATE_DEPTH_READ,
    }
}

fn to_d3d12_descriptor_heap_type(heap_type: super::HeapType) -> D3D12_DESCRIPTOR_HEAP_TYPE {
    match heap_type {
        super::HeapType::Shader => D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        super::HeapType::RenderTarget => D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
        super::HeapType::DepthStencil => D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
        super::HeapType::Sampler => D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER
    }
}

fn to_d3d12_descriptor_heap_flags(heap_type: super::HeapType) -> D3D12_DESCRIPTOR_HEAP_FLAGS {
    match heap_type {
        super::HeapType::Shader => D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        super::HeapType::RenderTarget => D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        super::HeapType::DepthStencil => D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        super::HeapType::Sampler => D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
    }
}

fn to_d3d12_texture_usage_flags(usage: super::TextureUsage) -> D3D12_RESOURCE_FLAGS {
    let mut flags = D3D12_RESOURCE_FLAG_NONE;
    if usage.contains(super::TextureUsage::RENDER_TARGET) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if usage.contains(super::TextureUsage::DEPTH_STENCIL) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
    }
    if usage.contains(super::TextureUsage::UNORDERED_ACCESS) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    flags
}

fn to_d3d12_primitive_topology(topology: super::Topology, patch_index: u32) -> D3D_PRIMITIVE_TOPOLOGY {
    match topology {
        super::Topology::Undefined => D3D_PRIMITIVE_TOPOLOGY_UNDEFINED,
        super::Topology::PointList => D3D_PRIMITIVE_TOPOLOGY_POINTLIST,
        super::Topology::LineList => D3D_PRIMITIVE_TOPOLOGY_LINELIST,
        super::Topology::LineStrip => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP,
        super::Topology::TriangleList => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
        super::Topology::TriangleStrip => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
        super::Topology::LineListAdj => D3D_PRIMITIVE_TOPOLOGY_LINELIST_ADJ,
        super::Topology::LineStripAdj => D3D_PRIMITIVE_TOPOLOGY_LINESTRIP_ADJ,
        super::Topology::TriangleListAdj => D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST_ADJ,
        super::Topology::TriangleStripAdj => D3D_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP_ADJ,
        super::Topology::PatchList => D3D_PRIMITIVE_TOPOLOGY(
            D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 as i32 + patch_index as i32
        )
    }
}

fn to_d3d12_primitive_topology_type(topology: super::Topology) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
    match topology {
        super::Topology::Undefined => D3D12_PRIMITIVE_TOPOLOGY_TYPE_UNDEFINED,
        super::Topology::PointList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_POINT,
        super::Topology::LineList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        super::Topology::LineStrip => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        super::Topology::TriangleList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        super::Topology::TriangleStrip => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        super::Topology::LineListAdj => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        super::Topology::LineStripAdj => D3D12_PRIMITIVE_TOPOLOGY_TYPE_LINE,
        super::Topology::TriangleListAdj => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        super::Topology::TriangleStripAdj => D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
        super::Topology::PatchList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH
    }
}

fn to_d3d12_fill_mode(fill_mode: &super::FillMode) -> D3D12_FILL_MODE {
    match fill_mode {
        super::FillMode::Wireframe => D3D12_FILL_MODE_WIREFRAME,
        super::FillMode::Solid => D3D12_FILL_MODE_SOLID,
    }
}

fn to_d3d12_cull_mode(cull_mode: &super::CullMode) -> D3D12_CULL_MODE {
    match cull_mode {
        super::CullMode::None => D3D12_CULL_MODE_NONE,
        super::CullMode::Front => D3D12_CULL_MODE_FRONT,
        super::CullMode::Back => D3D12_CULL_MODE_BACK,
    }
}

fn to_d3d12_write_mask(mask: &super::DepthWriteMask) -> D3D12_DEPTH_WRITE_MASK {
    match mask {
        super::DepthWriteMask::Zero => D3D12_DEPTH_WRITE_MASK_ZERO,
        super::DepthWriteMask::All => D3D12_DEPTH_WRITE_MASK_ALL
    }
}

fn to_d3d12_stencil_op(op: &super::StencilOp) -> D3D12_STENCIL_OP {
    match op {
        super::StencilOp::Keep => D3D12_STENCIL_OP_KEEP,
        super::StencilOp::Zero => D3D12_STENCIL_OP_ZERO,
        super::StencilOp::Replace => D3D12_STENCIL_OP_REPLACE,
        super::StencilOp::IncrSat => D3D12_STENCIL_OP_INCR_SAT,
        super::StencilOp::DecrSat => D3D12_STENCIL_OP_DECR_SAT,
        super::StencilOp::Invert => D3D12_STENCIL_OP_INVERT,
        super::StencilOp::Incr => D3D12_STENCIL_OP_INCR,
        super::StencilOp::Decr => D3D12_STENCIL_OP_DECR,
    }
}

fn to_d3d12_render_target_blend(blend_info: &Vec<super::RenderTargetBlendInfo>) -> [D3D12_RENDER_TARGET_BLEND_DESC; 8] {
    let mut rtb : [D3D12_RENDER_TARGET_BLEND_DESC; 8] = [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8];
    let mut i = 0;
    for b in blend_info {
        rtb[i] = D3D12_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL::from(b.blend_enabled),
            LogicOpEnable: BOOL::from(b.logic_op_enabled),
            SrcBlend: to_d3d12_blend_factor(&b.src_blend),
            DestBlend: to_d3d12_blend_factor(&b.dst_blend),
            BlendOp: to_d3d12_blend_op(&b.blend_op),
            SrcBlendAlpha: to_d3d12_blend_factor(&b.src_blend_alpha),
            DestBlendAlpha: to_d3d12_blend_factor(&b.dst_blend_alpha),
            BlendOpAlpha: to_d3d12_blend_op(&b.blend_op_alpha),
            LogicOp: to_d3d12_logic_op(&b.logic_op),
            RenderTargetWriteMask: u8::from(b.write_mask)
        };
        i += 1;
    }
    rtb
}

fn to_d3d12_blend_factor(factor: &super::BlendFactor) -> D3D12_BLEND {
    match factor {
        super::BlendFactor::Zero => D3D12_BLEND_ZERO,
        super::BlendFactor::One => D3D12_BLEND_ONE,
        super::BlendFactor::SrcColour => D3D12_BLEND_SRC_COLOR,
        super::BlendFactor::InvSrcColour => D3D12_BLEND_INV_SRC_COLOR,
        super::BlendFactor::SrcAlpha => D3D12_BLEND_SRC_ALPHA,
        super::BlendFactor::InvSrcAlpha => D3D12_BLEND_INV_SRC_ALPHA,
        super::BlendFactor::DstAlpha => D3D12_BLEND_DEST_ALPHA,
        super::BlendFactor::InvDstAlpha => D3D12_BLEND_INV_DEST_ALPHA,
        super::BlendFactor::DstColour => D3D12_BLEND_DEST_COLOR,
        super::BlendFactor::InvDstColour => D3D12_BLEND_INV_DEST_COLOR,
        super::BlendFactor::SrcAlphaSat => D3D12_BLEND_SRC_ALPHA_SAT,
        super::BlendFactor::BlendFactor => D3D12_BLEND_BLEND_FACTOR,
        super::BlendFactor::InvBlendFactor => D3D12_BLEND_INV_BLEND_FACTOR,
        super::BlendFactor::Src1Colour => D3D12_BLEND_SRC1_COLOR,
        super::BlendFactor::InvSrc1Colour => D3D12_BLEND_INV_SRC1_COLOR,
        super::BlendFactor::Src1Alpha => D3D12_BLEND_SRC1_ALPHA,
        super::BlendFactor::InvSrc1Alpha => D3D12_BLEND_INV_SRC1_ALPHA,
    }
}

fn to_d3d12_blend_op(op: &super::BlendOp) -> D3D12_BLEND_OP {
    match op {
        super::BlendOp::Add => D3D12_BLEND_OP_ADD,
        super::BlendOp::Subtract => D3D12_BLEND_OP_SUBTRACT,
        super::BlendOp::RevSubtract => D3D12_BLEND_OP_REV_SUBTRACT,
        super::BlendOp::Min => D3D12_BLEND_OP_MIN,
        super::BlendOp::Max => D3D12_BLEND_OP_MAX,
    }
}

fn to_d3d12_logic_op(op: &super::LogicOp) -> D3D12_LOGIC_OP {
    match op {
        super::LogicOp::Clear => D3D12_LOGIC_OP_CLEAR,
        super::LogicOp::Set => D3D12_LOGIC_OP_SET,
        super::LogicOp::Copy => D3D12_LOGIC_OP_COPY,
        super::LogicOp::CopyInverted => D3D12_LOGIC_OP_COPY_INVERTED,
        super::LogicOp::NoOp => D3D12_LOGIC_OP_NOOP,
        super::LogicOp::Invert => D3D12_LOGIC_OP_INVERT,
        super::LogicOp::And => D3D12_LOGIC_OP_AND,
        super::LogicOp::Nand => D3D12_LOGIC_OP_NAND, 
        super::LogicOp::Or => D3D12_LOGIC_OP_OR,
        super::LogicOp::Nor => D3D12_LOGIC_OP_NOR,
        super::LogicOp::Xor => D3D12_LOGIC_OP_XOR,
        super::LogicOp::Equiv => D3D12_LOGIC_OP_EQUIV,
        super::LogicOp::AndReverse => D3D12_LOGIC_OP_AND_REVERSE,
        super::LogicOp::AndInverted => D3D12_LOGIC_OP_AND_INVERTED,
        super::LogicOp::OrReverse => D3D12_LOGIC_OP_OR_REVERSE,
        super::LogicOp::OrInverted => D3D12_LOGIC_OP_OR_INVERTED,
    }
}

fn get_d3d12_error_blob_string(blob: &ID3DBlob) -> String {
    unsafe {
        String::from_raw_parts(
            blob.GetBufferPointer() as *mut _,
            blob.GetBufferSize(),
            blob.GetBufferSize(),
        )
    }
}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    let trans = std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: Some(resource.clone()),
        StateBefore: state_before,
        StateAfter: state_after,
        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
    });
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: trans },
    }
}

fn get_hardware_adapter(factory: &IDXGIFactory4, adapter_name: &Option<String>) -> Result<(IDXGIAdapter1, super::AdapterInfo)> {
    unsafe {
        let mut adapter_info = super::AdapterInfo {
            name: String::from(""),
            description:  String::from(""),
            dedicated_video_memory: 0,
            dedicated_system_memory: 0,
            shared_system_memory: 0,
            available: vec![]
        };

        // enumerate info
        let mut selected_index = -1;
        for i in 0.. {
            let adapter = factory.EnumAdapters1(i);
            if !adapter.is_ok() {
                break;
            }
            let desc = adapter.unwrap().GetDesc1()?;

            // decode utf-16 dfescription
            let decoded1 = decode_utf16(desc.Description)
                .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
                .collect::<String>();

            // trim utf-16 nul terminators
            let x: &[_] = &['\0', '\0'];
            let decoded = decoded1.trim_matches(x);
            adapter_info.available.push(decoded.to_string());

            if adapter_name.is_some() {
                let s = adapter_name.as_ref().unwrap().to_string();
                if s == decoded.to_string() {
                    selected_index = i as i32;
                }
            }
            else {
                // auto select first non software adapter
                if (DXGI_ADAPTER_FLAG::from(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE) == DXGI_ADAPTER_FLAG_NONE {
                    if selected_index == -1 {
                        selected_index = i as i32;
                    }
                }
            }
        }

        // default to adapter 0
        if selected_index == -1 {
            selected_index = 0;
        }
        
        let adapter = factory.EnumAdapters1(selected_index as u32)?;
        let desc = adapter.GetDesc1()?;

        if D3D12CreateDevice(
            &adapter,
            D3D_FEATURE_LEVEL_12_1,
            std::ptr::null_mut::<Option<ID3D12Device>>(),
        )
        .is_ok() {
            // fill adapter info out
            adapter_info.name = String::from("hotline::d3d12::Device");
            adapter_info.description = adapter_info.available[selected_index as usize].to_string();
            adapter_info.dedicated_video_memory = desc.DedicatedVideoMemory;
            adapter_info.dedicated_system_memory = desc.DedicatedSystemMemory;
            adapter_info.shared_system_memory = desc.SharedSystemMemory;
            return Ok((adapter, adapter_info));
        }
    }
    unreachable!()
}

fn create_read_back_buffer(device: &Device, size: u64) -> Option<ID3D12Resource> {
    let mut readback_buffer: Option<ID3D12Resource> = None;
    unsafe {
        // readback buffer
        device
            .device
            .CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_READBACK,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: size,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Format: DXGI_FORMAT_UNKNOWN,
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COPY_DEST,
                std::ptr::null(),
                &mut readback_buffer,
            ).expect("hotline::gfx::d3d12: failed to create readback buffer");
    }
    readback_buffer
}

fn create_heap(device: &ID3D12Device, info: &HeapInfo) -> Heap {
    unsafe {
        let d3d12_type = to_d3d12_descriptor_heap_type(info.heap_type);
        let heap : ID3D12DescriptorHeap = device
        .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            Type: d3d12_type,
            NumDescriptors: std::cmp::max(info.num_descriptors, 1) as u32,
            Flags: to_d3d12_descriptor_heap_flags(info.heap_type),
            ..Default::default()
        }).expect("hotline::gfx::d3d12: failed to create heap");
        let base_address = heap.GetCPUDescriptorHandleForHeapStart().ptr;
        Heap {
            heap: heap,
            base_address: base_address as usize,
            increment_size: device.GetDescriptorHandleIncrementSize(d3d12_type) as usize,
            capacity: info.num_descriptors,
            offset: 0,
            free_list: Vec::new()
        }
    }
}

fn create_swap_chain_rtv(
    swap_chain: &IDXGISwapChain3,
    device: &mut Device,
    num_bb: u32,
) -> Vec<Texture> {
    unsafe {
        // render targets for the swap chain
        let mut textures: Vec<Texture> = Vec::new();
        for i in 0..num_bb {
            let render_target: ID3D12Resource = swap_chain.GetBuffer(i).unwrap();
            let h = device.rtv_heap.allocate();
            device.device.CreateRenderTargetView(&render_target, std::ptr::null_mut(), &h);
            textures.push(Texture {
                resource: render_target.clone(),
                srv: None,
                srv_index: usize::MAX,
                rtv: Some(h),
                uav: None,
                dsv: None
            });
        }
        textures
    }
}

fn null_terminate_semantics(layout: &super::InputLayout) -> Vec<CString> {
    let mut c_strs: Vec<CString> = Vec::new();
    for elem in layout {
        c_strs.push(CString::new(elem.semantic.clone()).unwrap());
    }
    c_strs
}

/// validates the length of data is consistent with a known size_bytes of a buffer or texture
fn validate_data_size<T: Sized>(size_bytes: usize, data: Option<&[T]>) -> result::Result<(), super::Error> {
    if data.is_some() {
        let data = data.unwrap();
        let data_size_bytes = data.len() * std::mem::size_of::<T>();
        if data_size_bytes != size_bytes {
            return Err(super::Error {
                error_type: super::ErrorType::DataSize,
                msg: String::from(format!(
                    "data size: ({}) bytes does not match expected size: ({}) bytes",
                    data_size_bytes, size_bytes
                )),
            });
        }
    }
    Ok(())
}

impl super::Shader<Device> for Shader {}
impl super::RenderPipeline<Device> for RenderPipeline {}
impl super::RenderPass<Device> for RenderPass {}
impl super::Heap<Device> for Heap {}

impl From<windows::core::Error> for super::Error {
    fn from(err: windows::core::Error) -> super::Error {
        super::Error {
            error_type: ErrorType::Direct3D12,
            msg: err.message().to_string_lossy(),
        }
    }
}

impl Heap {
    fn allocate(&mut self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        unsafe {
            let mut ptr = 0;
            if self.free_list.is_empty() {
                // allocates a new handle
                ptr = self.heap.GetCPUDescriptorHandleForHeapStart().ptr + self.offset;
                self.offset += self.increment_size;
            }
            else {
                // pulls new handle from the free list
                ptr = self.free_list.pop().unwrap();
            }
            D3D12_CPU_DESCRIPTOR_HANDLE { 
                ptr: ptr 
            }
        }
    }

    fn get_handle_index(&self, handle: &D3D12_CPU_DESCRIPTOR_HANDLE) -> usize {
        let ptr = handle.ptr - self.base_address;
        ptr / self.increment_size
    }

    fn deallocate(&mut self, handle: &D3D12_CPU_DESCRIPTOR_HANDLE) {
        self.free_list.push(handle.ptr);
    }
}

impl Device {
    fn create_d3d12_input_element_desc(
        layout: &super::InputLayout,
        null_terminated_semantics: &Vec<CString>,
    ) -> Vec<D3D12_INPUT_ELEMENT_DESC> {
        let mut d3d12_elems: Vec<D3D12_INPUT_ELEMENT_DESC> = Vec::new();
        let mut ielem = 0;
        for elem in layout {
            d3d12_elems.push(D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PSTR(null_terminated_semantics[ielem].as_ptr() as _),
                SemanticIndex: elem.index,
                Format: to_dxgi_format(elem.format),
                InputSlot: elem.input_slot,
                AlignedByteOffset: elem.aligned_byte_offset,
                InputSlotClass: match elem.input_slot_class {
                    super::InputSlotClass::PerVertex => D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    super::InputSlotClass::PerInstance => {
                        D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA
                    }
                },
                InstanceDataStepRate: elem.step_rate,
            });
            ielem = ielem + 1;
        }
        d3d12_elems
    }

    fn create_root_signature(
        &self,
        layout: &super::DescriptorLayout,
    ) -> result::Result<ID3D12RootSignature, super::Error> {
        
        let mut root_params: Vec<D3D12_ROOT_PARAMETER> = Vec::new();

        // push constants
        if layout.push_constants.is_some() {
            let constants_set = layout.push_constants.as_ref();
            for constants in constants_set.unwrap() {
                root_params.push(D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Constants: D3D12_ROOT_CONSTANTS {
                            ShaderRegister: constants.shader_register,
                            RegisterSpace: constants.register_space,
                            Num32BitValues: constants.num_values,
                        },
                    },
                    ShaderVisibility: to_d3d12_shader_visibility(&constants.visibility),
                });
            }
        }

        // tables for (SRV, UAV, CBV an Samplers)
        let mut visibility_map : HashMap<super::ShaderVisibility, Vec<D3D12_DESCRIPTOR_RANGE>> = HashMap::new();
        if layout.tables.is_some() {
            let table_info = layout.tables.as_ref();
            for table in table_info.unwrap() {
                let count = if table.num_descriptors.is_some() {
                    table.num_descriptors.unwrap()
                } else {
                    u32::MAX
                };
                let range = D3D12_DESCRIPTOR_RANGE {
                    RangeType: match table.table_type {
                        super::DescriptorTableType::ShaderResource => {
                            D3D12_DESCRIPTOR_RANGE_TYPE_SRV
                        }
                        super::DescriptorTableType::UnorderedAccess => {
                            D3D12_DESCRIPTOR_RANGE_TYPE_UAV
                        }
                        super::DescriptorTableType::ConstantBuffer => {
                            D3D12_DESCRIPTOR_RANGE_TYPE_CBV
                        }
                        super::DescriptorTableType::Sampler => D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
                    },
                    NumDescriptors: count,
                    BaseShaderRegister: table.shader_register,
                    RegisterSpace: table.register_space,
                    OffsetInDescriptorsFromTableStart: 0,
                };

                let map = visibility_map.get_mut(&table.visibility);
                if map.is_some() {
                    map.unwrap().push(range);
                }
                else {
                    visibility_map.insert(table.visibility, vec![range]);
                }
            }

            for (visibility, ranges) in visibility_map.iter() {
                root_params.push(D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: ranges.len() as u32,
                            pDescriptorRanges: ranges.as_ptr() as *mut D3D12_DESCRIPTOR_RANGE,
                        },
                    },
                    ShaderVisibility: to_d3d12_shader_visibility(visibility)
                });
            }
        }

        // immutable samplers
        let mut static_samplers: Vec<D3D12_STATIC_SAMPLER_DESC> = Vec::new();
        if layout.static_samplers.is_some() {
            let samplers = layout.static_samplers.as_ref();
            for sampler in samplers.unwrap() {
                static_samplers.push(D3D12_STATIC_SAMPLER_DESC {
                    Filter: to_d3d12_filter(sampler.filter),
                    AddressU: to_d3d12_address_mode(sampler.address_u),
                    AddressV: to_d3d12_address_mode(sampler.address_v),
                    AddressW: to_d3d12_address_mode(sampler.address_w),
                    MipLODBias: sampler.mip_lod_bias,
                    MaxAnisotropy: sampler.max_aniso,
                    ComparisonFunc: to_d3d12_address_comparison_func(sampler.comparison),
                    BorderColor: to_d3d12_sampler_boarder_colour(sampler.border_colour),
                    MinLOD: sampler.min_lod,
                    MaxLOD: sampler.max_lod,
                    ShaderRegister: sampler.shader_register,
                    RegisterSpace: sampler.register_space,
                    ShaderVisibility: to_d3d12_shader_visibility(&sampler.visibility),
                })
            }
        }

        // desc
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: root_params.len() as u32,
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            pParameters: root_params.as_mut_ptr(),
            NumStaticSamplers: static_samplers.len() as u32,
            pStaticSamplers: static_samplers.as_mut_ptr(),
            ..Default::default()
        };

        unsafe {
            // serialise signature
            let mut signature = None;
            let mut error = None;
            let _ = D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature,
                &mut error,
            );

            // handle errors
            if error.is_some() {
                let blob = error.unwrap();
                return Err( super::Error {
                    error_type: super::ErrorType::DescriptorLayout,
                    msg: get_d3d12_error_blob_string(&blob)
                });
            }

            // create signature
            let sig = signature.unwrap();
            let sig = self.device.CreateRootSignature(0, sig.GetBufferPointer(), sig.GetBufferSize())?;
            Ok(sig)
        }
    }

    fn create_render_passes_for_swap_chain(&self, num_buffers: u32, textures: &Vec<Texture>) -> Vec<RenderPass> {
        let mut passes = Vec::new();
        for i in 0..num_buffers as usize {
            passes.push(self.create_render_pass(&super::RenderPassInfo {
                render_targets: vec![textures[i].clone()],
                rt_clear: Some(super::ClearColour {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                }),
                depth_stencil: None,
                ds_clear: None,
                resolve: false,
                discard: false,
            }).unwrap());
        }
        passes
    }
}

impl super::Device for Device {
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Buffer = Buffer;
    type Shader = Shader;
    type RenderPipeline = RenderPipeline;
    type Texture = Texture;
    type ReadBackRequest = ReadBackRequest;
    type RenderPass = RenderPass;
    type Heap = Heap;
    type ComputePipeline = ComputePipeline;
    fn create(info: &super::DeviceInfo) -> Device {
        unsafe {
            // enable debug layer
            let mut dxgi_factory_flags: u32 = 0;
            if cfg!(debug_assertions) {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                    debug.EnableDebugLayer();
                    println!("hotline::gfx::d3d12: enabling debug layer");
                }
                dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
            }

            // create dxgi factory
            let dxgi_factory = CreateDXGIFactory2(dxgi_factory_flags)
                .expect("hotline::gfx::d3d12: failed to create dxgi factory");

            // create adapter
            let (adapter, adapter_info) = get_hardware_adapter(&dxgi_factory, &info.adapter_name)
                .expect("hotline::gfx::d3d12: failed to get hardware adapter");

            // create device
            let mut d3d12_device: Option<ID3D12Device> = None;
            D3D12CreateDevice(adapter.clone(), D3D_FEATURE_LEVEL_11_0, &mut d3d12_device)
                .expect("hotline::gfx::d3d12: failed to create d3d12 device");
            let device = d3d12_device.unwrap();

            // create command allocator
            let command_allocator = device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                .expect("hotline::gfx::d3d12: failed to create command allocator");

            // create command list
            let command_list = device
                .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &command_allocator, None)
                .expect("hotline::gfx::d3d12: failed to create command list");

            // create queue
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                NodeMask: 1,
                ..Default::default()
            };
            let command_queue = device
                .CreateCommandQueue(&desc)
                .expect("hotline::gfx::d3d12: failed to create command queue");

            // default heaps

            // shader (srv, cbv, uav)
            let shader_heap = create_heap(&device, &HeapInfo {
                heap_type: super::HeapType::Shader,
                num_descriptors: info.shader_heap_size
            });

            // rtv
            let rtv_heap = create_heap(&device, &HeapInfo {
                heap_type: super::HeapType::RenderTarget,
                num_descriptors: info.render_target_heap_size
            });

            // dsv
            let dsv_heap = create_heap(&device, &HeapInfo {
                heap_type: super::HeapType::DepthStencil,
                num_descriptors: info.depth_stencil_heap_size
            });

            // initialise struct
            Device {
                _adapter: adapter,
                adapter_info: adapter_info,
                device: device,
                dxgi_factory: dxgi_factory,
                command_allocator: command_allocator,
                command_list: command_list,
                command_queue: command_queue,
                pix: WinPixEventRuntime::create(),
                shader_heap: shader_heap,
                rtv_heap: rtv_heap,
                dsv_heap: dsv_heap
            }
        }
    }

    fn create_heap(&self, info: &HeapInfo) -> Heap {
        create_heap(&self.device, &info)
    }

    fn create_swap_chain(&mut self, info: &super::SwapChainInfo, win: &platform::Window) -> SwapChain {
        unsafe {
            // set flags, these could be passed in
            let flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0;
            let format = super::Format::RGBA8n;
            let dxgi_format = to_dxgi_format(format);

            // create swap chain desc
            let rect = win.get_rect();
            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                BufferCount: info.num_buffers,
                Width: rect.width as u32,
                Height: rect.height as u32,
                Format: dxgi_format,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                Flags: flags as u32,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            // create swap chain itself
            let swap_chain1 = self
                .dxgi_factory
                .CreateSwapChainForHwnd(
                    &self.command_queue,
                    win.get_native_handle(),
                    &swap_chain_desc,
                    std::ptr::null(),
                    None,
                )
                .expect("hotline::gfx::d3d12: failed to create swap chain for window");
            let swap_chain: IDXGISwapChain3 = swap_chain1.cast().unwrap();

            // create rtv heap and handles
            let textures = create_swap_chain_rtv(&swap_chain, self, info.num_buffers);
            let data_size = (rect.width * rect.height * 4) as u64;
            let passes = self.create_render_passes_for_swap_chain(info.num_buffers, &textures);

            SwapChain {
                width: rect.width,
                height: rect.height,
                format: format,
                num_bb: info.num_buffers,
                flags: flags as u32,
                bb_index: 0,
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
                fence_last_signalled_value: 0,
                fence_event: CreateEventA(std::ptr::null_mut(), false, false, None),
                swap_chain: swap_chain,
                backbuffer_textures: textures,
                backbuffer_passes: passes,
                frame_index: 0,
                frame_fence_value: vec![0; info.num_buffers as usize],
                readback_buffer: create_read_back_buffer(&self, data_size),
            }
        }
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> CmdBuf {
        unsafe {
            let mut command_allocators: Vec<ID3D12CommandAllocator> = Vec::new();
            let mut command_lists: Vec<ID3D12GraphicsCommandList> = Vec::new();
            let mut barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>> = Vec::new();

            for _ in 0..num_buffers as usize {
                // create command allocator
                let command_allocator = self
                    .device
                    .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                    .expect("hotline::gfx::d3d12: failed to create command allocator");

                // create command list
                let command_list = self
                    .device
                    .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &command_allocator, None)
                    .expect("hotline::gfx::d3d12: failed to create command list");

                command_allocators.push(command_allocator);
                command_lists.push(command_list);

                barriers.push(Vec::new());
            }

            CmdBuf {
                bb_index: 1,
                command_allocator: command_allocators,
                command_list: command_lists,
                pix: self.pix,
                in_flight_barriers: barriers,
            }
        }
    }

    fn create_render_pipeline(&self, info: &super::RenderPipelineInfo<Device>) -> result::Result<RenderPipeline, super::Error> {
        let root_signature = self.create_root_signature(&info.descriptor_layout)?;

        // TODO: select which shaders
        let vs = &info.vs.as_ref().unwrap().blob;
        let ps = &info.fs.as_ref().unwrap().blob;

        let semantics = null_terminate_semantics(&info.input_layout);
        let mut elems = Device::create_d3d12_input_element_desc(&info.input_layout, &semantics);
        let input_layout = D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: elems.as_mut_ptr(),
            NumElements: elems.len() as u32,
        };

        let raster = &info.raster_info;
        let depth_stencil = &info.depth_stencil_info;
        let blend = &info.blend_info;

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            InputLayout: input_layout,
            pRootSignature: Some(root_signature.clone()),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { vs.GetBufferPointer() },
                BytecodeLength: unsafe { vs.GetBufferSize() },
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { ps.GetBufferPointer() },
                BytecodeLength: unsafe { ps.GetBufferSize() },
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: to_d3d12_fill_mode(&raster.fill_mode),
                CullMode: to_d3d12_cull_mode(&raster.cull_mode),
                FrontCounterClockwise: BOOL::from(raster.front_ccw),
                DepthBias: raster.depth_bias,
                DepthBiasClamp: raster.depth_bias_clamp,
                SlopeScaledDepthBias: raster.slope_scaled_depth_bias,
                DepthClipEnable: BOOL::from(raster.front_ccw),
                MultisampleEnable: BOOL::from(raster.front_ccw),
                AntialiasedLineEnable: BOOL::from(raster.front_ccw),
                ForcedSampleCount: raster.forced_sample_count,
                ConservativeRaster: if raster.conservative_raster_mode { 
                    D3D12_CONSERVATIVE_RASTERIZATION_MODE_ON 
                } else { 
                    D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF 
                }
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: BOOL::from(blend.alpha_to_coverage_enabled),
                IndependentBlendEnable: BOOL::from(blend.independant_blend_enabled),
                RenderTarget: to_d3d12_render_target_blend(&blend.render_target),
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC {
                DepthEnable: BOOL::from(depth_stencil.depth_enabled),
                DepthWriteMask: to_d3d12_write_mask(&depth_stencil.depth_write_mask),
                DepthFunc: to_d3d12_comparison_func(depth_stencil.depth_func),
                StencilEnable: BOOL::from(depth_stencil.stencil_enabled),
                StencilReadMask: depth_stencil.stencil_read_mask,
                StencilWriteMask: depth_stencil.stencil_write_mask,
                FrontFace: D3D12_DEPTH_STENCILOP_DESC {
                    StencilFailOp: to_d3d12_stencil_op(&depth_stencil.front_face.fail),
                    StencilDepthFailOp: to_d3d12_stencil_op(&depth_stencil.front_face.depth_fail),
                    StencilPassOp: to_d3d12_stencil_op(&depth_stencil.front_face.pass),
                    StencilFunc: to_d3d12_comparison_func(depth_stencil.front_face.func),
                },
                BackFace: D3D12_DEPTH_STENCILOP_DESC {
                    StencilFailOp: to_d3d12_stencil_op(&depth_stencil.back_face.fail),
                    StencilDepthFailOp: to_d3d12_stencil_op(&depth_stencil.back_face.depth_fail),
                    StencilPassOp: to_d3d12_stencil_op(&depth_stencil.back_face.pass),
                    StencilFunc: to_d3d12_comparison_func(depth_stencil.back_face.func),
                },
            },
            SampleMask: u32::max_value(), // TODO:
            PrimitiveTopologyType: to_d3d12_primitive_topology_type(info.topology),
            NumRenderTargets: info.pass.rt_formats.len() as u32,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: info.pass.sample_count,
                Quality: 0,
            },
            ..Default::default()
        };

        // Set formats from pass
        for i in 0..info.pass.rt_formats.len() {
            desc.RTVFormats[i] = info.pass.rt_formats[i];
        }

        Ok(RenderPipeline {
            pso: unsafe { self.device.CreateGraphicsPipelineState(&desc)? },
            root_signature: root_signature,
            topology: to_d3d12_primitive_topology(info.topology, info.patch_index)
        })
    }

    fn create_shader<T: Sized>(
        &self,
        info: &super::ShaderInfo,
        src: &[T],
    ) -> std::result::Result<Shader, super::Error> {
        let mut shader_blob = None;
        if info.compile_info.is_some() {
            let compile_info = info.compile_info.as_ref().unwrap();
            let compile_flags = to_d3d12_compile_flags(&compile_info.flags);
            unsafe {
                let nullt_entry_point = CString::new(compile_info.entry_point.clone())?;
                let nullt_target = CString::new(compile_info.target.clone())?;
                let src_u8 = slice_as_u8_slice(src);
                let nullt_data = CString::new(src_u8)?;
                let mut errors = None;
                let result = D3DCompile(
                    nullt_data.as_ptr() as *const core::ffi::c_void,
                    src_u8.len(),
                    PSTR(std::ptr::null_mut() as _),
                    std::ptr::null(),
                    None,
                    PSTR(nullt_entry_point.as_ptr() as _),
                    PSTR(nullt_target.as_ptr() as _),
                    compile_flags,
                    0,
                    &mut shader_blob,
                    &mut errors,
                );
                if !result.is_ok() {
                    if errors.is_some() {
                        let w = errors.unwrap();
                        let buf = w.GetBufferPointer();
                        let c_str: &CStr = CStr::from_ptr(buf as *const i8);
                        let str_slice: &str = c_str.to_str().unwrap();
                        return Err(super::Error {
                            error_type: super::ErrorType::ShaderCompile,
                            msg: String::from(str_slice),
                        });
                    }
                    panic!("hotline::gfx::d3d12: shader compile failed with no error information!");
                }
            }
        }
        Ok(Shader {
            blob: shader_blob.unwrap(),
        })
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        let mut buf: Option<ID3D12Resource> = None;
        let dxgi_format = to_dxgi_format(info.format);
        let size_bytes = info.stride * info.num_elements;
        validate_data_size(size_bytes, data)?;
        unsafe {
            self.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: size_bytes as u64,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COPY_DEST,
                std::ptr::null(),
                &mut buf,
            )?;

            if data.is_some() {
                let mut upload: Option<ID3D12Resource> = None;
                self.device.CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_UPLOAD,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Alignment: 0,
                        Width: size_bytes as u64,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        Format: DXGI_FORMAT_UNKNOWN,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        Flags: D3D12_RESOURCE_FLAG_NONE,
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    std::ptr::null(),
                    &mut upload,
                )?;

                // copy data to upload buffer
                let range = D3D12_RANGE {
                    Begin: 0,
                    End: size_bytes as usize,
                };
                let mut map_data = std::ptr::null_mut();
                let res = upload.clone().unwrap();
                res.Map(0, &range, &mut map_data)?;
                if map_data != std::ptr::null_mut() {
                    let src = data.as_ref().unwrap().as_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(src, map_data as *mut u8, size_bytes as usize);
                }
                res.Unmap(0, std::ptr::null());

                // copy resource
                let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();

                self.command_list.CopyResource(&buf, upload);

                let barrier = transition_barrier(
                    &buf.clone().unwrap(),
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                );

                // transition to shader resource
                self.command_list.ResourceBarrier(1, &barrier);
                self.command_list.Close()?;

                let cmd = ID3D12CommandList::from(&self.command_list);
                self.command_queue.ExecuteCommandLists(1, &mut Some(cmd));
                self.command_queue.Signal(&fence, 1)?;

                let event = CreateEventA(std::ptr::null_mut(), false, false, None);
                fence.SetEventOnCompletion(1, event)?;
                WaitForSingleObject(event, INFINITE);

                self.command_list.Reset(&self.command_allocator, None)?;
                let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                    std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
            }

            // create optional views
            let mut vbv: Option<D3D12_VERTEX_BUFFER_VIEW> = None;
            let mut ibv: Option<D3D12_INDEX_BUFFER_VIEW> = None;
            let mut srv: Option<D3D12_CPU_DESCRIPTOR_HANDLE> = None;
            let mut srv_index = usize::MAX;

            match info.usage {
                super::BufferUsage::Vertex => {
                    vbv = Some(D3D12_VERTEX_BUFFER_VIEW {
                        BufferLocation: buf.clone().unwrap().GetGPUVirtualAddress(),
                        StrideInBytes: info.stride as u32,
                        SizeInBytes: size_bytes as u32,
                    });
                }
                super::BufferUsage::Index => {
                    ibv = Some(D3D12_INDEX_BUFFER_VIEW {
                        BufferLocation: buf.clone().unwrap().GetGPUVirtualAddress(),
                        SizeInBytes: size_bytes as u32,
                        Format: dxgi_format,
                    })
                }
                super::BufferUsage::ConstantBuffer => {
                    let h = self.shader_heap.allocate();
                    self.device.CreateConstantBufferView(
                        &D3D12_CONSTANT_BUFFER_VIEW_DESC {
                            BufferLocation: buf.clone().unwrap().GetGPUVirtualAddress(),
                            SizeInBytes: size_bytes as u32,
                        },
                        &h,
                    );
                    srv = Some(h);
                    srv_index = self.shader_heap.get_handle_index(&h);
                }
            }

            Ok(Buffer {
                resource: buf.unwrap(),
                vbv: vbv,
                ibv: ibv,
                srv: srv,
                srv_index: srv_index
            })
        }
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {
        let mut tex: Option<ID3D12Resource> = None;
        let dxgi_format = to_dxgi_format(info.format);
        let size_bytes = size_for_format(info.format, info.width, info.height, info.depth) as usize;
        validate_data_size(size_bytes, data)?;
        let initial_state = to_d3d12_resource_state(info.initial_state);
        unsafe {
            // create texture resource
            self.device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: match info.tex_type {
                            super::TextureType::Texture1D => D3D12_RESOURCE_DIMENSION_TEXTURE1D,
                            super::TextureType::Texture2D => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                            super::TextureType::Texture3D => D3D12_RESOURCE_DIMENSION_TEXTURE3D,
                        },
                        Alignment: 0,
                        Width: info.width as u64,
                        Height: info.height as u32,
                        DepthOrArraySize: info.depth as u16,
                        MipLevels: info.mip_levels as u16,
                        Format: dxgi_format,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: info.samples as u32,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                        Flags: to_d3d12_texture_usage_flags(info.usage),
                    },
                    if data.is_some() { D3D12_RESOURCE_STATE_COPY_DEST } else { initial_state },
                    std::ptr::null(),
                    &mut tex,
                )?;

            if data.is_some() {
                let data = data.unwrap();

                // create upload buffer
                let row_pitch = super::row_pitch_for_format(info.format, info.width);
                let upload_pitch =
                    super::align_pow2(row_pitch, D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u64);
                let upload_size = info.height * upload_pitch;

                let mut upload: Option<ID3D12Resource> = None;
                self.device
                    .CreateCommittedResource(
                        &D3D12_HEAP_PROPERTIES {
                            Type: D3D12_HEAP_TYPE_UPLOAD,
                            ..Default::default()
                        },
                        D3D12_HEAP_FLAG_NONE,
                        &D3D12_RESOURCE_DESC {
                            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                            Alignment: 0,
                            Width: upload_size,
                            Height: 1,
                            DepthOrArraySize: 1,
                            MipLevels: 1,
                            Format: DXGI_FORMAT_UNKNOWN,
                            SampleDesc: DXGI_SAMPLE_DESC {
                                Count: 1,
                                Quality: 0,
                            },
                            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                            Flags: D3D12_RESOURCE_FLAG_NONE,
                        },
                        D3D12_RESOURCE_STATE_GENERIC_READ,
                        std::ptr::null(),
                        &mut upload,
                    )?;

                // copy data to upload buffer
                let range = D3D12_RANGE {
                    Begin: 0,
                    End: upload_size as usize,
                };
                let mut map_data = std::ptr::null_mut();
                let res = upload.clone().unwrap();
                res.Map(0, &range, &mut map_data)?;
                if map_data != std::ptr::null_mut() {
                    for y in 0..info.height {
                        let src = data.as_ptr().offset((y * info.width * 4) as isize) as *const u8;
                        let dst = (map_data as *mut u8).offset((y * upload_pitch) as isize);
                        std::ptr::copy_nonoverlapping(src, dst, (info.width * 4) as usize);
                    }
                }
                res.Unmap(0, std::ptr::null());

                // copy resource
                let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;

                let src = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: Some(upload.clone().unwrap()),
                    Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                            Offset: 0,
                            Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                                Width: info.width as u32,
                                Height: info.height as u32,
                                Depth: 1,
                                Format: dxgi_format,
                                RowPitch: upload_pitch as u32,
                            },
                        },
                    },
                };

                let dst = D3D12_TEXTURE_COPY_LOCATION {
                    pResource: Some(tex.clone().unwrap()),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: 0,
                    },
                };

                self.command_list.CopyTextureRegion(&dst, 0, 0, 0, &src, std::ptr::null_mut());

                let barrier = transition_barrier(
                    &tex.clone().unwrap(),
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    initial_state,
                );

                // transition to shader resource
                self.command_list.ResourceBarrier(1, &barrier);
                let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                    std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);

                self.command_list.Close()?;

                let cmd = ID3D12CommandList::from(&self.command_list);
                self.command_queue.ExecuteCommandLists(1, &mut Some(cmd));
                self.command_queue.Signal(&fence, 1)?;

                let event = CreateEventA(std::ptr::null_mut(), false, false, None);
                fence.SetEventOnCompletion(1, event)?;
                WaitForSingleObject(event, INFINITE);
                self.command_list.Reset(&self.command_allocator, None)?;
            }
            
            // create srv
            let mut srv_handle = None;
            let mut srv_index = usize::MAX;
            if info.usage.contains(super::TextureUsage::SHADER_RESOURCE) {
                let h = self.shader_heap.allocate();
                self.device.CreateShaderResourceView(
                    &tex,
                    &D3D12_SHADER_RESOURCE_VIEW_DESC {
                        Format: dxgi_format,
                        ViewDimension: match info.tex_type {
                            super::TextureType::Texture1D => D3D12_SRV_DIMENSION_TEXTURE1D,
                            super::TextureType::Texture2D => D3D12_SRV_DIMENSION_TEXTURE2D,
                            super::TextureType::Texture3D => D3D12_SRV_DIMENSION_TEXTURE3D,
                        },
                        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D12_TEX2D_SRV {
                                MipLevels: info.mip_levels,
                                MostDetailedMip: 0,
                                ..Default::default()
                            },
                        },
                        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    },
                    &h,
                );
                srv_handle = Some(h);
                srv_index = self.shader_heap.get_handle_index(&h);
            }

            // create rtv
            let mut rtv_handle = None;
            if info.usage.contains(super::TextureUsage::RENDER_TARGET) {
                let h = self.rtv_heap.allocate();
                self.device.CreateRenderTargetView(&tex.clone().unwrap(), std::ptr::null_mut(), &h);
                rtv_handle = Some(h);
            }

            // create dsv
            let mut dsv_handle = None;
            if info.usage.contains(super::TextureUsage::DEPTH_STENCIL) {
                let h = self.dsv_heap.allocate();
                self.device.CreateDepthStencilView(&tex.clone().unwrap(), std::ptr::null_mut(), &h);
                dsv_handle = Some(h);
            }

            // create uav
            let mut uav_handle = None;
            if info.usage.contains(super::TextureUsage::UNORDERED_ACCESS) {
                let h = self.shader_heap.allocate();
                self.device.CreateUnorderedAccessView(
                    &tex.clone().unwrap(), 
                    None, 
                    std::ptr::null_mut(),
                    &h
                );
                uav_handle = Some(h);
            }

            Ok(Texture {
                resource: tex.unwrap(),
                rtv: rtv_handle,
                dsv: dsv_handle,
                srv: srv_handle,
                uav: uav_handle,
                srv_index: srv_index
            })
        }
    }

    fn create_render_pass(&self, info: &super::RenderPassInfo<Device>) -> result::Result<RenderPass, super::Error> {
        let mut rt: Vec<D3D12_RENDER_PASS_RENDER_TARGET_DESC> = Vec::new();
        let mut formats: Vec<DXGI_FORMAT> = Vec::new();
        let mut begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE;
        let mut clear_col = ClearColour {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        };
        let end_type = D3D12_RENDER_PASS_ENDING_ACCESS_TYPE_PRESERVE;
        if info.rt_clear.is_some() {
            begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR;
            clear_col = info.rt_clear.unwrap();
        } else if info.discard {
            begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD;
        }
        let mut sample_count = None;
        for target in &info.render_targets {
            let desc = unsafe { target.resource.GetDesc() };
            let dxgi_format = desc.Format;
            let target_sample_count = desc.SampleDesc.Count;
            if sample_count.is_none() {
                sample_count = Some(target_sample_count);
            }
            else {
                if sample_count.unwrap() != target_sample_count {
                    return Err( super::Error {
                        error_type: super::ErrorType::RenderPass,
                        msg: format!("Sample counts must match on all targets: expected {} samples, found {}", 
                        sample_count.unwrap(),
                        target_sample_count
                    )});
                }
            }
            let begin = D3D12_RENDER_PASS_BEGINNING_ACCESS {
                Type: begin_type,
                Anonymous: D3D12_RENDER_PASS_BEGINNING_ACCESS_0 {
                    Clear: D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS {
                        ClearValue: D3D12_CLEAR_VALUE {
                            Format: dxgi_format,
                            Anonymous: D3D12_CLEAR_VALUE_0 {
                                Color: [clear_col.r, clear_col.g, clear_col.b, clear_col.a],
                            },
                        },
                    },
                },
            };
            let end = D3D12_RENDER_PASS_ENDING_ACCESS {
                Type: end_type,
                Anonymous: D3D12_RENDER_PASS_ENDING_ACCESS_0 {
                    Resolve: Default::default(),
                },
            };
            formats.push(dxgi_format);
            rt.push(D3D12_RENDER_PASS_RENDER_TARGET_DESC {
                cpuDescriptor: target.rtv.unwrap(),
                BeginningAccess: begin,
                EndingAccess: end,
            })
        }

        let mut ds = None;
        let mut ds_format = DXGI_FORMAT_UNKNOWN;
        if info.depth_stencil.is_some() {
            let mut clear_depth = 0.0;
            let mut depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE;
            if info.ds_clear.is_some() {
                let ds_clear = info.ds_clear.as_ref().unwrap();
                if ds_clear.depth.is_some() {
                    depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR;
                    clear_depth = *ds_clear.depth.as_ref().unwrap();
                }
            } else if info.discard {
                depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD;
            }

            let mut clear_stencil = 0x0;
            let mut stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE;
            if info.ds_clear.is_some() {
                let ds_clear = info.ds_clear.as_ref().unwrap();
                if ds_clear.stencil.is_some() {
                    stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR;
                    clear_stencil = *ds_clear.stencil.as_ref().unwrap();
                }
            } else if info.discard {
                stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD;
            }

            let depth_stencil = info.depth_stencil.as_ref().unwrap();
            let desc = unsafe { depth_stencil.resource.GetDesc() };
            ds_format = desc.Format;

            let depth_begin = D3D12_RENDER_PASS_BEGINNING_ACCESS {
                Type: depth_begin_type,
                Anonymous: D3D12_RENDER_PASS_BEGINNING_ACCESS_0 {
                    Clear: D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS {
                        ClearValue: D3D12_CLEAR_VALUE {
                            Format: ds_format,
                            Anonymous: D3D12_CLEAR_VALUE_0 {
                                DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                                    Depth: clear_depth,
                                    Stencil: clear_stencil
                                }
                            },
                        },
                    },
                },
            };
            let depth_end = D3D12_RENDER_PASS_ENDING_ACCESS {
                Type: end_type,
                Anonymous: D3D12_RENDER_PASS_ENDING_ACCESS_0 {
                    Resolve: Default::default(),
                },
            };

            let stencil_begin = D3D12_RENDER_PASS_BEGINNING_ACCESS {
                Type: stencil_begin_type,
                Anonymous: D3D12_RENDER_PASS_BEGINNING_ACCESS_0 {
                    Clear: D3D12_RENDER_PASS_BEGINNING_ACCESS_CLEAR_PARAMETERS {
                        ClearValue: D3D12_CLEAR_VALUE {
                            Format: ds_format,
                            Anonymous: D3D12_CLEAR_VALUE_0 {
                                DepthStencil: D3D12_DEPTH_STENCIL_VALUE {
                                    Depth: clear_depth,
                                    Stencil: clear_stencil
                                }
                            },
                        },
                    },
                },
            };
            let stencil_end = D3D12_RENDER_PASS_ENDING_ACCESS {
                Type: end_type,
                Anonymous: D3D12_RENDER_PASS_ENDING_ACCESS_0 {
                    Resolve: Default::default(),
                },
            };

            ds = Some( D3D12_RENDER_PASS_DEPTH_STENCIL_DESC {
                cpuDescriptor: info.depth_stencil.as_ref().unwrap().dsv.unwrap(),
                DepthBeginningAccess: depth_begin,
                StencilBeginningAccess: stencil_begin,
                DepthEndingAccess: depth_end,
                StencilEndingAccess: stencil_end
            });
        }

        Ok(RenderPass {
            rt: rt,
            ds: ds,
            ds_format: ds_format,
            rt_formats: formats,
            sample_count: sample_count.unwrap()
        })
    }

    fn create_compute_pipeline(&self, info: &super::ComputePipelineInfo<Self>) -> result::Result<ComputePipeline, super::Error> {
        
        let cs = &info.cs.blob;
        let root_signature = self.create_root_signature(&info.descriptor_layout)?;

        let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { cs.GetBufferPointer() },
                BytecodeLength: unsafe { cs.GetBufferSize() },
            },
            pRootSignature: Some(root_signature.clone()),
            ..Default::default()
        };

        unsafe {
            Ok(ComputePipeline{
                pso: self.device.CreateComputePipelineState(&desc)?,
                root_signature:root_signature
            }) 
        }
    }

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = ID3D12CommandList::from(&cmd.command_list[cmd.bb_index]);
            self.command_queue.ExecuteCommandLists(1, &mut Some(command_list));
        }
    }

    fn get_shader_heap(&self) -> &Self::Heap {
        &self.shader_heap
    }

    fn get_adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }
}

impl SwapChain {
    fn wait_for_frame(&mut self, frame_index: usize) {
        unsafe {
            let mut waitable: [HANDLE; 2] =
                [self.swap_chain.GetFrameLatencyWaitableObject(), HANDLE(0)];
            let mut num_waitable = 1;
            let mut fv = self.frame_fence_value[frame_index as usize];

            // 0 means no fence was signaled
            if fv != 0 {
                fv = 0;
                self.fence.SetEventOnCompletion(fv as u64, self.fence_event)
                    .expect("hotline::gfx::d3d12: failed to set on completion event!");
                waitable[1] = self.fence_event;
                num_waitable = 2;
            }

            WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
        }
    }
}

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
        self.wait_for_frame(self.bb_index as usize);
    }

    fn update(&mut self, device: &mut Device, window: &platform::Window, cmd: &mut CmdBuf) {
        let (width, height) = window.get_size();
        if width != self.width || height != self.height {
            unsafe {
                self.wait_for_frame(self.bb_index as usize);
                cmd.drop_complete_in_flight_barriers(cmd.bb_index);

                // clean up rtv handles
                for bb_tex in &self.backbuffer_textures {
                    if bb_tex.rtv.is_some() {
                        device.rtv_heap.deallocate(&bb_tex.rtv.unwrap());
                    } 
                }

                // clean up texture resource
                self.backbuffer_textures.clear();

                self.swap_chain
                    .ResizeBuffers(
                        self.num_bb,
                        width as u32,
                        height as u32,
                        DXGI_FORMAT_UNKNOWN,
                        self.flags,
                    )
                    .expect("hotline::gfx::d3d12: warning: present failed!");

                let data_size = super::slice_pitch_for_format(self.format, self.width as u64, self.height as u64);
                self.backbuffer_textures = create_swap_chain_rtv(&self.swap_chain, device, self.num_bb);
                self.backbuffer_passes = device.create_render_passes_for_swap_chain(self.num_bb, &self.backbuffer_textures);
                self.readback_buffer = create_read_back_buffer(&device, data_size);
                self.width = width;
                self.height = height;
                self.bb_index = 0;
            }
        } else {
            self.new_frame();
        }
    }

    fn get_backbuffer_index(&self) -> i32 {
        self.bb_index
    }

    fn get_backbuffer_texture(&self) -> &Texture {
        return &self.backbuffer_textures[self.bb_index as usize];
    }

    fn get_backbuffer_pass(&mut self) -> &RenderPass {
        return &self.backbuffer_passes[self.bb_index as usize];
    }

    fn get_backbuffer_pass_mut(&mut self) -> &mut RenderPass {
        return &mut self.backbuffer_passes[self.bb_index as usize];
    }

    fn swap(&mut self, device: &Device) {
        unsafe {
            // present
            self.swap_chain.Present(1, 0).expect("hotline::gfx::d3d12: warning: present failed!");

            // signal fence
            let fv = self.fence_last_signalled_value + 1;
            device
                .command_queue
                .Signal(&self.fence, fv as u64)
                .expect("hotline::gfx::d3d12: warning: command_queue.Signal failed!");

            // update fence tracking
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.bb_index as usize] = fv;

            // swap buffers
            self.frame_index = self.frame_index + 1;
            self.bb_index = (self.bb_index + 1) % self.num_bb as i32;
        }
    }
}

impl CmdBuf {
    fn cmd(&self) -> &ID3D12GraphicsCommandList {
        &self.command_list[self.bb_index]
    }

    fn drop_complete_in_flight_barriers(&mut self, bb: usize) {
        let size = self.in_flight_barriers[bb].len();
        for i in (0..size).rev() {
            let barrier = self.in_flight_barriers[bb].remove(i);
            unsafe {
                let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                    std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
            }
        }
        self.in_flight_barriers[bb].clear();
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        let prev_bb = self.bb_index;
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        self.bb_index = bb;
        if swap_chain.frame_fence_value[bb] != 0 {
            unsafe {
                self.command_allocator[bb].Reset().expect("hotline::gfx::d3d12: failed to reset command_allocator!");
                self.command_list[bb].Reset(&self.command_allocator[bb], None)
                    .expect("hotline::gfx::d3d12: failed to reset command_list!");
            }
        }
        self.drop_complete_in_flight_barriers(prev_bb);
    }

    fn begin_render_pass(&self, render_pass: &mut RenderPass) {
        unsafe {
            let cmd4: ID3D12GraphicsCommandList4 = self.cmd().cast().unwrap();
            // TODO: fix this branch
            if render_pass.ds.is_some() {
                cmd4.BeginRenderPass(
                    render_pass.rt.len() as u32,
                    render_pass.rt.as_mut_ptr(),
                    render_pass.ds.as_ref().unwrap(),
                    D3D12_RENDER_PASS_FLAG_NONE,
                );
            }
            else {
                cmd4.BeginRenderPass(
                    render_pass.rt.len() as u32,
                    render_pass.rt.as_mut_ptr(),
                    std::ptr::null_mut(),
                    D3D12_RENDER_PASS_FLAG_NONE,
                );
            }
        }
    }

    fn end_render_pass(&self) {
        unsafe {
            let cmd4: ID3D12GraphicsCommandList4 = self.cmd().cast().unwrap();
            cmd4.EndRenderPass();
        }
    }

    fn begin_event(&self, colour: u32, name: &str) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().begin_event_on_command_list(cmd.clone(), colour as u64, name);
        }
    }

    fn end_event(&self) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().end_event_on_command_list(cmd.clone());
        }
    }

    fn transition_barrier(&mut self, barrier: &TransitionBarrier<Device>) {
        if barrier.texture.is_some() {
            let tex = barrier.texture.as_ref().unwrap();
            let barrier = transition_barrier(
                &tex.resource,
                to_d3d12_resource_state(barrier.state_before),
                to_d3d12_resource_state(barrier.state_after),
            );
            unsafe {
                let bb = self.bb_index;
                self.command_list[bb].ResourceBarrier(1, &barrier);
                self.in_flight_barriers[bb].push(barrier);
            }
        }
    }

    fn set_viewport(&self, viewport: &super::Viewport) {
        let d3d12_vp = D3D12_VIEWPORT {
            TopLeftX: viewport.x,
            TopLeftY: viewport.y,
            Width: viewport.width,
            Height: viewport.height,
            MinDepth: viewport.min_depth,
            MaxDepth: viewport.max_depth,
        };
        unsafe {
            self.cmd().RSSetViewports(1, &d3d12_vp);
        }
    }

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
        let d3d12_sr = RECT {
            left: scissor_rect.left,
            top: scissor_rect.top,
            right: scissor_rect.right,
            bottom: scissor_rect.bottom,
        };
        unsafe {
            self.cmd().RSSetScissorRects(1, &d3d12_sr);
        }
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        let cmd = self.cmd();
        if buffer.vbv.is_some() {
            unsafe {
                cmd.IASetVertexBuffers(slot, 1, &buffer.vbv.unwrap());
            }
        }
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
        let cmd = self.cmd();
        if buffer.ibv.is_some() {
            unsafe {
                cmd.IASetIndexBuffer(&buffer.ibv.unwrap());
            }
        }
    }

    fn set_render_pipeline(&self, pipeline: &RenderPipeline) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetGraphicsRootSignature(&pipeline.root_signature);
            cmd.SetPipelineState(&pipeline.pso);
            cmd.IASetPrimitiveTopology(pipeline.topology)
        }
    }

    fn set_compute_pipeline(&self, pipeline: &ComputePipeline) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetComputeRootSignature(&pipeline.root_signature);
            cmd.SetPipelineState(&pipeline.pso);
        }
    }

    fn set_compute_heap(&self, slot: u32, heap: &Heap) {
        unsafe {
            self.cmd().SetDescriptorHeaps(1, &Some(heap.heap.clone()));
            self.cmd().SetComputeRootDescriptorTable(
                slot,
                &heap.heap.GetGPUDescriptorHandleForHeapStart(),
            );
        }
    }

    fn set_render_heap(&self, slot: u32, heap: &Heap) {
        unsafe {
            self.cmd().SetDescriptorHeaps(1, &Some(heap.heap.clone()));
            self.cmd().SetGraphicsRootDescriptorTable(
                slot,
                &heap.heap.GetGPUDescriptorHandleForHeapStart(),
            );
        }
    }

    fn set_marker(&self, colour: u32, name: &str) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().set_marker_on_command_list(cmd.clone(), colour as u64, name);
        }
    }   

    fn push_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetGraphicsRoot32BitConstants(
                slot,
                num_values,
                data.as_ptr() as *const ::core::ffi::c_void,
                dest_offset,
            )
        }
    }

    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawInstanced(vertex_count, instance_count, start_vertex, start_instance);
        }
    }

    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawIndexedInstanced(
                index_count,
                instance_count,
                start_index,
                base_vertex,
                start_instance,
            );
        }
    }

    fn dispatch(&self, group_count: Size3, thread_count: Size3) {
        unsafe {
            self.cmd().Dispatch(group_count.x, group_count.y, group_count.z);
        }
    }

    fn close(&mut self, swap_chain: &SwapChain) {
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        unsafe {
            self.command_list[bb].Close().expect("hotline: d3d12 failed to close command list.");
        }
    }

    fn read_back_backbuffer(&mut self, swap_chain: &SwapChain) -> ReadBackRequest {
        let bb = self.bb_index;
        let bbz = self.bb_index as u32;
        unsafe {
            let resource = swap_chain.swap_chain.GetBuffer(bbz);
            let r2 = resource.as_ref();

            // transition to copy source
            let barrier = transition_barrier(
                &r2.unwrap(),
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            );
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(resource.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };

            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(swap_chain.readback_buffer.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Width: swap_chain.width as u32,
                            Height: swap_chain.height as u32,
                            Depth: 1,
                            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                            RowPitch: (swap_chain.width * 4) as u32,
                        },
                    },
                },
            };

            self.command_list[bb].CopyTextureRegion(&dst, 0, 0, 0, &src, std::ptr::null_mut());

            let barrier = transition_barrier(
                &r2.unwrap(),
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );

            // transition back to render target
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            ReadBackRequest {
                resource: Some(swap_chain.readback_buffer.clone().unwrap()),
                fence_value: swap_chain.frame_index as u64,
                size: (swap_chain.width * swap_chain.height * 4) as usize,
                row_pitch: (swap_chain.width * 4) as usize,
                slice_pitch: (swap_chain.width * swap_chain.height * 4) as usize,
            }
        }
    }
}

impl super::Buffer<Device> for Buffer {
    fn get_srv_index(&self) -> usize {
        self.srv_index
    }
}

impl super::Texture<Device> for Texture {
    fn get_srv_index(&self) -> usize {
        self.srv_index
    }
}

impl super::ReadBackRequest<Device> for ReadBackRequest {
    fn is_complete(&self, swap_chain: &SwapChain) -> bool {
        if swap_chain.frame_index as u64 > self.fence_value + 1 {
            return true;
        }
        false
    }

    fn get_data(&self) -> std::result::Result<super::ReadBackData, &str> {
        let range = D3D12_RANGE {
            Begin: 0,
            End: self.size,
        };
        let mut map_data = std::ptr::null_mut();
        unsafe {
            let res = self.resource.as_ref().unwrap();
            res.Map(0, &range, &mut map_data).map_err(|_| "hotline::gfx::d3d12: map failed!")?;
            if map_data != std::ptr::null_mut() {
                let slice = std::slice::from_raw_parts(map_data as *const u8, self.size);
                let rb_data = super::ReadBackData {
                    data: slice,
                    size: self.size,
                    format: super::Format::Unknown,
                    row_pitch: self.row_pitch,
                    slice_pitch: self.size,
                };
                return Ok(rb_data);
            } else {
                return Err("hotline::gfx::d3d12: map failed!");
            }
            // TODO: ownership
            //res.Unmap(0, std::ptr::null());
        }
    }
}

impl super::ComputePipeline<Device> for ComputePipeline {

}