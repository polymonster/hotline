#![cfg(target_os = "windows")]

// some hardcoded switches
const MANAGE_DROPS : bool = true;
const GPU_VALIDATION : bool = false;

use crate::os::Window;
use crate::os::NativeHandle;

// use super::Shader;
use super::*;
use super::Device as SuperDevice;
use super::ReadBackRequest as SuperReadBackRequest;
use super::Heap as SuperHeap;
use super::Pipeline as SuperPipleline;

use std::char::{decode_utf16, REPLACEMENT_CHARACTER};
use std::hash::{Hash, Hasher};
use std::collections::{HashMap, hash_map::DefaultHasher};
use std::ffi::{CStr, CString, c_void};
use std::result;
use std::str;
use std::sync::Mutex;

use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D::Fxc::*, Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, Win32::System::Threading::*
};

#[macro_export]
macro_rules! d3d12_debug_name {
    ($object:expr, $name:expr) => {
        let obj : ID3D12Object = $object.cast().unwrap();
        let wstr = os::win32::string_to_wide($name.to_string());
        obj.SetName(PCWSTR(wstr.as_ptr() as _)).unwrap();
    }
}

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
            let module = LoadLibraryA(PCSTR("WinPixEventRuntime.dll\0".as_ptr() as _));
            if let Ok(module) = module {
                let p_begin_event = GetProcAddress(module, PCSTR("PIXBeginEventOnCommandList\0".as_ptr() as _));
                let p_end_event = GetProcAddress(module, PCSTR("PIXEndEventOnCommandList\0".as_ptr() as _));
                let p_set_marker = GetProcAddress(module, PCSTR("PIXSetMarkerOnCommandList\0".as_ptr() as _));
                if let (Some(begin), Some(end), Some(marker)) = (p_begin_event, p_end_event, p_set_marker) {
                    Some(WinPixEventRuntime {
                        begin_event: std::mem::transmute::<*const usize, BeginEventOnCommandList>(
                            begin as *const usize,
                        ),
                        end_event: std::mem::transmute::<*const usize, EndEventOnCommandList>(
                            end as *const usize,
                        ),
                        set_marker: std::mem::transmute::<*const usize, SetMarkerOnCommandList>(
                            marker as *const usize,
                        ),
                    })
                }
                else {
                    None
                }
            }
            else {
                None
            }
        }
    }

    pub fn begin_event_on_command_list(
        &self,
        command_list: &ID3D12GraphicsCommandList,
        color: u64,
        name: &str,
    ) {
        unsafe {
            let null_name = CString::new(name).unwrap();
            let fn_begin_event: BeginEventOnCommandList = self.begin_event;
            let p_cmd_list = 
                std::mem::transmute::<ID3D12GraphicsCommandList, *const core::ffi::c_void>(command_list.clone());
            fn_begin_event(p_cmd_list, color, PSTR(null_name.as_ptr() as _));
            // this ensures we drop the command_list ref
            let _cc = std::mem::transmute::<*const core::ffi::c_void, ID3D12GraphicsCommandList>(p_cmd_list); 
        }
    }

    pub fn end_event_on_command_list(&self, command_list: &ID3D12GraphicsCommandList) {
        unsafe {
            let fn_end_event: EndEventOnCommandList = self.end_event;
            let p_cmd_list = 
                std::mem::transmute::<ID3D12GraphicsCommandList, *const core::ffi::c_void>(command_list.clone());
            fn_end_event(p_cmd_list);
            // this ensures we drop the command_list ref
            let _cc = std::mem::transmute::<*const core::ffi::c_void, ID3D12GraphicsCommandList>(p_cmd_list); 
        }
    }

    pub fn set_marker_on_command_list(
        &self,
        command_list: &ID3D12GraphicsCommandList,
        color: u64,
        name: &str,
    ) {
        unsafe {
            let null_name = CString::new(name).unwrap();
            let fn_set_marker: SetMarkerOnCommandList = self.set_marker;
            let p_cmd_list = 
                std::mem::transmute::<ID3D12GraphicsCommandList, *const core::ffi::c_void>(command_list.clone());
            fn_set_marker(p_cmd_list, color, PSTR(null_name.as_ptr() as _));
            // this ensures we drop the command_list ref
            let _cc = std::mem::transmute::<*const core::ffi::c_void, ID3D12GraphicsCommandList>(p_cmd_list); 
        }
    }
}

type D3D12DeviceVersion = ID3D12Device;
type D3D12DebugVersion = ID3D12Debug;

#[derive(Clone)]
pub struct Device {
    adapter_info: super::AdapterInfo,
    feature_flags: super::DeviceFeatureFlags,
    dxgi_factory: IDXGIFactory4,
    device: D3D12DeviceVersion,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
    command_queue: ID3D12CommandQueue,
    pix: Option<WinPixEventRuntime>,
    shader_heap: Option<Heap>,
    rtv_heap: Heap,
    dsv_heap: Heap,
    timestamp_frequency: f64,
    generate_mip_maps_pipeline: Option<ComputePipeline>,
    heap_id: u16
}

#[derive(Clone)]
pub struct SwapChain {
    width: i32,
    height: i32,
    format: super::Format,
    num_bb: u32,
    flags: u32,
    frame_index: usize,
    bb_index: usize,
    swap_chain: IDXGISwapChain3,
    backbuffer_textures: Vec<Texture>,
    backbuffer_passes: Vec<RenderPass>,
    backbuffer_passes_no_clear: Vec<RenderPass>,
    fence: ID3D12Fence,
    fence_last_signalled_value: u64,
    fence_event: HANDLE,
    frame_fence_value: Vec<u64>,
    readback_buffer: Option<ID3D12Resource>,
    require_wait: Vec<bool>,
    clear_col: Option<ClearColour>,
}

#[derive(Clone)]
pub struct RenderPipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
    topology: D3D_PRIMITIVE_TOPOLOGY,
    lookup: RootSignatureLookup
}

#[derive(Clone)]
pub struct CmdBuf {
    bb_index: usize,
    command_allocator: Vec<ID3D12CommandAllocator>,
    command_list: Vec<ID3D12GraphicsCommandList>,
    needs_reset: Vec<bool>,
    pix: Option<WinPixEventRuntime>,
    in_flight_barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>>,
    event_stack_count: u32
}

#[derive(Clone)]
pub struct Buffer {
    resource: Option<ID3D12Resource>,
    vbv: Option<D3D12_VERTEX_BUFFER_VIEW>,
    ibv: Option<D3D12_INDEX_BUFFER_VIEW>,
    srv_index: Option<usize>,
    cbv_index: Option<usize>,
    uav_index: Option<usize>,
    counter_offset: Option<usize>,
    drop_list: Option<DropListRef>,
    persistent_mapped_data: *mut c_void
}

#[derive(Clone)]
pub struct Shader {
    blob: Option<ID3DBlob>,
    precompiled: Option<Vec<u8>>
}

#[derive(Clone)]
pub struct TextureTarget {
    ptr: D3D12_CPU_DESCRIPTOR_HANDLE,
    index: usize,
    drop_list: DropListRef,
}

#[derive(Clone)]
pub struct Texture {
    resource: Option<ID3D12Resource>,
    resolved_resource: Option<ID3D12Resource>,
    resolved_format: DXGI_FORMAT,
    rtv: Vec<TextureTarget>,
    dsv: Vec<TextureTarget>,
    srv_index: Option<usize>,
    resolved_srv_index: Option<usize>,
    uav_index: Option<usize>,
    subresource_uav_index: Vec<usize>,
    shared_handle: Option<HANDLE>,
    // drop list for srv, uav and resolved srv
    drop_list: Option<DropListRef>,
    // the id of the shader heap for (uav, srv etc)
    shader_heap_id: Option<u16>
}

#[derive(Clone)]
pub struct ReadBackRequest {
    pub resource: Option<ID3D12Resource>,
    pub fence_value: u64,
    pub size: usize,
    pub row_pitch: usize,
    pub slice_pitch: usize,
}

#[derive(Clone)]
pub struct RenderPass {
    rt: Vec<D3D12_RENDER_PASS_RENDER_TARGET_DESC>,
    rt_formats: Vec<DXGI_FORMAT>,
    ds: Option<D3D12_RENDER_PASS_DEPTH_STENCIL_DESC>,
    ds_format: DXGI_FORMAT,
    sample_count: u32,
    format_hash: u64 
}

#[derive(Clone)]
pub struct CommandSignature {
    command_signature: ID3D12CommandSignature
}

type DesciptorSlotLookup = HashMap<u64, PipelineSlotInfo>;

#[derive(Clone)]
struct RootSignatureLookup {
    root_signature: ID3D12RootSignature,
    /// lookup for all slots based on register, space and 
    slot_lookup: DesciptorSlotLookup,
    /// All descriptors (non push constant or samplers)
    descriptor_slots: Vec<u32>
}

/// A free list thread safe with mutex
struct FreeList {
    list: Mutex<Vec<usize>>
}

/// Thread safe ref counted free-list that can be safely used in drop traits 
type FreeListRef = std::sync::Arc<FreeList>;

impl FreeList {
    fn new() -> std::sync::Arc<FreeList> {
        std::sync::Arc::new(FreeList {
            list: Mutex::new(Vec::new())
        })
    }
}

/// Structure to track resources and resoure view allocations in `Drop` traits
struct DropResource {
    resources: Vec<ID3D12Resource>,
    frame: usize,
    heap_allocs: Vec<usize>
}

struct DropList {
    list: Mutex<Vec<DropResource>>
}

/// Thread safe ref counted drop-list that can be safely used in drop traits,
/// tracks the frame a resource was dropped on so it can be waited on
type DropListRef = std::sync::Arc<DropList>;

impl DropList {
    fn new() -> std::sync::Arc<DropList> {
        std::sync::Arc::new(DropList {
            list: Mutex::new(Vec::new())
        })
    }
}

#[derive(Clone)]
pub struct Heap {
    heap: ID3D12DescriptorHeap,
    base_address: usize,
    increment_size: usize,
    capacity: usize,
    offset: usize,
    free_list: FreeListRef,
    drop_list: DropListRef,
    id: u16
}

#[derive(Clone)]
pub struct QueryHeap {
    heap: ID3D12QueryHeap,
    alloc_index: usize,
    capacity: usize
}

#[derive(Clone)]
pub struct ComputePipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
    lookup: RootSignatureLookup,
}

#[derive(Clone)]
pub struct RaytracingPipeline {
    state_object: ID3D12StateObject,
    root_signature: ID3D12RootSignature,
    lookup: RootSignatureLookup
}

const fn to_dxgi_format(format: super::Format) -> DXGI_FORMAT {
    match format {
        super::Format::Unknown => DXGI_FORMAT_UNKNOWN,
        super::Format::R16n => DXGI_FORMAT_R16_UNORM,
        super::Format::R16u => DXGI_FORMAT_R16_UINT,
        super::Format::R16i => DXGI_FORMAT_R16_SINT,
        super::Format::R16f => DXGI_FORMAT_R16_FLOAT,
        super::Format::R32u => DXGI_FORMAT_R32_UINT,
        super::Format::R32i => DXGI_FORMAT_R32_SINT,
        super::Format::R32f => DXGI_FORMAT_R32_FLOAT,
        super::Format::RG16u => DXGI_FORMAT_R16G16_UINT,
        super::Format::RG16i => DXGI_FORMAT_R16G16_SINT,
        super::Format::RG16f => DXGI_FORMAT_R16G16_FLOAT,
        super::Format::RG32u => DXGI_FORMAT_R32G32_UINT,
        super::Format::RG32i => DXGI_FORMAT_R32G32_SINT,
        super::Format::RG32f => DXGI_FORMAT_R32G32_FLOAT,
        super::Format::RGB32u => DXGI_FORMAT_R32G32B32_UINT,
        super::Format::RGB32i => DXGI_FORMAT_R32G32B32_SINT,
        super::Format::RGB32f => DXGI_FORMAT_R32G32B32_FLOAT,
        super::Format::RGBA8nSRGB => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        super::Format::RGBA8n => DXGI_FORMAT_R8G8B8A8_UNORM,
        super::Format::RGBA8u => DXGI_FORMAT_R8G8B8A8_UINT,
        super::Format::RGBA8i => DXGI_FORMAT_R8G8B8A8_SINT,
        super::Format::BGRA8n => DXGI_FORMAT_B8G8R8A8_UNORM,
        super::Format::BGRX8n => DXGI_FORMAT_B8G8R8X8_UNORM,
        super::Format::BGRA8nSRGB => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        super::Format::BGRX8nSRGB => DXGI_FORMAT_B8G8R8X8_UNORM_SRGB,
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
        super::Format::BC1n => DXGI_FORMAT_BC1_UNORM,
        super::Format::BC1nSRGB => DXGI_FORMAT_BC1_UNORM_SRGB,
        super::Format::BC2n => DXGI_FORMAT_BC2_UNORM,
        super::Format::BC2nSRGB => DXGI_FORMAT_BC2_UNORM_SRGB,
        super::Format::BC3n => DXGI_FORMAT_BC3_UNORM,
        super::Format::BC3nSRGB => DXGI_FORMAT_BC3_UNORM_SRGB,
        super::Format::BC4n => DXGI_FORMAT_BC4_UNORM,
        super::Format::BC5n => DXGI_FORMAT_BC5_UNORM,
    }
}

const fn to_dxgi_format_srv(format: super::Format) -> DXGI_FORMAT {
    match format {
        super::Format::D32fS8X24u => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        super::Format::D32f => DXGI_FORMAT_R32_FLOAT,
        super::Format::D24nS8u => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
        super::Format::D16n => DXGI_FORMAT_R16_UNORM,
        _ => to_dxgi_format(format)
    }
}

const fn to_d3d12_compile_flags(flags: &super::ShaderCompileFlags) -> u32 {
    let mut d3d12_flags = 0;
    if flags.contains(super::ShaderCompileFlags::SKIP_OPTIMIZATION) {
        d3d12_flags |= D3DCOMPILE_SKIP_OPTIMIZATION;
    }
    if flags.contains(super::ShaderCompileFlags::DEBUG) {
        d3d12_flags |= D3DCOMPILE_DEBUG;
    }
    d3d12_flags
}

const fn to_d3d12_shader_visibility(visibility: &super::ShaderVisibility) -> D3D12_SHADER_VISIBILITY {
    match visibility {
        super::ShaderVisibility::All => D3D12_SHADER_VISIBILITY_ALL,
        super::ShaderVisibility::Vertex => D3D12_SHADER_VISIBILITY_VERTEX,
        super::ShaderVisibility::Fragment => D3D12_SHADER_VISIBILITY_PIXEL,
        super::ShaderVisibility::Compute => D3D12_SHADER_VISIBILITY_ALL,
    }
}

fn to_d3d12_sampler_boarder_colour(col: Option<u32>) -> D3D12_STATIC_BORDER_COLOR {
    let mut r = D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK;
    if let Some(col) = col {
        r = D3D12_STATIC_BORDER_COLOR(col as i32);
    }
    r
}

const fn to_d3d12_filter(filter: super::SamplerFilter, func: Option<super::ComparisonFunc>) -> D3D12_FILTER {
    if func.is_some() {
        match filter {
            super::SamplerFilter::Point => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_POINT,
            super::SamplerFilter::Linear => D3D12_FILTER_COMPARISON_MIN_MAG_MIP_LINEAR,
            super::SamplerFilter::Anisotropic => D3D12_FILTER_COMPARISON_ANISOTROPIC,
        }
    }
    else {
        match filter {
            super::SamplerFilter::Point => D3D12_FILTER_MIN_MAG_MIP_POINT,
            super::SamplerFilter::Linear => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
            super::SamplerFilter::Anisotropic => D3D12_FILTER_ANISOTROPIC,
        }

    }
}

const fn to_d3d12_address_mode(mode: super::SamplerAddressMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    match mode {
        super::SamplerAddressMode::Wrap => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        super::SamplerAddressMode::Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        super::SamplerAddressMode::Clamp => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        super::SamplerAddressMode::Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
        super::SamplerAddressMode::MirrorOnce => D3D12_TEXTURE_ADDRESS_MODE_MIRROR_ONCE,
    }
}

const fn to_d3d12_comparison_func(func: super::ComparisonFunc) -> D3D12_COMPARISON_FUNC {
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

const fn to_d3d12_address_comparison_func(func: Option<super::ComparisonFunc>) -> D3D12_COMPARISON_FUNC {
    if let Some(func) = func {
        to_d3d12_comparison_func(func)
    }
    else {
        D3D12_COMPARISON_FUNC_ALWAYS
    }
}

const fn to_d3d12_resource_state(state: super::ResourceState) -> D3D12_RESOURCE_STATES {
    match state {
        super::ResourceState::RenderTarget => D3D12_RESOURCE_STATE_RENDER_TARGET,
        super::ResourceState::Present => D3D12_RESOURCE_STATE_PRESENT,
        super::ResourceState::UnorderedAccess => D3D12_RESOURCE_STATE_UNORDERED_ACCESS,
        super::ResourceState::ShaderResource => D3D12_RESOURCE_STATE_ALL_SHADER_RESOURCE,
        super::ResourceState::VertexConstantBuffer => {
            D3D12_RESOURCE_STATE_VERTEX_AND_CONSTANT_BUFFER
        }
        super::ResourceState::IndexBuffer => D3D12_RESOURCE_STATE_INDEX_BUFFER,
        super::ResourceState::DepthStencil => D3D12_RESOURCE_STATE_DEPTH_WRITE,
        super::ResourceState::DepthStencilReadOnly => D3D12_RESOURCE_STATE_DEPTH_READ,
        super::ResourceState::ResolveSrc => D3D12_RESOURCE_STATE_RESOLVE_SOURCE,
        super::ResourceState::ResolveDst => D3D12_RESOURCE_STATE_RESOLVE_DEST,
        super::ResourceState::CopySrc => D3D12_RESOURCE_STATE_COPY_SOURCE,
        super::ResourceState::CopyDst => D3D12_RESOURCE_STATE_COPY_DEST,
        super::ResourceState::GenericRead => D3D12_RESOURCE_STATE_GENERIC_READ,
        super::ResourceState::IndirectArgument => D3D12_RESOURCE_STATE_INDIRECT_ARGUMENT,
        super::ResourceState::AccelerationStructure => D3D12_RESOURCE_STATE_RAYTRACING_ACCELERATION_STRUCTURE
    }
}

const fn to_d3d12_descriptor_heap_type(heap_type: super::HeapType) -> D3D12_DESCRIPTOR_HEAP_TYPE {
    match heap_type {
        super::HeapType::Shader => D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
        super::HeapType::RenderTarget => D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
        super::HeapType::DepthStencil => D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
        super::HeapType::Sampler => D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
    }
}

const fn to_d3d12_query_heap_type(heap_type: super::QueryType) -> D3D12_QUERY_HEAP_TYPE {
    match heap_type {
        super::QueryType::Occlusion => D3D12_QUERY_HEAP_TYPE_OCCLUSION,
        super::QueryType::BinaryOcclusion => D3D12_QUERY_HEAP_TYPE_OCCLUSION,
        super::QueryType::Timestamp => D3D12_QUERY_HEAP_TYPE_TIMESTAMP,
        super::QueryType::PipelineStatistics => D3D12_QUERY_HEAP_TYPE_PIPELINE_STATISTICS,
        super::QueryType::VideoDecodeStatistics => D3D12_QUERY_HEAP_TYPE_VIDEO_DECODE_STATISTICS,
    }
}

const fn to_d3d12_query_type(heap_type: super::QueryType) -> D3D12_QUERY_TYPE {
    match heap_type {
        super::QueryType::Occlusion => D3D12_QUERY_TYPE_OCCLUSION,
        super::QueryType::BinaryOcclusion => D3D12_QUERY_TYPE_BINARY_OCCLUSION,
        super::QueryType::Timestamp => D3D12_QUERY_TYPE_TIMESTAMP,
        super::QueryType::PipelineStatistics => D3D12_QUERY_TYPE_PIPELINE_STATISTICS,
        super::QueryType::VideoDecodeStatistics => D3D12_QUERY_TYPE_VIDEO_DECODE_STATISTICS,
    }
}

const fn to_d3d12_descriptor_heap_flags(heap_type: super::HeapType) -> D3D12_DESCRIPTOR_HEAP_FLAGS {
    match heap_type {
        super::HeapType::Shader => D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
        super::HeapType::RenderTarget => D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        super::HeapType::DepthStencil => D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
        super::HeapType::Sampler => D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
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
    if usage.contains(super::TextureUsage::VIDEO_DECODE_TARGET) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET | D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS;
    }
    flags
}

fn to_d3d12_buffer_usage_flags(usage: super::BufferUsage) -> D3D12_RESOURCE_FLAGS {
    let mut flags = D3D12_RESOURCE_FLAG_NONE;
    if usage.contains(super::BufferUsage::UNORDERED_ACCESS) {
        flags |= D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }
    flags
}

fn to_d3d12_texture_heap_flags(usage: super::TextureUsage) -> D3D12_HEAP_FLAGS {
    let mut flags = D3D12_HEAP_FLAG_NONE;
    if usage.contains(super::TextureUsage::VIDEO_DECODE_TARGET) {
        flags |= D3D12_HEAP_FLAG_SHARED;
    }
    flags
}

const fn to_d3d12_primitive_topology(
    topology: super::Topology,
    patch_index: u32,
) -> D3D_PRIMITIVE_TOPOLOGY {
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
            D3D_PRIMITIVE_TOPOLOGY_1_CONTROL_POINT_PATCHLIST.0 + patch_index as i32,
        ),
    }
}

const fn to_d3d12_primitive_topology_type(topology: super::Topology) -> D3D12_PRIMITIVE_TOPOLOGY_TYPE {
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
        super::Topology::PatchList => D3D12_PRIMITIVE_TOPOLOGY_TYPE_PATCH,
    }
}

const fn to_d3d12_fill_mode(fill_mode: &super::FillMode) -> D3D12_FILL_MODE {
    match fill_mode {
        super::FillMode::Wireframe => D3D12_FILL_MODE_WIREFRAME,
        super::FillMode::Solid => D3D12_FILL_MODE_SOLID,
    }
}

const fn to_d3d12_cull_mode(cull_mode: &super::CullMode) -> D3D12_CULL_MODE {
    match cull_mode {
        super::CullMode::None => D3D12_CULL_MODE_NONE,
        super::CullMode::Front => D3D12_CULL_MODE_FRONT,
        super::CullMode::Back => D3D12_CULL_MODE_BACK,
    }
}

const fn to_d3d12_write_mask(mask: &super::DepthWriteMask) -> D3D12_DEPTH_WRITE_MASK {
    match mask {
        super::DepthWriteMask::Zero => D3D12_DEPTH_WRITE_MASK_ZERO,
        super::DepthWriteMask::All => D3D12_DEPTH_WRITE_MASK_ALL,
    }
}

const fn to_d3d12_stencil_op(op: &super::StencilOp) -> D3D12_STENCIL_OP {
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

fn to_d3d12_render_target_blend(
    blend_info: &[super::RenderTargetBlendInfo],
) -> [D3D12_RENDER_TARGET_BLEND_DESC; 8] {
    let mut rtb: [D3D12_RENDER_TARGET_BLEND_DESC; 8] =
        [D3D12_RENDER_TARGET_BLEND_DESC::default(); 8];
    for (i, b) in blend_info.iter().enumerate() {
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
            RenderTargetWriteMask: u8::from(b.write_mask),
        };
    }
    rtb
}

const fn to_d3d12_blend_factor(factor: &super::BlendFactor) -> D3D12_BLEND {
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

const fn to_d3d12_blend_op(op: &super::BlendOp) -> D3D12_BLEND_OP {
    match op {
        super::BlendOp::Add => D3D12_BLEND_OP_ADD,
        super::BlendOp::Subtract => D3D12_BLEND_OP_SUBTRACT,
        super::BlendOp::RevSubtract => D3D12_BLEND_OP_REV_SUBTRACT,
        super::BlendOp::Min => D3D12_BLEND_OP_MIN,
        super::BlendOp::Max => D3D12_BLEND_OP_MAX,
    }
}

const fn to_d3d12_logic_op(op: &super::LogicOp) -> D3D12_LOGIC_OP {
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

const fn to_d3d12_texture_srv_dimension(tex_type: super::TextureType, samples: u32) -> D3D12_SRV_DIMENSION {
    if samples > 1 {
        match tex_type {
            super::TextureType::Texture2D => D3D12_SRV_DIMENSION_TEXTURE2DMS,
            super::TextureType::Texture2DArray => D3D12_SRV_DIMENSION_TEXTURE2DMSARRAY,
            _ => panic!()
        }
    }
    else {
        match tex_type {
            super::TextureType::Texture1D => D3D12_SRV_DIMENSION_TEXTURE1D,
            super::TextureType::Texture1DArray => D3D12_SRV_DIMENSION_TEXTURE1DARRAY,
            super::TextureType::Texture2D => D3D12_SRV_DIMENSION_TEXTURE2D,
            super::TextureType::Texture2DArray => D3D12_SRV_DIMENSION_TEXTURE2DARRAY,
            super::TextureType::TextureCube => D3D12_SRV_DIMENSION_TEXTURECUBE,
            super::TextureType::TextureCubeArray => D3D12_SRV_DIMENSION_TEXTURECUBEARRAY,
            super::TextureType::Texture3D => D3D12_SRV_DIMENSION_TEXTURE3D
        }
    }
}

const fn to_d3d12_resource_dimension(tex_type: super::TextureType) -> D3D12_RESOURCE_DIMENSION {
    match tex_type {
        super::TextureType::Texture1D => D3D12_RESOURCE_DIMENSION_TEXTURE1D,
        super::TextureType::Texture1DArray => D3D12_RESOURCE_DIMENSION_TEXTURE1D,
        super::TextureType::Texture2D => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        super::TextureType::Texture2DArray => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        super::TextureType::TextureCube => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        super::TextureType::TextureCubeArray => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        super::TextureType::Texture3D => D3D12_RESOURCE_DIMENSION_TEXTURE3D
    }
}

const fn to_d3d12_indirect_argument_type(indirect_type: IndirectArgumentType) -> D3D12_INDIRECT_ARGUMENT_TYPE {
    match indirect_type {
        IndirectArgumentType::Draw => D3D12_INDIRECT_ARGUMENT_TYPE_DRAW,
        IndirectArgumentType::DrawIndexed => D3D12_INDIRECT_ARGUMENT_TYPE_DRAW_INDEXED,
        IndirectArgumentType::Dispatch => D3D12_INDIRECT_ARGUMENT_TYPE_DISPATCH,
        IndirectArgumentType::PushConstants => D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT,
        IndirectArgumentType::ConstantBuffer => D3D12_INDIRECT_ARGUMENT_TYPE_CONSTANT_BUFFER_VIEW,
        IndirectArgumentType::VertexBuffer => D3D12_INDIRECT_ARGUMENT_TYPE_VERTEX_BUFFER_VIEW,
        IndirectArgumentType::IndexBuffer => D3D12_INDIRECT_ARGUMENT_TYPE_INDEX_BUFFER_VIEW,
        IndirectArgumentType::UnorderedAccess => D3D12_INDIRECT_ARGUMENT_TYPE_UNORDERED_ACCESS_VIEW,
        IndirectArgumentType::ShaderResource => D3D12_INDIRECT_ARGUMENT_TYPE_SHADER_RESOURCE_VIEW,
    }
}

const fn to_d3d12_hit_group_type(op: &super::RaytracingHitGeometry) -> D3D12_HIT_GROUP_TYPE {
    match op {
        super::RaytracingHitGeometry::Triangles => D3D12_HIT_GROUP_TYPE_TRIANGLES,
        super::RaytracingHitGeometry::ProceduralPrimitive => D3D12_HIT_GROUP_TYPE_PROCEDURAL_PRIMITIVE,
    }
}

fn to_d3d12_raytracing_geometry_flags(flags: super::RaytracingGeometryFlags) -> D3D12_RAYTRACING_GEOMETRY_FLAGS {
    let mut d3d12_flags : D3D12_RAYTRACING_GEOMETRY_FLAGS = D3D12_RAYTRACING_GEOMETRY_FLAGS(0);
    if flags.contains(RaytracingGeometryFlags::OPAQUE) {
        d3d12_flags |= D3D12_RAYTRACING_GEOMETRY_FLAG_OPAQUE;
    }

    if flags.contains(RaytracingGeometryFlags::NO_DUPLICATE_ANY_HIT) {
        d3d12_flags |= D3D12_RAYTRACING_GEOMETRY_FLAG_NO_DUPLICATE_ANYHIT_INVOCATION;
    }

    d3d12_flags
}

fn to_d3d12_raytracing_acceleration_structure_build_flags
    (flags: super::AccelerationStructureBuildFlags) -> D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAGS {
    let mut d3d12_flags = D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAGS(0);

    if flags.contains(AccelerationStructureBuildFlags::ALLOW_COMPACTION) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_COMPACTION;
    }

    if flags.contains(AccelerationStructureBuildFlags::ALLOW_UPDATE) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_ALLOW_UPDATE;
    }

    if flags.contains(AccelerationStructureBuildFlags::MINIMIZE_MEMORY) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_MINIMIZE_MEMORY;
    }

    if flags.contains(AccelerationStructureBuildFlags::PERFORM_UPDATE) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PERFORM_UPDATE;
    }

    if flags.contains(AccelerationStructureBuildFlags::PREFER_FAST_BUILD) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_BUILD;
    }

    if flags.contains(AccelerationStructureBuildFlags::PREFER_FAST_TRACE) {
        d3d12_flags |= D3D12_RAYTRACING_ACCELERATION_STRUCTURE_BUILD_FLAG_PREFER_FAST_TRACE;
    }

    d3d12_flags
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

fn get_binding_descriptor_hash(register: u32, space: u32, ty: super::DescriptorType) -> u64 {
    let mut hash = DefaultHasher::new();
    register.hash(&mut hash);
    space.hash(&mut hash);
    ty.hash(&mut hash);
    hash.finish()
}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    let trans = std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: unsafe { std::mem::transmute_copy(resource) },
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

#[repr(C)]
struct GenerateMipMapConstants {
    num_mips: u32,
    top_level_srv: u32,
    mip_level_uav: [u32; 16]
}

fn create_generate_mip_maps_pipeline(device: &Device) -> result::Result<ComputePipeline, super::Error> {
    let filepath = super::super::get_data_path("shaders/util/cs_mip_chain_texture2d.csc");
    let shader_data = std::fs::read(filepath)?;
    
    device.create_compute_pipeline(&ComputePipelineInfo{
        cs: &device.create_shader(&ShaderInfo{
                shader_type: super::ShaderType::Compute,
                compile_info: None
            }, 
            &shader_data
        )?,
        pipeline_layout: PipelineLayout { 
            push_constants: Some(vec![super::PushConstantInfo {
                visibility: super::ShaderVisibility::Compute,
                num_values: GenerateMipMapConstants::num_constants(),
                shader_register: 0,
                register_space: 0,
            }]),
            bindings: Some(vec![
                super::DescriptorBinding {
                    visibility: super::ShaderVisibility::Compute,
                    binding_type: super::DescriptorType::ShaderResource,
                    num_descriptors: None,
                    shader_register: 0,
                    register_space: 0,
                },
                super::DescriptorBinding {
                    visibility: super::ShaderVisibility::Compute,
                    binding_type: super::DescriptorType::UnorderedAccess,
                    num_descriptors: None,
                    shader_register: 0,
                    register_space: 0,
                }
            ]),
            static_samplers: None 
        }
    })
}

#[derive(Copy, Clone)]
enum Vendor
{
    Unknown,
    Intel,
    Amd,
    Nvidia
}

fn to_vendor(vendor_id: u32) -> Vendor {
    match vendor_id {
        0x163C | 0x8086 | 0x8087 => Vendor::Intel,
        0x1002 | 0x1022 => Vendor::Amd,
        0x10DE => Vendor::Nvidia,
        _ => Vendor::Unknown
    }
}

fn is_discrete_gpu(vendor: Vendor) -> bool {
    match vendor {
        Vendor::Unknown => false,
        Vendor::Intel => false,
        Vendor::Amd => true,
        Vendor::Nvidia => true
    }
}

pub fn get_hardware_adapter(
    factory: &IDXGIFactory4,
    adapter_name: &Option<String>,
) -> Result<(IDXGIAdapter1, super::AdapterInfo)> {
    unsafe {
        let mut adapter_info = super::AdapterInfo {
            name: String::from(""),
            description: String::from(""),
            dedicated_video_memory: 0,
            dedicated_system_memory: 0,
            shared_system_memory: 0,
            available: vec![],
        };

        // enumerate info
        let mut selected_index = -1;
        let mut selected_vendor = Vendor::Unknown;
        for i in 0.. {
            let adapter = factory.EnumAdapters1(i);
            if adapter.is_err() {
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

            if let Some(adapter_name) = &adapter_name {
                let s = adapter_name.to_string();
                if s == *decoded {
                    selected_index = i as i32;
                }
            } else {
                // auto select first non software adapter
                let adapter_flag = DXGI_ADAPTER_FLAG(desc.Flags as i32);
                if (adapter_flag & DXGI_ADAPTER_FLAG_SOFTWARE) == DXGI_ADAPTER_FLAG_NONE {
                    let adpater_vendor = to_vendor(desc.VendorId);

                    // select first
                    if selected_index == -1 {
                        selected_index = i as i32;
                        selected_vendor = adpater_vendor;
                    }

                    // override rules to select discrete gpu
                    if !is_discrete_gpu(selected_vendor) && is_discrete_gpu(adpater_vendor) {
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
            std::ptr::null_mut::<Option<D3D12DeviceVersion>>(),
        )
        .is_ok()
        {
            // fill adapter info out
            adapter_info.name = String::from("hotline_rs::d3d12::Device");
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
                None,
                &mut readback_buffer,
            )
            .expect("hotline_rs::gfx::d3d12: failed to create readback buffer");
    }
    readback_buffer
}

fn create_heap(device: &D3D12DeviceVersion, info: &HeapInfo, id: u16) -> Heap {
    unsafe {
        // reserve slot 0 as null so add 1 to num
        let num_descriptors = std::cmp::max(info.num_descriptors + 1, 1);
        let d3d12_type = to_d3d12_descriptor_heap_type(info.heap_type);
        let heap: ID3D12DescriptorHeap = device
            .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                Type: d3d12_type,
                NumDescriptors: num_descriptors as u32,
                Flags: to_d3d12_descriptor_heap_flags(info.heap_type),
                ..Default::default()
            })
            .expect("hotline_rs::gfx::d3d12: failed to create heap");
        let base_address = heap.GetCPUDescriptorHandleForHeapStart().ptr;
        let incr = device.GetDescriptorHandleIncrementSize(d3d12_type) as usize;

        Heap {
            heap,
            base_address,
            increment_size: incr,
            capacity: num_descriptors * incr,
            offset: incr, // resverve the first element as null
            free_list: FreeList::new(),
            drop_list: DropList::new(),
            id
        }
    }
}

fn create_query_heap(device: &D3D12DeviceVersion, info: &QueryHeapInfo) -> QueryHeap {    
    let mut heap = None;
    let desc = D3D12_QUERY_HEAP_DESC {
        Type: to_d3d12_query_heap_type(info.heap_type),
        Count: info.num_queries as u32,
        NodeMask: 0
    };
    unsafe { device.CreateQueryHeap(&desc, &mut heap).unwrap(); }
    QueryHeap {
        heap: heap.unwrap(),
        alloc_index: 0,
        capacity: info.num_queries
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
            device.device.CreateRenderTargetView(&render_target, None, h);
            textures.push(Texture {
                resource: Some(render_target.clone()),
                rtv: vec![TextureTarget{
                    ptr: h,
                    index: device.rtv_heap.get_handle_index(&h),
                    drop_list: device.rtv_heap.drop_list.clone()
                }],
                resolved_resource: None,
                resolved_format: DXGI_FORMAT_UNKNOWN,
                dsv: Vec::new(),
                srv_index: None,
                resolved_srv_index: None,
                uav_index: None,
                shared_handle: None,
                drop_list: None,
                subresource_uav_index: Vec::new(),
                shader_heap_id: None
            });
            d3d12_debug_name!(render_target, format!("swap_chain_texture"));
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
fn validate_data_size<T: Sized>(
    size_bytes: usize,
    data: Option<&[T]>,
) -> result::Result<(), super::Error> {
    if let Some(data) = data {
        let data_size_bytes = data.len() * std::mem::size_of::<T>();
        if data_size_bytes != size_bytes {
            return Err(super::Error {
                msg: format!(
                    "data size: ({}) bytes does not match expected size: ({}) bytes",
                    data_size_bytes, size_bytes
                ),
            });
        }
    }
    Ok(())
}

fn align_buffer_data_size(size_bytes: usize, usage: super::BufferUsage) -> usize {
    if usage.contains(super::BufferUsage::CONSTANT_BUFFER) {
        align_pow2(size_bytes as u64, D3D12_CONSTANT_BUFFER_DATA_PLACEMENT_ALIGNMENT as u64) as usize
    }
    else if usage.contains(super::BufferUsage::APPEND_COUNTER) {
        // must align the original buffer to D3D12_UAV_COUNTER_PLACEMENT_ALIGNMENT and then add a u32 counter
        align_pow2(size_bytes as u64, D3D12_UAV_COUNTER_PLACEMENT_ALIGNMENT as u64) as usize + std::mem::size_of::<u32>()
    }   
    else {
        size_bytes
    }
}

impl super::RenderPass<Device> for RenderPass {
    fn get_format_hash(&self) -> u64 {
        self.format_hash
    }
}

impl Heap {
    fn allocate(&mut self) -> D3D12_CPU_DESCRIPTOR_HANDLE {
        unsafe {
            let mut free_list = self.free_list.list.lock().unwrap();
            if free_list.is_empty() {
                // allocates a new handle
                if self.offset >= self.capacity {
                    println!("hotline_rs::gfx::d3d12: heap is full!");
                    panic!();
                }
                let ptr = self.heap.GetCPUDescriptorHandleForHeapStart().ptr + self.offset;
                self.offset += self.increment_size;
                D3D12_CPU_DESCRIPTOR_HANDLE { ptr }
            }
            else {
                 // pulls new handle from the free list
                let index = free_list.pop().unwrap();
                let ptr = self.base_address + self.increment_size * index;
                D3D12_CPU_DESCRIPTOR_HANDLE { ptr }
            }
        }
    }

    fn get_handle_index(&self, handle: &D3D12_CPU_DESCRIPTOR_HANDLE) -> usize {
        let ptr = handle.ptr - self.base_address;
        ptr / self.increment_size
    }
}

impl super::Heap<Device> for Heap {
    fn deallocate(&mut self, index: usize) {
        let mut free_list = self.free_list.list.lock().unwrap();
        free_list.push(index);
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &SwapChain) {
        let mut drop_list = self.drop_list.list.lock().unwrap();
        let mut free_list = self.free_list.list.lock().unwrap();
        let mut complete_indices = Vec::new();
        for (res_index, drop_res) in drop_list.iter_mut().enumerate() {
            // initialise the frame, and then wait
            if drop_res.frame == 0 {
                drop_res.frame = swap_chain.frame_index;
            }
            else {
                let diff = swap_chain.frame_index - drop_res.frame;
                if diff > swap_chain.num_bb as usize {
                    // waited long enough we can add the resource views to the free list
                    for alloc in &drop_res.heap_allocs {
                        free_list.push(*alloc);
                    }
                    drop_res.resources.clear();
                    drop_res.heap_allocs.clear();
                    complete_indices.push(res_index);
                }
            }
        }

        // remove complete items in reverse
        complete_indices.reverse();
        for i in complete_indices {
            drop_list.remove(i);
        }
    }

    fn get_heap_id(&self) -> u16 {
        self.id
    }
}

impl QueryHeap {
    fn allocate(&mut self) -> usize {
        let alloc = self.alloc_index;
        assert!(self.alloc_index < self.capacity, 
            "hotline::gfx::d3d12:: overflowed query heap with {} queries", self.alloc_index);
        self.alloc_index += 1;
        alloc
    }
}

impl super::QueryHeap<Device> for QueryHeap {
    fn reset(&mut self) {
        self.alloc_index = 0;
    }
}

impl Buffer {
    /// Get the d3d virtual address as u64 or 0 if the resource is None
    fn d3d_virtual_address(&self) -> u64 {
        unsafe {
            self.resource.as_ref().map(|x| x.GetGPUVirtualAddress()).unwrap_or(0)
        }
    }
    /// Get the d3d virtual address as u64 or 0 if the resource is None
    fn size_bytes(&self) -> u64 {
        unsafe {
            self.resource.as_ref().map(|x| x.GetDesc().Width).unwrap_or(0)
        }
    }
}

impl Device {
    fn create_d3d12_input_element_desc(
        layout: &super::InputLayout,
        null_terminated_semantics: &[CString],
    ) -> Vec<D3D12_INPUT_ELEMENT_DESC> {
        let mut d3d12_elems: Vec<D3D12_INPUT_ELEMENT_DESC> = Vec::new();
        for (ielem, elem) in layout.iter().enumerate() {
            d3d12_elems.push(D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(null_terminated_semantics[ielem].as_ptr() as _),
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
        }
        d3d12_elems
    }

    fn create_root_signature_with_lookup(
        &self,
        layout: &super::PipelineLayout,
    ) -> result::Result<RootSignatureLookup, super::Error> {
        let mut root_params: Vec<D3D12_ROOT_PARAMETER> = Vec::new();

        let mut slot_iter = 0;
        let mut lookup = HashMap::new();

        // push constants
        if let Some(constants_set) = &layout.push_constants {
            for constants in constants_set {
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

                // track the slots push constants occupy
                let h = get_binding_descriptor_hash(constants.shader_register, constants.register_space, super::DescriptorType::PushConstants);
                lookup.insert(h, PipelineSlotInfo {
                    index: slot_iter,
                    count: Some(constants.num_values)
                });
                slot_iter += 1;
            }
        }

        // bindings for (SRV, UAV, CBV an Samplers)
        #[derive(Default)]
        struct RangeInfo {
            ranges: Vec<D3D12_DESCRIPTOR_RANGE>,
            info: Vec<DescriptorBinding>,
            visibility: super::ShaderVisibility
        }

        let mut visibility_map: 
            HashMap<u64, RangeInfo> = HashMap::new();

        let mut descriptor_root_params = Vec::new();
        if let Some(bindings) = &layout.bindings {
            for binding in bindings {
                let count = if binding.num_descriptors.is_some() {
                    binding.num_descriptors.unwrap()
                } 
                else {
                    u32::MAX
                };
                let range = D3D12_DESCRIPTOR_RANGE {
                    RangeType: match binding.binding_type {
                        super::DescriptorType::ShaderResource => D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                        super::DescriptorType::UnorderedAccess => D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
                        super::DescriptorType::ConstantBuffer => D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
                        super::DescriptorType::Sampler => D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
                        super::DescriptorType::PushConstants => panic!("hotline_rs::d3d12:: cannot use push constants as a descriptor binding type")
                    },
                    NumDescriptors: count,
                    BaseShaderRegister: binding.shader_register,
                    RegisterSpace: binding.register_space,
                    OffsetInDescriptorsFromTableStart: 0,
                };

                let mut h = DefaultHasher::new();
                binding.visibility.hash(&mut h);
                binding.shader_register.hash(&mut h);
                binding.binding_type.hash(&mut h);

                let entry = visibility_map.entry(h.finish()).or_default();
                entry.ranges.push(range);
                entry.info.push(binding.clone());
            }

            for range_info in visibility_map.values() {
                root_params.push(D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: range_info.ranges.len() as u32,
                            pDescriptorRanges: range_info.ranges.as_ptr() as *mut D3D12_DESCRIPTOR_RANGE,
                        },
                    },
                    ShaderVisibility: to_d3d12_shader_visibility(&range_info.visibility),
                });
                descriptor_root_params.push(slot_iter);

                for binding in &range_info.info {
                    let h = get_binding_descriptor_hash(binding.shader_register, binding.register_space, binding.binding_type);
                    lookup.entry(h).or_insert(PipelineSlotInfo {
                        index: slot_iter,
                        count: binding.num_descriptors
                    });
                }
                slot_iter += 1;
            }
        }

        // immutable samplers
        let mut static_samplers: Vec<D3D12_STATIC_SAMPLER_DESC> = Vec::new();
        if let Some(samplers) = &layout.static_samplers {
            for sampler in samplers {
                static_samplers.push(D3D12_STATIC_SAMPLER_DESC {
                    Filter: to_d3d12_filter(sampler.sampler_info.filter, sampler.sampler_info.comparison),
                    AddressU: to_d3d12_address_mode(sampler.sampler_info.address_u),
                    AddressV: to_d3d12_address_mode(sampler.sampler_info.address_v),
                    AddressW: to_d3d12_address_mode(sampler.sampler_info.address_w),
                    MipLODBias: sampler.sampler_info.mip_lod_bias,
                    MaxAnisotropy: sampler.sampler_info.max_aniso,
                    ComparisonFunc: to_d3d12_address_comparison_func(sampler.sampler_info.comparison),
                    BorderColor: to_d3d12_sampler_boarder_colour(sampler.sampler_info.border_colour),
                    MinLOD: sampler.sampler_info.min_lod,
                    MaxLOD: sampler.sampler_info.max_lod,
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
        };

        unsafe {
            // serialise signature
            let mut signature = None;
            let mut error = None;
            let _ = D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature,
                Some(&mut error),
            );

            // handle errors
            if let Some(blob) = error {
                return Err(super::Error {
                    msg: get_d3d12_error_blob_string(&blob),
                });
            }

            // create signature
            let sig = signature.unwrap();
            let slice : &[u8] = std::slice::from_raw_parts(sig.GetBufferPointer() as *mut u8, sig.GetBufferSize());
            let sig = self.device.CreateRootSignature(0, slice)?;
            
            Ok(RootSignatureLookup {
                root_signature: sig,
                slot_lookup: lookup,
                descriptor_slots: descriptor_root_params
            })
        }
    }

    fn create_render_passes_for_swap_chain(
        &self,
        num_buffers: u32,
        textures: &[Texture],
        clear_col: Option<ClearColour>,
    ) -> Vec<RenderPass> {
        let mut passes = Vec::new();
        for texture in textures.iter().take(num_buffers as usize) {
            passes.push(
                self.create_render_pass(&super::RenderPassInfo {
                    render_targets: vec![texture],
                    rt_clear: clear_col,
                    depth_stencil: None,
                    ds_clear: None,
                    resolve: false,
                    discard: false,
                    array_slice: 0
                })
                .unwrap(),
            );
        }
        passes
    }

    #[allow(clippy::too_many_arguments)] 
    fn upload_texture_data_subresource(
        &mut self, 
        width: u64,
        height: u64,
        depth: u32,
        subresource: u32,
        data: *const u8,
        format: super::Format,
        dxgi_format: DXGI_FORMAT,
        resource: &ID3D12Resource,
        upload_resources: &mut Vec<ID3D12Resource>) -> std::result::Result<(), super::Error> {
        
        let tpb = super::texels_per_block_for_format(format);
        let align_width = width.max(tpb);
        let align_height = height.max(tpb);

        // create upload buffer
        let row_pitch = super::row_pitch_for_format(format, width);
        let upload_pitch = super::align_pow2(row_pitch, D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u64);
        let upload_size = align_height * upload_pitch * depth as u64;
        let src_size = super::size_for_format(format, width, height, depth);
        
        unsafe {
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
                None,
                &mut upload,
            )?;
            let back = upload_resources.len();
            upload_resources.push(upload.unwrap());
    
            // copy data to upload buffer
            let mut map_data = std::ptr::null_mut();
            upload_resources[back].Map(0, None, Some(&mut map_data))?;
            
            if row_pitch == upload_pitch {
                // copy the entire mip level in 1 go
                let src = data;
                let dst = map_data as *mut u8;
                std::ptr::copy_nonoverlapping(src, dst, src_size as usize);
            }
            else {
                // copy with the upload pitch padded
                if !map_data.is_null() {
                    for y in 0..height {
                        let src = data.offset((y * row_pitch) as isize) as *const u8;
                        let dst = (map_data as *mut u8).offset((y * upload_pitch) as isize);
                        std::ptr::copy_nonoverlapping(src, dst, row_pitch as usize);
                    }
                }
            }

            upload_resources[back].Unmap(0, None);
    
            // copy resource
            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&upload_resources[back]),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Width: align_width as u32,
                            Height: align_height as u32,
                            Depth: depth,
                            Format: dxgi_format,
                            RowPitch: upload_pitch as u32,
                        },
                    },
                },
            };
    
            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(resource),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: subresource,
                },
            };
    
            self.command_list.CopyTextureRegion(&dst, 0, 0, 0, &src, None);
        }
        Ok(())
    }

    fn upload_texture_data<T>(
        &mut self, 
        info: &TextureInfo, 
        data: &[T], 
        dxgi_format: DXGI_FORMAT,
        resource: &ID3D12Resource, 
        initial_state: D3D12_RESOURCE_STATES) -> std::result::Result<(), super::Error> {

        let mut data_itr = data.as_ptr() as *const u8;
        let mut upload_resources = Vec::new(); 

        for layer in 0..info.array_layers {
            let mut mip_width = info.width;
            let mut mip_height = info.height;
            let mut mip_depth = info.depth;
            for mip in 0..info.mip_levels {
                self.upload_texture_data_subresource(
                    mip_width,
                    mip_height,
                    mip_depth,
                    layer * info.mip_levels + mip,
                    data_itr,
                    info.format,
                    dxgi_format,
                    resource,
                    &mut upload_resources
                )?;
                let slice_pitch = slice_pitch_for_format(info.format, mip_width, mip_height);
                data_itr = unsafe { data_itr.add(slice_pitch as usize) };
                mip_width = max(mip_width / 2, 1);
                mip_height = max(mip_height / 2, 1);
                mip_depth = max(mip_depth / 2, 1);
            }
        }

        unsafe {
            // transition to requested initial state
            let barrier = transition_barrier(
                resource,
                D3D12_RESOURCE_STATE_COPY_DEST,
                initial_state,
            );
    
            self.command_list.ResourceBarrier(&[barrier.clone()]);
            let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);

            self.command_list.Close()?;
    
            let cmd = Some(self.command_list.cast().unwrap());
            self.command_queue.ExecuteCommandLists(&[cmd]);

            let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            self.command_queue.Signal(&fence, 1)?;
    
            let event = CreateEventA(None, false, false, None)?;
            fence.SetEventOnCompletion(1, event)?;
            WaitForSingleObject(event, INFINITE);
            self.command_list.Reset(&self.command_allocator, None)?;
        }

        Ok(())
    }

    fn execute_and_wait_on_resource_cmd(&self) -> result::Result<(), super::Error> {
        unsafe {
            // execute commandlist
            let cmd = Some(self.command_list.cast().unwrap());
            self.command_queue.ExecuteCommandLists(&[cmd]);

            // wait
            let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            self.command_queue.Signal(&fence, 1)?;
            let event = CreateEventA(None, false, false, None)?;
            fence.SetEventOnCompletion(1, event)?;
            WaitForSingleObject(event, INFINITE);

            // reset command list
            self.command_list.Reset(&self.command_allocator, None)?;
        }

        Ok(())
    }
}

// public accessor for device
pub fn get_dxgi_factory(device: &Device) -> &IDXGIFactory4 {
    &device.dxgi_factory
}

impl Shader {
    fn get_buffer_pointer(&self) -> *const std::ffi::c_void {
        if let Some(blob) = &self.blob {
            unsafe { blob.GetBufferPointer() }
        }
        else if let Some(precompiled) = &self.precompiled {
            precompiled.as_ptr() as _
        }
        else {
            std::ptr::null()
        }
    }

    fn get_buffer_size(&self) -> usize {
        if let Some(blob) = &self.blob {
            unsafe { blob.GetBufferSize() }
        }
        else if let Some(precompiled) = &self.precompiled {
            precompiled.len()
        }
        else {
            0
        }
    }
}

pub(crate) struct ShaderTable {
    pub buffer: Option<ID3D12Resource>,
    pub count: usize,
    pub stride: usize
}

pub struct RaytracingShaderBindingTable {
    pub(crate) ray_generation: ShaderTable,
    pub(crate) miss: ShaderTable,
    pub(crate) hit_group: ShaderTable,
    pub(crate) callable: ShaderTable
}

pub struct RaytracingBLAS {
    pub(crate) blas_buffer: Buffer
}

pub struct RaytracingTLAS {
    pub(crate) tlas_buffer: Buffer
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
    type ComputePipeline = ComputePipeline;
    type RaytracingPipeline = RaytracingPipeline;
    type Heap = Heap;
    type QueryHeap = QueryHeap;
    type CommandSignature = CommandSignature;
    type RaytracingShaderBindingTable = RaytracingShaderBindingTable;
    type RaytracingBLAS = RaytracingBLAS;
    type RaytracingTLAS = RaytracingTLAS;
    fn create(info: &super::DeviceInfo) -> Device {
        unsafe {
            // enable debug layer
            let mut dxgi_factory_flags: u32 = 0;
            if cfg!(debug_assertions) {
                let mut debug: Option<D3D12DebugVersion> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and(debug) {
                    debug.EnableDebugLayer();

                    // slower but more detailed GPU validation
                    if GPU_VALIDATION {
                        let debug1 : ID3D12Debug1 = debug.cast().unwrap();
                        debug1.SetEnableGPUBasedValidation(true);
                    }

                    println!("hotline_rs::gfx::d3d12: enabling debug layer");
                }
                dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG.0;
            }

            // create dxgi factory
            let dxgi_factory = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(dxgi_factory_flags))
                .expect("hotline_rs::gfx::d3d12: failed to create dxgi factory");

            // create adapter
            let (adapter, adapter_info) = get_hardware_adapter(&dxgi_factory, &info.adapter_name)
                .expect("hotline_rs::gfx::d3d12: failed to get hardware adapter");

            // create device
            let mut d3d12_device: Option<D3D12DeviceVersion> = None;
            D3D12CreateDevice(&adapter, D3D_FEATURE_LEVEL_12_1, &mut d3d12_device)
                .expect("hotline_rs::gfx::d3d12: failed to create d3d12 device");
            let device = d3d12_device.unwrap();

            // create command allocator
            let command_allocator = device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                .expect("hotline_rs::gfx::d3d12: failed to create command allocator");

            // create command list
            let command_list = device
                .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &command_allocator, None)
                .expect("hotline_rs::gfx::d3d12: failed to create command list");

            // create queue
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                NodeMask: 1,
                ..Default::default()
            };
            let command_queue : ID3D12CommandQueue = device
                .CreateCommandQueue(&desc)
                .expect("hotline_rs::gfx::d3d12: failed to create command queue");

            let timestamp_frequency = command_queue
                .GetTimestampFrequency()
                .expect("hotline_rs::gfx::d3d12: failed to obtain timestamp frquency") as f64;

            // default heaps
            // shader (srv, cbv, uav)
            let mut heap_id = 1;
            let shader_heap = create_heap(
                &device,
                &HeapInfo {
                    heap_type: super::HeapType::Shader,
                    num_descriptors: info.shader_heap_size,
                },
                heap_id
            );
            d3d12_debug_name!(shader_heap.heap, "device_shader_heap");

            // rtv
            heap_id += 1;
            let rtv_heap = create_heap(
                &device,
                &HeapInfo {
                    heap_type: super::HeapType::RenderTarget,
                    num_descriptors: info.render_target_heap_size,
                },
                heap_id
            );
            d3d12_debug_name!(rtv_heap.heap, "device_render_target_heap");
            
            // dsv
            heap_id += 1;
            let dsv_heap = create_heap(
                &device,
                &HeapInfo {
                    heap_type: super::HeapType::DepthStencil,
                    num_descriptors: info.depth_stencil_heap_size,
                },
                heap_id
            );
            d3d12_debug_name!(dsv_heap.heap, "device_depth_stencil_heap");

            // query feature flags
            let mut feature_flags = DeviceFeatureFlags::NONE;

            // ray tracing
            let options5 = D3D12_FEATURE_DATA_D3D12_OPTIONS5::default();
            if device.CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS5, 
                std::ptr::addr_of!(options5) as *mut _, 
                std::mem::size_of::<D3D12_FEATURE_DATA_D3D12_OPTIONS5>() as u32).is_ok() {
                if options5.RaytracingTier != D3D12_RAYTRACING_TIER_NOT_SUPPORTED {
                    feature_flags |= super::DeviceFeatureFlags::RAYTRACING;
                }
            }

            // mesh shader
            let options7 = D3D12_FEATURE_DATA_D3D12_OPTIONS7::default();
            if device.CheckFeatureSupport(
                D3D12_FEATURE_D3D12_OPTIONS7, 
                std::ptr::addr_of!(options7) as *mut _, 
                std::mem::size_of::<D3D12_FEATURE_DATA_D3D12_OPTIONS7>() as u32).is_ok() {
                if options7.MeshShaderTier != D3D12_MESH_SHADER_TIER_NOT_SUPPORTED {
                    feature_flags |= super::DeviceFeatureFlags::MESH_SAHDER;
                }
            }

            // initialise struct
            let mut device = Device {
                timestamp_frequency,
                adapter_info,
                feature_flags,
                device,
                dxgi_factory,
                command_allocator,
                command_list,
                command_queue,
                pix: WinPixEventRuntime::create(),
                shader_heap: Some(shader_heap),
                rtv_heap,
                dsv_heap,
                generate_mip_maps_pipeline: None,
                heap_id
            };

            // pipeline for command buffers to generate mips.. if it exists
            if let Ok(pipeline) = create_generate_mip_maps_pipeline(&device) {
                device.generate_mip_maps_pipeline = Some(pipeline);
            }
            
            device
        }
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Heap {
        self.heap_id += 1; // bump heap id
        create_heap(&self.device, info, self.heap_id)
    }

    fn create_query_heap(&self, info: &QueryHeapInfo) -> QueryHeap {
        create_query_heap(&self.device, info)
    }

    fn create_swap_chain<A: os::App>(
        &mut self,
        info: &super::SwapChainInfo,
        win: &A::Window,
    ) -> result::Result<SwapChain, super::Error> {
        unsafe {
            // set flags, these could be passed in
            let flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0;
            let format = info.format;
            let dxgi_format = to_dxgi_format(format);

            // create swap chain desc
            let size = win.get_size();
            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                BufferCount: info.num_buffers,
                Width: size.x as u32,
                Height: size.y as u32,
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

            let hwnd : HWND = std::mem::transmute(win.get_native_handle().get_isize());

            // create swap chain itself
            let swap_chain1 = self
                .dxgi_factory
                .CreateSwapChainForHwnd(
                    &self.command_queue,
                    hwnd,
                    &swap_chain_desc,
                    None,
                    None,
                )?;
            let swap_chain: IDXGISwapChain3 = swap_chain1.cast()?;

            // create rtv heap and handles
            let textures = create_swap_chain_rtv(&swap_chain, self, info.num_buffers);

            let data_size = size_for_format(format, size.x as u64, size.y as u64, 1);
            let passes = self.create_render_passes_for_swap_chain(
                info.num_buffers,
                &textures,
                info.clear_colour,
            );

            let passes_no_clear = self.create_render_passes_for_swap_chain(
                info.num_buffers,
                &textures,
                None,
            );

            Ok(SwapChain {
                width: size.x,
                height: size.y,
                format,
                num_bb: info.num_buffers,
                flags: flags as u32,
                bb_index: 0,
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?,
                fence_last_signalled_value: 0,
                fence_event: CreateEventA(None, false, false, None)?,
                swap_chain,
                backbuffer_textures: textures,
                backbuffer_passes: passes,
                backbuffer_passes_no_clear: passes_no_clear,
                frame_index: 0,
                frame_fence_value: vec![0; info.num_buffers as usize],
                readback_buffer: create_read_back_buffer(self, data_size),
                require_wait: vec![false; info.num_buffers as usize],
                clear_col: info.clear_colour,
            })
        }
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> CmdBuf {
        unsafe {
            let mut command_allocators: Vec<ID3D12CommandAllocator> = Vec::new();
            let mut command_lists: Vec<ID3D12GraphicsCommandList> = Vec::new();
            let mut barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>> = Vec::new();
            let mut needs_reset = Vec::new();

            for _ in 0..num_buffers as usize {
                // create command allocator
                let command_allocator = self
                    .device
                    .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                    .expect("hotline_rs::gfx::d3d12: failed to create command allocator");

                // create command list
                let command_list = self
                    .device
                    .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &command_allocator, None)
                    .expect("hotline_rs::gfx::d3d12: failed to create command list");

                command_allocators.push(command_allocator);
                command_lists.push(command_list);

                barriers.push(Vec::new());
                needs_reset.push(false);
            }

            CmdBuf {
                bb_index: 0,
                command_allocator: command_allocators,
                command_list: command_lists,
                pix: self.pix,
                in_flight_barriers: barriers,
                event_stack_count: 0,
                needs_reset
            }
        }
    }

    fn create_render_pipeline(
        &self,
        info: &super::RenderPipelineInfo<Device>,
    ) -> result::Result<RenderPipeline, super::Error> {
        let sig_lookup = self.create_root_signature_with_lookup(&info.pipeline_layout)?;

        let semantics = null_terminate_semantics(&info.input_layout);
        let mut elems = Device::create_d3d12_input_element_desc(&info.input_layout, &semantics);
        let input_layout = D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: elems.as_mut_ptr(),
            NumElements: elems.len() as u32,
        };

        let raster = &info.raster_info;
        let depth_stencil = &info.depth_stencil_info;
        let blend = &info.blend_info;

        let null_bytecode = D3D12_SHADER_BYTECODE {
            pShaderBytecode: std::ptr::null_mut(),
            BytecodeLength: 0,
        };

        // unwrap pass
        let pass = info.pass.expect("hotline::gfx::d3d12:: a pass is required when creating a render pipline");
        let msaa_format = pass.sample_count > 1;

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            InputLayout: input_layout,
            pRootSignature: unsafe { std::mem::transmute_copy(&sig_lookup.root_signature) },
            VS: if let Some(vs) = &info.vs {
                D3D12_SHADER_BYTECODE {
                    pShaderBytecode: vs.get_buffer_pointer(),
                    BytecodeLength: vs.get_buffer_size(),
                }
            } else {
                null_bytecode
            },
            PS: if let Some(ps) = &info.fs {
                D3D12_SHADER_BYTECODE {
                    pShaderBytecode: ps.get_buffer_pointer(),
                    BytecodeLength: ps.get_buffer_size(),
                }
            } else {
                null_bytecode
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: to_d3d12_fill_mode(&raster.fill_mode),
                CullMode: to_d3d12_cull_mode(&raster.cull_mode),
                FrontCounterClockwise: BOOL::from(raster.front_ccw),
                DepthBias: raster.depth_bias,
                DepthBiasClamp: raster.depth_bias_clamp,
                SlopeScaledDepthBias: raster.slope_scaled_depth_bias,
                DepthClipEnable: BOOL::from(raster.front_ccw),
                MultisampleEnable: BOOL::from(msaa_format),
                AntialiasedLineEnable: BOOL::from(msaa_format),
                ForcedSampleCount: raster.forced_sample_count,
                ConservativeRaster: if raster.conservative_raster_mode {
                    D3D12_CONSERVATIVE_RASTERIZATION_MODE_ON
                } else {
                    D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF
                },
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: BOOL::from(blend.alpha_to_coverage_enabled),
                IndependentBlendEnable: BOOL::from(blend.independent_blend_enabled),
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
            SampleMask: u32::max_value(), // TODO: supply sample mask
            PrimitiveTopologyType: to_d3d12_primitive_topology_type(info.topology),
            NumRenderTargets: pass.rt_formats.len() as u32,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: pass.sample_count,
                Quality: 0,
            },
            ..Default::default()
        };

        // Set formats from pass
        for i in 0..pass.rt_formats.len() {
            desc.RTVFormats[i] = pass.rt_formats[i];
        }
        desc.DSVFormat = pass.ds_format;

        Ok(RenderPipeline {
            pso: unsafe { self.device.CreateGraphicsPipelineState(&desc)? },
            root_signature: sig_lookup.root_signature.clone(),
            topology: to_d3d12_primitive_topology(info.topology, info.patch_index),
            lookup: sig_lookup
        })
    }

    fn create_shader<T: Sized>(
        &self,
        info: &super::ShaderInfo,
        src: &[T],
    ) -> std::result::Result<Shader, super::Error> {
        // compile source
        let mut shader_blob = None;
        if let Some(compile_info) = &info.compile_info {
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
                    PCSTR(std::ptr::null_mut() as _),
                    None,
                    None,
                    PCSTR(nullt_entry_point.as_ptr() as _),
                    PCSTR(nullt_target.as_ptr() as _),
                    compile_flags,
                    0,
                    &mut shader_blob,
                    Some(&mut errors),
                );
                if result.is_err() {
                    if let Some(e) = errors {
                        let buf = e.GetBufferPointer();
                        let c_str: &CStr = CStr::from_ptr(buf as *const i8);
                        let str_slice: &str = c_str.to_str().unwrap();
                        return Err(super::Error {
                            msg: String::from(str_slice),
                        });
                    }
                    panic!("hotline_rs::gfx::d3d12: shader compile failed with no error information!");
                }
            }

            return Ok(Shader {
                blob: Some(shader_blob.unwrap()),
                precompiled: None
            });
        }

        // copy byte code
        // we need at least 4 bytes to check the fourcc code
        if src.len() > 4 {
            // copies precompiled shader to be re-used in pipelines etc
            let mut bytes: Vec<u8> = vec![0; src.len()];
            unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr() as *mut u8, bytes.as_mut_ptr(), src.len());
            }

            // validate DXBC 
            let mut valid = true;
            let validate = [b'D', b'X', b'B', b'C'];
            for i in 0..4 {
                if bytes[i] != validate[i] {
                    valid = false;
                    break;
                }
            }

            if valid {
                return Ok(Shader {
                    blob: None,
                    precompiled: Some(bytes)
                });
            }
        }

        // invalid dxil/dxbc shader bytecode
        Err( super::Error {
            msg: String::from("hotline_rs::gfx::d3d12: shader byte code (src) is not valid"),
        })
    }

    fn create_buffer_with_heap<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
        heap: &mut Heap
    ) -> result::Result<Buffer, super::Error> {
        let mut buf: Option<ID3D12Resource> = None;
        let dxgi_format = to_dxgi_format(info.format);
        let size_bytes = info.stride * info.num_elements;
        validate_data_size(size_bytes, data)?;

        let aligned_size = align_buffer_data_size(size_bytes, info.usage);

        unsafe {
            // create upload buffer
            let upload = if let Some(data) = &data {
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
                        Width: aligned_size as u64,
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
                    None,
                    &mut upload,
                )?;
                let upload = upload.unwrap();

                // copy data to upload buffer
                let mut map_data = std::ptr::null_mut();
                let res = upload.clone();
                res.Map(0, None, Some(&mut map_data))?;
                if !map_data.is_null() {
                    let src = data.as_ptr() as *mut u8;
                    std::ptr::copy_nonoverlapping(src, map_data as *mut u8, size_bytes);
                }
                res.Unmap(0, None);

                // return the uplaod buffer
                Some(upload)
            }
            else {
                None
            };

            // acceleration structure just needs an upload buffer. TODO: separate to function
            if info.usage == super::BufferUsage::UPLOAD {
                return Ok(Buffer {
                    resource: upload,
                    vbv: None,
                    ibv: None,
                    srv_index: None,
                    cbv_index: None,
                    counter_offset: None,
                    uav_index: None,
                    drop_list: Some(heap.drop_list.clone()),
                    persistent_mapped_data: std::ptr::null_mut()
                });
            }

            // create a buffer resource
            self.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: if info.cpu_access.contains(super::CpuAccessFlags::WRITE) {
                        D3D12_HEAP_TYPE_UPLOAD
                    } else {
                        D3D12_HEAP_TYPE_DEFAULT
                    },
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: aligned_size as u64,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    Flags: to_d3d12_buffer_usage_flags(info.usage),
                    ..Default::default()
                },
                // initial state
                if info.cpu_access.contains(super::CpuAccessFlags::WRITE) {
                    D3D12_RESOURCE_STATE_GENERIC_READ
                } 
                else if data.is_some() {
                    D3D12_RESOURCE_STATE_COPY_DEST
                }
                else {
                    to_d3d12_resource_state(info.initial_state)
                },
                None,
                &mut buf,
            )?;
            let buf = buf.unwrap();

            // load buffer with initialised data
            if data.is_some() {
                let upload = upload.unwrap();
                // copy resource from upload buffer
                let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();
                self.command_list.CopyResource(&buf, &upload);
                let barrier = transition_barrier(
                    &buf,
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    to_d3d12_resource_state(info.initial_state),
                );

                // transition to shader resource
                self.command_list.ResourceBarrier(&[barrier.clone()]);
                self.command_list.Close()?;

                let cmd = Some(self.command_list.cast().unwrap());
                self.command_queue.ExecuteCommandLists(&[cmd]);
                self.command_queue.Signal(&fence, 1)?;

                let event = CreateEventA(None, false, false, None)?;
                fence.SetEventOnCompletion(1, event)?;
                WaitForSingleObject(event, INFINITE);

                self.command_list.Reset(&self.command_allocator, None)?;
                let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                    std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
            }

            // map data
            let mut map_data = std::ptr::null_mut();
            if info.cpu_access.contains(super::CpuAccessFlags::PERSISTENTLY_MAPPED) {
                buf.Map(0, None, Some(&mut map_data))?;
            }

            // append counter
            let counter_offset = if info.usage.contains(super::BufferUsage::APPEND_COUNTER) {
                Some(aligned_size - std::mem::size_of::<u32>())
            }
            else {
                None
            };

            // create optional views
            let mut vbv: Option<D3D12_VERTEX_BUFFER_VIEW> = None;
            let mut ibv: Option<D3D12_INDEX_BUFFER_VIEW> = None;
            let mut uav_index = None;
            let mut cbv_index = None;
            let mut srv_index = None;

            if !info.usage.contains(super::BufferUsage::BUFFER_ONLY) {
                if info.usage.contains(super::BufferUsage::VERTEX) {
                    vbv = Some(D3D12_VERTEX_BUFFER_VIEW {
                        BufferLocation: buf.GetGPUVirtualAddress(),
                        StrideInBytes: info.stride as u32,
                        SizeInBytes: size_bytes as u32,
                    });
                }

                if info.usage.contains(super::BufferUsage::INDEX) {
                    ibv = Some(D3D12_INDEX_BUFFER_VIEW {
                        BufferLocation: buf.GetGPUVirtualAddress(),
                        SizeInBytes: size_bytes as u32,
                        Format: dxgi_format,
                    })
                }

                if info.usage.contains(super::BufferUsage::CONSTANT_BUFFER) {
                    let h = heap.allocate();
                    self.device.CreateConstantBufferView(
                        Some(&D3D12_CONSTANT_BUFFER_VIEW_DESC {
                            BufferLocation: buf.GetGPUVirtualAddress(),
                            SizeInBytes: aligned_size as u32
                        }),
                        h,
                    );
                    cbv_index = Some(heap.get_handle_index(&h));
                }

                if info.usage.contains(super::BufferUsage::SHADER_RESOURCE) {
                    let h = heap.allocate();
                    self.device.CreateShaderResourceView(
                        &buf,
                        Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                            Format: dxgi_format,
                            ViewDimension: D3D12_SRV_DIMENSION_BUFFER,
                            Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                            Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                                Buffer: D3D12_BUFFER_SRV {
                                    FirstElement: 0,
                                    NumElements: info.num_elements as u32,
                                    StructureByteStride: info.stride as u32,
                                    Flags: D3D12_BUFFER_SRV_FLAG_NONE
                                }
                            }
                        }),
                        h,
                    );
                    srv_index = Some(heap.get_handle_index(&h));
                }

                // create uav
                if info.usage.contains(super::BufferUsage::UNORDERED_ACCESS) {
                    let h = heap.allocate();
                    if let Some(offset) = counter_offset {
                        // append counter buffers are implictly added to the end of the buffer
                        // different approches could be used with manually tracking and adding counters
                        // but this approach creates a d3d friendly `AppendStructuredBuffer`
                        self.device.CreateUnorderedAccessView(
                            &buf,
                            &buf,
                            Some(&D3D12_UNORDERED_ACCESS_VIEW_DESC{
                                Format: DXGI_FORMAT_UNKNOWN,
                                ViewDimension: D3D12_UAV_DIMENSION_BUFFER,
                                Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                                    Buffer: D3D12_BUFFER_UAV {
                                        FirstElement: 0,
                                        NumElements: info.num_elements as u32,
                                        StructureByteStride: info.stride as u32,
                                        CounterOffsetInBytes: offset as u64,
                                        Flags: D3D12_BUFFER_UAV_FLAG_NONE
                                    }
                                }
                            }),
                            h,
                        );
                    }
                    else {
                        self.device.CreateUnorderedAccessView(
                            &buf,
                            None,
                            None,
                            h,
                        );
                    }

                    uav_index = Some(heap.get_handle_index(&h));
                }
            }

            Ok(Buffer {
                resource: Some(buf),
                vbv,
                ibv,
                srv_index,
                cbv_index,
                counter_offset,
                uav_index,
                drop_list: Some(heap.drop_list.clone()),
                persistent_mapped_data: map_data
            })
        }
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        let mut heap = std::mem::take(&mut self.shader_heap).unwrap();
        let result = self.create_buffer_with_heap(info, data, &mut heap);
        self.shader_heap = Some(heap);
        result
    }

    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> result::Result<Self::Buffer, super::Error> {
        let buf = create_read_back_buffer(self, size as u64);
        if let Some(buf) = buf {
            Ok(Buffer {
                resource: Some(buf),
                vbv: None,
                ibv: None,
                srv_index: None,
                cbv_index: None,
                uav_index: None,
                drop_list: None,
                counter_offset: None,
                persistent_mapped_data: std::ptr::null_mut()
            })
        }
        else {
            Err( super::Error {
                msg: "hotline::gfx::d3d12:: failed to create readback buffer!".to_string()
            })
        }
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {
        self.create_texture_with_heaps(
            info,
            TextureHeapInfo {
                shader: None,
                render_target: None,
                depth_stencil: None
            },
            data
        )
    }

    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> result::Result<Self::Texture, super::Error> {
        let mut resource: Option<ID3D12Resource> = None;
        let mut resolved_resource: Option<ID3D12Resource> = None;

        let dxgi_format = to_dxgi_format(info.format);
        let size_bytes = size_for_format_mipped(
            info.format, info.width, info.height, info.depth, info.array_layers, info.mip_levels) as usize;
        validate_data_size(size_bytes, data)?;

        let initial_state = to_d3d12_resource_state(info.initial_state);

        let depth_or_array_size = max(info.depth, info.array_layers);
        unsafe {

            // msaa resources can only have 1 mip level, if we want to generate mips, it's on the resolve resource
            let primary_mip_levels = if info.samples > 1 {
                1
            }
            else {
                info.mip_levels
            };

            let extra_usage_flags = if info.usage.contains(super::TextureUsage::GENERATE_MIP_MAPS) && info.samples == 1 {
                super::TextureUsage::UNORDERED_ACCESS
            }
            else {
                super::TextureUsage::NONE
            };

            // create texture resource
            self.device.CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_DEFAULT,
                    ..Default::default()
                },
                to_d3d12_texture_heap_flags(info.usage),
                &D3D12_RESOURCE_DESC {
                    Dimension: to_d3d12_resource_dimension(info.tex_type),
                    Alignment: 0,
                    Width: info.width,
                    Height: info.height as u32,
                    DepthOrArraySize: depth_or_array_size as u16,
                    MipLevels: primary_mip_levels as u16,
                    Format: dxgi_format,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: info.samples,
                        Quality: 0,
                    },
                    Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                    Flags: to_d3d12_texture_usage_flags(info.usage | extra_usage_flags),
                },
                if data.is_some() {
                    D3D12_RESOURCE_STATE_COPY_DEST
                } else {
                    initial_state
                },
                None,
                &mut resource,
            )?;
            let resource = resource.unwrap();

            // create a resolvable texture if we have samples
            let extra_usage_flags = if info.usage.contains(super::TextureUsage::GENERATE_MIP_MAPS) {
                super::TextureUsage::UNORDERED_ACCESS
            }
            else {
                super::TextureUsage::NONE
            };

            if info.samples > 1 {
                self.device.CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    to_d3d12_texture_heap_flags(info.usage),
                    &D3D12_RESOURCE_DESC {
                        Dimension: to_d3d12_resource_dimension(info.tex_type),
                        Alignment: 0,
                        Width: info.width,
                        Height: info.height as u32,
                        DepthOrArraySize: depth_or_array_size as u16,
                        MipLevels: info.mip_levels as u16,
                        Format: dxgi_format,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                        Flags: to_d3d12_texture_usage_flags(info.usage | extra_usage_flags),
                    },
                    if data.is_some() {
                        D3D12_RESOURCE_STATE_COPY_DEST
                    } else {
                        initial_state
                    },
                    None,
                    &mut resolved_resource,
                )?;
            }

            // upload data
            if let Some(data) = data {
                self.upload_texture_data(info, data, dxgi_format, &resource, initial_state)?;
            }

            // select user specified shader heap or default
            let shader_heap = if let Some(shader_heap) = heaps.shader {
                shader_heap
            }
            else { 
                self.shader_heap.as_mut().unwrap()
            };

            // select user specified render target heap or default
            let rtv_heap = if let Some(rtv_heap) = heaps.render_target {
                rtv_heap
            }
            else { 
                &mut self.rtv_heap
            };      

            // select user specified depth stencil heap or default
            let dsv_heap = if let Some(dsv_heap) = heaps.depth_stencil {
                dsv_heap
            }
            else { 
                &mut self.dsv_heap
            };

            // create srv
            let mut srv_index = None;
            if info.usage.contains(super::TextureUsage::SHADER_RESOURCE) {
                let h = shader_heap.allocate();

                let dxgi_dormat_srv = to_dxgi_format_srv(info.format);
                let srv_dimension = to_d3d12_texture_srv_dimension(info.tex_type, info.samples);

                match info.tex_type {
                    // technically these should use thier own struct, but the members are equivalent within the union
                    // so we can just minimise code duplocation
                    super::TextureType::Texture2D | super::TextureType::TextureCube | super::TextureType::Texture3D => {
                        self.device.CreateShaderResourceView(
                            &resource,
                            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                                Format: dxgi_dormat_srv,
                                ViewDimension: srv_dimension,
                                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                                    Texture2D: D3D12_TEX2D_SRV {
                                        MipLevels: info.mip_levels,
                                        MostDetailedMip: 0,
                                        ..Default::default()
                                    },
                                },
                                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                            }),
                            h,
                        );
                    }
                    super::TextureType::Texture2DArray => {
                        self.device.CreateShaderResourceView(
                            &resource,
                            Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                                Format: dxgi_dormat_srv,
                                ViewDimension: srv_dimension,
                                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                                    Texture2DArray: D3D12_TEX2D_ARRAY_SRV {
                                        MostDetailedMip: 0,
                                        MipLevels: info.mip_levels,
                                        FirstArraySlice: 0,
                                        ArraySize: info.mip_levels,
                                        PlaneSlice: 0,
                                        ResourceMinLODClamp: 0.0,
                                    },
                                },
                                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                            }),
                            h,
                        );
                    }
                    _ => panic!("hotline_rs::gfx::d3d12:: not implemented shader resource view for type {:?}", info.tex_type)
                }

                srv_index = Some(shader_heap.get_handle_index(&h));
            }
            
            // create a srv for resolve texture for msaa
            let mut resolved_srv_index = None;
            let mut resolved_format = DXGI_FORMAT_UNKNOWN;
            if info.samples > 1 && info.usage.contains(super::TextureUsage::SHADER_RESOURCE) {
                let h = shader_heap.allocate();
                self.device.CreateShaderResourceView(
                    &resolved_resource.as_ref().unwrap().clone(),
                    Some(&D3D12_SHADER_RESOURCE_VIEW_DESC {
                        Format: to_dxgi_format_srv(info.format),
                        ViewDimension: to_d3d12_texture_srv_dimension(info.tex_type, 1),
                        Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                            Texture2D: D3D12_TEX2D_SRV {
                                MipLevels: info.mip_levels,
                                MostDetailedMip: 0,
                                ..Default::default()
                            },
                        },
                        Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                    }),
                    h,
                );
                resolved_srv_index = Some(shader_heap.get_handle_index(&h));
                resolved_format = to_dxgi_format_srv(info.format);
            }

            // create rtv
            let mut rtv = Vec::new();
            if info.usage.contains(super::TextureUsage::RENDER_TARGET) {
                match info.tex_type {
                    super::TextureType::Texture2DArray | super::TextureType::TextureCube => {
                        for i in 0..depth_or_array_size {
                            let h = rtv_heap.allocate();
                            self.device.CreateRenderTargetView(&resource, Some(&D3D12_RENDER_TARGET_VIEW_DESC{
                                Format: to_dxgi_format(info.format),
                                ViewDimension: D3D12_RTV_DIMENSION_TEXTURE2DARRAY,
                                Anonymous: D3D12_RENDER_TARGET_VIEW_DESC_0 {
                                    Texture2DArray: D3D12_TEX2D_ARRAY_RTV {
                                        MipSlice: 0,
                                        FirstArraySlice: i,
                                        ArraySize: 1,
                                        PlaneSlice: 0
                                    }
                                }
                            }), h);
                            rtv.push(TextureTarget{
                                ptr: h,
                                index: rtv_heap.get_handle_index(&h),
                                drop_list: rtv_heap.drop_list.clone()
                            });
                        }
                    }
                    _ => {
                        let h = rtv_heap.allocate();
                        self.device.CreateRenderTargetView(&resource, None, h);
                        rtv.push(TextureTarget{
                            ptr: h,
                            index: rtv_heap.get_handle_index(&h),
                            drop_list: rtv_heap.drop_list.clone()
                        });
                    }
                }
            }

            // create dsv
            let mut dsv = Vec::new();
            if info.usage.contains(super::TextureUsage::DEPTH_STENCIL) {
                match info.tex_type {
                    super::TextureType::Texture2DArray | super::TextureType::TextureCube => {
                        for i in 0..depth_or_array_size {
                            let h = dsv_heap.allocate();
                            self.device.CreateDepthStencilView(&resource, Some(&D3D12_DEPTH_STENCIL_VIEW_DESC{
                                Format: to_dxgi_format(info.format),
                                ViewDimension: D3D12_DSV_DIMENSION_TEXTURE2DARRAY,
                                Anonymous: D3D12_DEPTH_STENCIL_VIEW_DESC_0 {
                                    Texture2DArray: D3D12_TEX2D_ARRAY_DSV {
                                        MipSlice: 0,
                                        FirstArraySlice: i,
                                        ArraySize: 1
                                    }
                                },
                                Flags: D3D12_DSV_FLAG_NONE
                            }), h);
                            dsv.push(TextureTarget{
                                ptr: h,
                                index: dsv_heap.get_handle_index(&h),
                                drop_list: dsv_heap.drop_list.clone()
                            });
                        }
                    }
                    _ => {
                        let h = dsv_heap.allocate();
                        self.device.CreateDepthStencilView(&resource, None, h);
                        dsv.push(TextureTarget{
                            ptr: h,
                            index: dsv_heap.get_handle_index(&h),
                            drop_list: dsv_heap.drop_list.clone()
                        });
                    }
                }
            }

            // create uav
            let mut uav_index = None;
            if info.usage.contains(super::TextureUsage::UNORDERED_ACCESS) {
                let h = shader_heap.allocate();
                self.device.CreateUnorderedAccessView(
                    &resource,
                    None,
                    None,
                    h,
                );
                uav_index = Some(shader_heap.get_handle_index(&h));
            }

            // create shared handle for video decode targets
            let mut shared_handle = None;
            if info.usage.contains(super::TextureUsage::VIDEO_DECODE_TARGET) {
                let h = self.device.CreateSharedHandle(
                    &resource,
                    None,
                    GENERIC_ALL.0,
                    PCWSTR(std::ptr::null())
                );
                shared_handle = Some(h?);
            }

            // create uav's for a mip chain
            let mut subresource_uav_index = Vec::new();
            if info.usage.contains(super::TextureUsage::GENERATE_MIP_MAPS) {
                for mip in 0..info.mip_levels {
                    let h = shader_heap.allocate();
                    self.device.CreateUnorderedAccessView(
                        if let Some(resolved_resource) = &resolved_resource { 
                            resolved_resource 
                        } 
                        else { 
                            &resource 
                        },
                        None,
                        Some(&D3D12_UNORDERED_ACCESS_VIEW_DESC{
                            Format: to_dxgi_format_srv(info.format),
                            ViewDimension: D3D12_UAV_DIMENSION_TEXTURE2D,
                            Anonymous: D3D12_UNORDERED_ACCESS_VIEW_DESC_0 {
                                Texture2D: D3D12_TEX2D_UAV {
                                    MipSlice: mip,
                                    PlaneSlice: 0
                                }
                            }
                        }),
                        h,
                    );
                    subresource_uav_index.push(shader_heap.get_handle_index(&h));
                }
            }

            Ok(Texture {
                resource: Some(resource),
                resolved_resource,
                resolved_format,
                rtv,
                dsv,
                srv_index,
                resolved_srv_index,
                uav_index,
                shared_handle,
                drop_list: Some(shader_heap.drop_list.clone()),
                subresource_uav_index,
                shader_heap_id: Some(shader_heap.id)
            })
        }
    }

    fn create_render_pass(
        &self,
        info: &super::RenderPassInfo<Device>,
    ) -> result::Result<RenderPass, super::Error> {
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
            let desc = unsafe { target.resource.as_ref().unwrap().GetDesc() };
            let dxgi_format = desc.Format;
            let target_sample_count = desc.SampleDesc.Count;
            if sample_count.is_none() {
                sample_count = Some(target_sample_count);
            } 
            else if sample_count.unwrap() != target_sample_count {
                return Err( super::Error {
                    msg: format!("Sample counts must match on all targets: expected {} samples, found {}", 
                    sample_count.unwrap(),
                    target_sample_count
                )});
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
                cpuDescriptor: target.rtv[info.array_slice].ptr,
                BeginningAccess: begin,
                EndingAccess: end,
            })
        }

        let mut ds = None;
        let mut ds_format = DXGI_FORMAT_UNKNOWN;
        if let Some(depth_stencil) = &info.depth_stencil {
            let mut clear_depth = 0.0;
            let mut depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE;
            let mut clear_stencil = 0x0;
            let mut stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_PRESERVE;

            match &info.ds_clear {
                None => {
                    if info.discard {
                        depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD;
                        stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_DISCARD;
                    }
                }
                Some(ds_clear) => {
                    match &ds_clear.depth {
                        Some(depth) => {
                            depth_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR;
                            clear_depth = *depth
                        }
                        None => (),
                    }
                    match &ds_clear.stencil {
                        Some(stencil) => {
                            stencil_begin_type = D3D12_RENDER_PASS_BEGINNING_ACCESS_TYPE_CLEAR;
                            clear_stencil = *stencil
                        }
                        None => (),
                    }
                }
            }

            let desc = unsafe { depth_stencil.resource.as_ref().unwrap().GetDesc() };
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
                                    Stencil: clear_stencil,
                                },
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
                                    Stencil: clear_stencil,
                                },
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

            // TODO: if no dsv
            ds = Some(D3D12_RENDER_PASS_DEPTH_STENCIL_DESC {
                cpuDescriptor: depth_stencil.dsv[info.array_slice].ptr,
                DepthBeginningAccess: depth_begin,
                StencilBeginningAccess: stencil_begin,
                DepthEndingAccess: depth_end,
                StencilEndingAccess: stencil_end,
            });
        }

        // hash together the rt, ds and sample count to get a unique hash for format combo
        let mut fmthash = DefaultHasher::new();

        let isample_count = if let Some(i) = sample_count { i } else { 1 };
        isample_count.hash(&mut fmthash);

        ds_format.0.hash(&mut fmthash);
        for rt in &formats {
            rt.0.hash(&mut fmthash);
        }
        
        Ok(RenderPass {
            rt,
            ds,
            ds_format,
            rt_formats: formats,
            sample_count: isample_count,
            format_hash: fmthash.finish()
        })
    }

    fn create_compute_pipeline(
        &self,
        info: &super::ComputePipelineInfo<Self>,
    ) -> result::Result<ComputePipeline, super::Error> {
        let cs = &info.cs;
        let sig_lookup = self.create_root_signature_with_lookup(&info.pipeline_layout)?;

        let desc = D3D12_COMPUTE_PIPELINE_STATE_DESC {
            CS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: cs.get_buffer_pointer(),
                BytecodeLength: cs.get_buffer_size(),
            },
            pRootSignature: unsafe { std::mem::transmute_copy(&sig_lookup.root_signature) },
            ..Default::default()
        };

        unsafe {
            Ok(ComputePipeline {
                pso: self.device.CreateComputePipelineState(&desc)?,
                root_signature: sig_lookup.root_signature.clone(),
                lookup: sig_lookup
            })
        }
    }

    fn create_raytracing_pipeline(
        &self,
        info: &super::RaytracingPipelineInfo<Self>,
    ) -> result::Result<RaytracingPipeline, super::Error> {

        let mut subobjects = Vec::new();
        
        // rt shader config // TODO: move consts into info struct
        let max_payload_size: u32 = 64; // Max size for ray payload (in bytes)
        let max_attribute_size: u32 = 32; // Max size for intersection attributes (in bytes)
        let shader_config = D3D12_RAYTRACING_SHADER_CONFIG {
            MaxPayloadSizeInBytes: max_payload_size,
            MaxAttributeSizeInBytes: max_attribute_size,
        };
        let shader_config_subobject = D3D12_STATE_SUBOBJECT {
            Type: D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_SHADER_CONFIG,
            pDesc: &shader_config as *const _ as *const _,
        };
        subobjects.push(shader_config_subobject);

        // pipeline config
        let pipeline_config = D3D12_RAYTRACING_PIPELINE_CONFIG {
            MaxTraceRecursionDepth: 1, // TODO: parameterize
        };
        let pipeline_config_subobject = D3D12_STATE_SUBOBJECT {
            Type: D3D12_STATE_SUBOBJECT_TYPE_RAYTRACING_PIPELINE_CONFIG,
            pDesc: &pipeline_config as *const _ as *const _,
        };
        subobjects.push(pipeline_config_subobject);

        // dxil library shaders
        let mut library_descs = Vec::new();
        for (index, shader) in info.shaders.iter().enumerate().rev() {
            // dxil library
            let dxil_library = D3D12_DXIL_LIBRARY_DESC {
                DXILLibrary: D3D12_SHADER_BYTECODE {
                    pShaderBytecode: shader.shader.get_buffer_pointer(),
                    BytecodeLength: shader.shader.get_buffer_size(),
                },
                ..Default::default()
            };
            library_descs.push(dxil_library);
            let dxil_library_subobject = D3D12_STATE_SUBOBJECT {
                Type: D3D12_STATE_SUBOBJECT_TYPE_DXIL_LIBRARY,
                pDesc: &library_descs[info.shaders.len() - index - 1] as *const _ as *const _,
            };
            subobjects.push(dxil_library_subobject);
        }

        // root signature, for now we use a global one per pipeline
        let root_signature = self.create_root_signature_with_lookup(&info.pipeline_layout)?;
        let global_root_signature = D3D12_GLOBAL_ROOT_SIGNATURE {
            pGlobalRootSignature: std::mem::ManuallyDrop::new(Some(root_signature.root_signature.clone())) // TODO: cleanup
        };
        let global_root_signature_subobject = D3D12_STATE_SUBOBJECT {
            Type: D3D12_STATE_SUBOBJECT_TYPE_GLOBAL_ROOT_SIGNATURE,
            pDesc: &global_root_signature as *const _ as *const _,
        };
        subobjects.push(global_root_signature_subobject);

        // retuns null if the vec is empty
        let wstr_or_null = |vec: &Vec<u16> | -> windows_core::PCWSTR {
            if vec.is_empty() {
                windows_core::PCWSTR::null()
            }
            else {
                windows_core::PCWSTR(vec.as_ptr())
            }
        };

        // hitgroup config
        let mut wide_hit_group_strings = Vec::new();
        let mut hit_group_descs = Vec::new();
        if let Some(hit_groups) = &info.hit_groups {
            for group in hit_groups {
                let index = hit_group_descs.len();
                // widen strings
                let vpos = wide_hit_group_strings.len();
                wide_hit_group_strings.push(os::win32::string_to_wide(group.name.clone()));
                wide_hit_group_strings.push(if let Some(entry) = group.any_hit.as_ref() { 
                    os::win32::string_to_wide(entry.clone()) } else { Vec::new() });
                wide_hit_group_strings.push(if let Some(entry) = group.closest_hit.as_ref() { 
                    os::win32::string_to_wide(entry.clone()) } else { Vec::new() });
                wide_hit_group_strings.push(if let Some(entry) = group.intersection.as_ref() { 
                    os::win32::string_to_wide(entry.clone()) } else { Vec::new() });
                // create desc
                let hit_group_desc = D3D12_HIT_GROUP_DESC {
                    Type: to_d3d12_hit_group_type(&group.geometry),
                    HitGroupExport: wstr_or_null(&wide_hit_group_strings[vpos]),
                    AnyHitShaderImport: wstr_or_null(&wide_hit_group_strings[vpos+1]),
                    ClosestHitShaderImport: wstr_or_null(&wide_hit_group_strings[vpos+2]),
                    IntersectionShaderImport: wstr_or_null(&wide_hit_group_strings[vpos+3]),
                };
                hit_group_descs.push(hit_group_desc);
                let hit_group_subobject = D3D12_STATE_SUBOBJECT {
                    Type: D3D12_STATE_SUBOBJECT_TYPE_HIT_GROUP,
                    pDesc: &hit_group_descs[index] as *const _ as *const _,
                };
                subobjects.push(hit_group_subobject);
            }
        }

        // create a state object descriptor
        let state_object_desc = D3D12_STATE_OBJECT_DESC {
            Type: D3D12_STATE_OBJECT_TYPE_RAYTRACING_PIPELINE,
            NumSubobjects: subobjects.len() as u32,
            pSubobjects: subobjects.as_ptr() as *const _,
        };

        unsafe {
            let device5 = self.device.cast::<ID3D12Device5>().expect("hotline_rs::gfx::d3d12: expected ID3D12Device5 availability to create raytracing pipeline");
            let state_object : ID3D12StateObject = device5.CreateStateObject(
                &state_object_desc,
            )?;

            Ok(RaytracingPipeline {
                state_object,
                root_signature: root_signature.root_signature.clone(), // TODO: we
                lookup: root_signature
            })
        }
    }

    fn create_raytracing_shader_binding_table(
        &self,
        info: &super::RaytracingShaderBindingTableInfo<Self>
    ) -> result::Result<RaytracingShaderBindingTable, super::Error> {
        unsafe { 
            let create_shader_table = |idents : Vec<*mut c_void> | -> ShaderTable {
                if idents.is_empty() {
                    ShaderTable {
                        buffer: None,
                        count: 0,
                        stride: 0
                    }
                }
                else {
                    // create a table resource
                    let mut table_buffer: Option<ID3D12Resource> = None;
                    let buffer_size = D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as usize * idents.len();
                    self.device.CreateCommittedResource(
                        &D3D12_HEAP_PROPERTIES {
                            Type: D3D12_HEAP_TYPE_UPLOAD,
                            ..Default::default()
                        },
                        D3D12_HEAP_FLAG_NONE,
                        &D3D12_RESOURCE_DESC {
                            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                            Alignment: 0,
                            Width: buffer_size as u64,
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
                        None,
                        &mut table_buffer,
                    ).expect("hotline_rs::gfx::d3d12: failed to create a shader binding table buffer");

                    // 
                    if let Some(resource) = table_buffer.as_ref() {
                        let range = D3D12_RANGE { Begin: 0, End: 0 };
                        let mut map_data = std::ptr::null_mut();
                        resource.Map(0, Some(&range), Some(&mut map_data)).expect("hotline_rs::gfx::d3d12: failed to map buffer data for the shader binding table");
                        std::ptr::copy_nonoverlapping(idents.as_ptr() as *mut _, map_data, buffer_size);
                        resource.Unmap(0, None);
                    }

                    ShaderTable {
                        buffer: table_buffer,
                        count: idents.len(),
                        stride: D3D12_SHADER_IDENTIFIER_SIZE_IN_BYTES as usize
                    }
                }
            };

            // get shader identifiers
            let props = info.pipeline.state_object.cast::<ID3D12StateObjectProperties>().expect("hotline_rs::gfx::d3d12: expected ID3D12StateObjectProperties");
            let raygen_id = props.GetShaderIdentifier(windows_core::PCWSTR(os::win32::string_ref_to_wide(&info.ray_generation_shader).as_ptr()));

            let miss_ids = info.miss_shaders.iter()
                .map(|x| props.GetShaderIdentifier(windows_core::PCWSTR(os::win32::string_ref_to_wide(x).as_ptr())))
                .collect();

            let callable_ids = info.callable_shaders.iter()
                .map(|x| props.GetShaderIdentifier(windows_core::PCWSTR(os::win32::string_ref_to_wide(x).as_ptr())))
                .collect();

            let hit_group_ids = info.hit_groups.iter()
                .map(|x| props.GetShaderIdentifier(windows_core::PCWSTR(os::win32::string_ref_to_wide(x).as_ptr())))
                .collect();

            // create
            Ok(RaytracingShaderBindingTable {
                ray_generation: create_shader_table(vec![raygen_id]),
                miss: create_shader_table(miss_ids),
                hit_group: create_shader_table(callable_ids),
                callable: create_shader_table(hit_group_ids),
            })
        }
    }

    fn create_raytracing_blas(
        &mut self,
        info: &RaytracingBLASInfo<Self>
    ) -> result::Result<RaytracingBLAS, super::Error> {
        // create geometry desc
        let geometry_desc = match &info.geometry {
            RaytracingGeometryInfo::Triangles(tris) => {
                D3D12_RAYTRACING_GEOMETRY_DESC {
                    Type: D3D12_RAYTRACING_GEOMETRY_TYPE_TRIANGLES,
                    Flags: to_d3d12_raytracing_geometry_flags(info.geometry_flags),
                    Anonymous: D3D12_RAYTRACING_GEOMETRY_DESC_0 {
                        Triangles: D3D12_RAYTRACING_GEOMETRY_TRIANGLES_DESC {
                            Transform3x4: tris.transform3x4.map(|x| x.d3d_virtual_address()).unwrap_or(0),
                            IndexFormat: to_dxgi_format(tris.index_format),
                            VertexFormat: to_dxgi_format(tris.vertex_format),
                            IndexCount: tris.index_count as u32,
                            VertexCount: tris.index_count as u32,
                            IndexBuffer: tris.index_buffer.d3d_virtual_address(),
                            VertexBuffer: D3D12_GPU_VIRTUAL_ADDRESS_AND_STRIDE {
                                StartAddress: tris.vertex_buffer.d3d_virtual_address(),
                                StrideInBytes: size_for_format(tris.vertex_format, 1, 1, 1)
                            },
                        }
                    }
                }
            }
            _ => {
                unimplemented!();
            }
        };

        // create acceleration structure inputs
        let inputs = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Type: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_BOTTOM_LEVEL,
            Flags: to_d3d12_raytracing_acceleration_structure_build_flags(info.build_flags),
            NumDescs: 1,
            DescsLayout: D3D12_ELEMENTS_LAYOUT_ARRAY,
            Anonymous: D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS_0 {
                pGeometryDescs: &geometry_desc
            }
        };

        // get prebuild info
        let prebuild_info = unsafe {
            let mut prebuild_info: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO = std::mem::zeroed();
            let device5 = self.device.cast::<ID3D12Device5>().expect("hotline_rs::gfx::d3d12: expected ID3D12Device5 availability to create raytracing blas");
            device5.GetRaytracingAccelerationStructurePrebuildInfo(&inputs, &mut prebuild_info);
            prebuild_info
        };

        // UAV scratch buffer
        let scratch_buffer = self.create_buffer::<u8>(&BufferInfo {
            usage: super::BufferUsage::UNORDERED_ACCESS | super::BufferUsage::BUFFER_ONLY,
            cpu_access: super::CpuAccessFlags::NONE,
            format: super::Format::Unknown,
            stride: prebuild_info.ScratchDataSizeInBytes as usize,
            num_elements: 1,
            initial_state: super::ResourceState::UnorderedAccess
        }, None).expect(format!("hotline_rs::gfx::d3d12: failed to create a scratch buffer for raytracing blas of size {}", prebuild_info.ScratchDataSizeInBytes).as_str());

        // UAV buffer the blas
        let blas_buffer = self.create_buffer::<u8>(&BufferInfo {
            usage: super::BufferUsage::UNORDERED_ACCESS | super::BufferUsage::BUFFER_ONLY,
            cpu_access: super::CpuAccessFlags::NONE,
            format: super::Format::Unknown,
            stride: prebuild_info.ResultDataMaxSizeInBytes as usize,
            num_elements: 1,
            initial_state: super::ResourceState::AccelerationStructure
        }, None).expect(format!("hotline_rs::gfx::d3d12: failed to create a scratch buffer for raytracing blas of size {}", prebuild_info.ScratchDataSizeInBytes).as_str());

        // create blas desc
        let blas_desc = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
            Inputs: inputs,
            SourceAccelerationStructureData: 0,
            DestAccelerationStructureData: blas_buffer.d3d_virtual_address(),
            ScratchAccelerationStructureData: scratch_buffer.d3d_virtual_address()
        };

        // build blas
        unsafe {
            let cmd = self.command_list.cast::<ID3D12GraphicsCommandList4>()
                .expect("hotline_rs::gfx::d3d12: expected ID3D12GraphicsCommandList4 availability to create raytracing blas");
            cmd.BuildRaytracingAccelerationStructure(&blas_desc, None);
            cmd.Close()?;

            // TODO: use submit and wait
            // execute commandlist
            let cmd = Some(self.command_list.cast().unwrap());
            self.command_queue.ExecuteCommandLists(&[cmd]);

            // wait
            let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE)?;
            self.command_queue.Signal(&fence, 1)?;
            let event = CreateEventA(None, false, false, None)?;
            fence.SetEventOnCompletion(1, event)?;
            WaitForSingleObject(event, INFINITE);

            // reset command list
            self.command_list.Reset(&self.command_allocator, None)?;

            Ok(RaytracingBLAS {
                blas_buffer
            })
        }
    }

    fn create_raytracing_tlas(
        &mut self,
        info: &RaytracingTLASInfo<Self>
    ) -> result::Result<RaytracingTLAS, super::Error> {

        // pack 24: 8 bits
        let pack_24_8 = |a, b| {
            ((a & 0x00ffffff) << 8) | (b & 0x000000ff)
        };

        // create instance descs
        let num_instances = info.instances.len();
        let instance_descs: Vec<D3D12_RAYTRACING_INSTANCE_DESC> = info.instances.iter()
            .map(|x| 
                D3D12_RAYTRACING_INSTANCE_DESC {
                Transform: x.transform,
                _bitfield1: pack_24_8(x.instance_mask, x.instance_id),
                _bitfield2: pack_24_8(x.hit_group_index, x.instance_flags),
                AccelerationStructure: x.blas.blas_buffer.d3d_virtual_address()
            })
            .collect();

        // create upload buffer for instance descs
        let stride = std::mem::size_of::<D3D12_RAYTRACING_INSTANCE_DESC>();
        let instance_buffer = self.create_buffer(&BufferInfo {
            usage: super::BufferUsage::UPLOAD,
            cpu_access: super::CpuAccessFlags::NONE,
            format: super::Format::Unknown,
            stride: std::mem::size_of::<D3D12_RAYTRACING_INSTANCE_DESC>(),
            num_elements: num_instances,
            initial_state: super::ResourceState::GenericRead
        }, Some(instance_descs.as_slice()))
        .expect(format!("hotline_rs::gfx::d3d12: failed to create a scratch buffer for raytracing blas of size {}", stride * num_instances).as_str());

        // create acceleration structure inputs
        let inputs = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS {
            Type: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_TYPE_TOP_LEVEL,
            Flags: to_d3d12_raytracing_acceleration_structure_build_flags(info.build_flags),
            NumDescs: num_instances as u32,
            DescsLayout: D3D12_ELEMENTS_LAYOUT_ARRAY,
            Anonymous: D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_INPUTS_0 {
                InstanceDescs: instance_buffer.d3d_virtual_address(),
            }
        };

        // get prebuild info
        let prebuild_info = unsafe {
            let mut prebuild_info: D3D12_RAYTRACING_ACCELERATION_STRUCTURE_PREBUILD_INFO = std::mem::zeroed();
            let device5 = self.device.cast::<ID3D12Device5>()
                .expect("hotline_rs::gfx::d3d12: expected ID3D12Device5 availability to create raytracing tlas");
            device5.GetRaytracingAccelerationStructurePrebuildInfo(&inputs, &mut prebuild_info);
            prebuild_info
        };

        // UAV scratch buffer
        let scratch_buffer = self.create_buffer::<u8>(&BufferInfo {
            usage: super::BufferUsage::UNORDERED_ACCESS | super::BufferUsage::BUFFER_ONLY,
            cpu_access: super::CpuAccessFlags::NONE,
            format: super::Format::Unknown,
            stride: prebuild_info.ScratchDataSizeInBytes as usize,
            num_elements: 1,
            initial_state: super::ResourceState::UnorderedAccess
        }, None)
        .expect(format!("hotline_rs::gfx::d3d12: failed to create a scratch buffer for raytracing tlas of size {}", prebuild_info.ScratchDataSizeInBytes).as_str());

        // UAV buffer the tlas
        let tlas_buffer = self.create_buffer::<u8>(&BufferInfo {
            usage: super::BufferUsage::UNORDERED_ACCESS | super::BufferUsage::BUFFER_ONLY,
            cpu_access: super::CpuAccessFlags::NONE,
            format: super::Format::Unknown,
            stride: prebuild_info.ResultDataMaxSizeInBytes as usize,
            num_elements: 1,
            initial_state: super::ResourceState::AccelerationStructure
        }, None).expect(format!("hotline_rs::gfx::d3d12: failed to create a scratch buffer for raytracing blas of size {}", prebuild_info.ScratchDataSizeInBytes).as_str());

        // create tlas desc
        let tlas_desc = D3D12_BUILD_RAYTRACING_ACCELERATION_STRUCTURE_DESC {
            Inputs: inputs,
            SourceAccelerationStructureData: 0,
            DestAccelerationStructureData: tlas_buffer.d3d_virtual_address(),
            ScratchAccelerationStructureData: scratch_buffer.d3d_virtual_address()
        };

        // build tlas
        unsafe {
            let cmd = self.command_list.cast::<ID3D12GraphicsCommandList4>()
                .expect("hotline_rs::gfx::d3d12: expected ID3D12GraphicsCommandList4 availability to create raytracing blas");
            cmd.BuildRaytracingAccelerationStructure(&tlas_desc, None);
            cmd.Close()?;
        }

        // wait for completion
        self.execute_and_wait_on_resource_cmd()?;

        // return the result
        Ok(RaytracingTLAS {
            tlas_buffer
        })
    }

    fn create_indirect_render_command<T: Sized>(
        &mut self, 
        arguments: Vec<super::IndirectArgument>,
        pipeline: Option<&RenderPipeline>) -> result::Result<CommandSignature, super::Error> {
        
        let descs = arguments.iter().map(|arg|
            match arg.argument_type {
                super::IndirectArgumentType::PushConstants => {
                    if let Some(args) = &arg.arguments {
                        unsafe {
                            D3D12_INDIRECT_ARGUMENT_DESC {
                                Type: to_d3d12_indirect_argument_type(arg.argument_type),
                                Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
                                    Constant: D3D12_INDIRECT_ARGUMENT_DESC_0_1 {
                                        RootParameterIndex: args.push_constants.slot,
                                        DestOffsetIn32BitValues: args.push_constants.offset,
                                        Num32BitValuesToSet: args.push_constants.num_values
                                    }
                                }
                            }
                        }
                    }
                    else {
                        panic!("hotline_rs::gfx::d3d12:: indirect push constants must specify `push_constants` arguments");
                    }
                },
                super::IndirectArgumentType::VertexBuffer => {
                    if let Some(args) = &arg.arguments {
                        unsafe {
                            D3D12_INDIRECT_ARGUMENT_DESC {
                                Type: to_d3d12_indirect_argument_type(arg.argument_type),
                                Anonymous: D3D12_INDIRECT_ARGUMENT_DESC_0 {
                                    VertexBuffer: D3D12_INDIRECT_ARGUMENT_DESC_0_5 {
                                        Slot: args.buffer.slot,
                                    }
                                }
                            }
                        }
                    }
                    else {
                        panic!("hotline_rs::gfx::d3d12:: indirect vertex buffer must specify `buffer` arguments");
                    }
                }
                _ => {
                    D3D12_INDIRECT_ARGUMENT_DESC {
                        Type: to_d3d12_indirect_argument_type(arg.argument_type),
                        ..Default::default()
                    }
                }
            }
        ).collect::<Vec<D3D12_INDIRECT_ARGUMENT_DESC>>();

        let cmd_desc = D3D12_COMMAND_SIGNATURE_DESC {
            pArgumentDescs: descs.as_ptr() as *const D3D12_INDIRECT_ARGUMENT_DESC,
            NumArgumentDescs: descs.len() as u32,
            ByteStride: std::mem::size_of::<T>() as u32,
            NodeMask: 0
        };

        // If the number of args is just 1 and is draw, draw_indexed it is invalid to pass a pipeline
        let mut sig : Option<ID3D12CommandSignature> = None;
        if let Some(pipeline) = pipeline {
            unsafe {
                self.device.CreateCommandSignature(&cmd_desc, &pipeline.root_signature, &mut sig)?;
            }
        }
        else {
            unsafe {
                self.device.CreateCommandSignature(&cmd_desc, None, &mut sig)?;
            }
        };

        Ok(CommandSignature {
            command_signature: sig.unwrap()
        })
    }    

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = Some(cmd.command_list[cmd.bb_index].cast().unwrap());
            self.command_queue.ExecuteCommandLists(&[command_list]);
        }
    }

    fn report_live_objects(&self) -> result::Result<(), super::Error> {
        if cfg!(debug_assertions) {
            let debug_device : ID3D12DebugDevice = self.device.cast()?;
            unsafe {
                debug_device.ReportLiveDeviceObjects(D3D12_RLDO_DETAIL)?;
            }
        }
        Ok(())
    }

    fn get_info_queue_messages(&self) -> result::Result<Vec<String>, super::Error> {
        let mut output = Vec::new();
        if cfg!(debug_assertions) {
            let info_queue : ID3D12InfoQueue = self.device.cast()?;
            unsafe {
                let count = info_queue.GetNumStoredMessages();
                for i in 0..count {
                    let mut size = 0;
                    info_queue.GetMessage(i, None, &mut size)?;

                    let layout = std::alloc::Layout::from_size_align(size, 4)?;
                    let data = std::alloc::alloc(layout);

                    assert!(size <= layout.size(), 
                        "{} vs {}", size, layout.size());

                    info_queue.GetMessage(i, Some(data as *mut D3D12_MESSAGE), &mut size)?;

                    let msg = *(data as *mut D3D12_MESSAGE);

                    let mut descripton_chars = Vec::new();
                    for c in 0..msg.DescriptionByteLength {
                        descripton_chars.push(*msg.pDescription.add(c));
                    }
                    let str = std::str::from_utf8(descripton_chars.as_slice())?;
                    output.push(str.to_string());

                    std::ptr::drop_in_place(data);
                }
                info_queue.ClearStoredMessages();
            }
        }
        Ok(output)
    }

    fn get_shader_heap(&self) -> &Self::Heap {
        self.shader_heap.as_ref().unwrap()
    }

    fn get_shader_heap_mut(&mut self) -> &mut Self::Heap {
        self.shader_heap.as_mut().unwrap()
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &Self::SwapChain) {
        self.shader_heap.as_mut().unwrap().cleanup_dropped_resources(swap_chain);
        self.rtv_heap.cleanup_dropped_resources(swap_chain);
        self.dsv_heap.cleanup_dropped_resources(swap_chain);
    }

    fn get_adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    fn get_feature_flags(&self) -> &DeviceFeatureFlags {
        &self.feature_flags
    }

    fn read_buffer(&self, swap_chain: &SwapChain, buffer: &Buffer, size: usize, frame_written_fence: u64) -> Option<super::ReadBackData> {
        let rr = ReadBackRequest {
            resource: Some(buffer.resource.as_ref().unwrap().clone()),
            fence_value: frame_written_fence,
            size,
            row_pitch: size,
            slice_pitch: size,
        };
    
        if rr.is_complete(swap_chain) {
            let map = rr.map(&MapInfo { 
                subresource: 0, 
                read_start: 0, 
                read_end: size 
            });
            if let Ok(map) = map {
                rr.unmap();
                Some(map)
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    fn read_timestamps(&self, swap_chain: &SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Vec<f64> {
        let data = self.read_buffer(swap_chain, buffer, size_bytes, frame_written_fence);
        let mut results = Vec::new();
        if let Some(data) = data {
            let elem_size = Self::get_timestamp_size_bytes();
            let count = size_bytes / elem_size;
            for i in 0..count {
                let offset = i * elem_size;
                let value = u64::from_ne_bytes(data.data[offset..elem_size].try_into().unwrap());
                let timestamp = value as f64 / self.timestamp_frequency;
                results.push(timestamp);
            }
        }
        results
    }

    fn read_pipeline_statistics(&self, swap_chain: &SwapChain, buffer: &Self::Buffer, frame_written_fence: u64) -> Option<super::PipelineStatistics> {
        let size_bytes = Self::get_pipeline_statistics_size_bytes();
        let data = self.read_buffer(swap_chain, buffer, size_bytes, frame_written_fence);
        if let Some(data) = data {
            let d3d12_query_stats: D3D12_QUERY_DATA_PIPELINE_STATISTICS = unsafe { 
                std::ptr::read_unaligned(data.data.as_ptr() as *const _) 
            };

            Some(super::PipelineStatistics{
                input_assembler_vertices: d3d12_query_stats.IAVertices,
                input_assembler_primitives: d3d12_query_stats.IAPrimitives,
                vertex_shader_invocations: d3d12_query_stats.VSInvocations,
                pixel_shader_primitives: d3d12_query_stats.PSInvocations, 
                compute_shader_invocations: d3d12_query_stats.CSInvocations
            })
        }
        else {
            None
        }
    }

    fn get_timestamp_size_bytes() -> usize {
        8 // U64 
    }

    fn get_pipeline_statistics_size_bytes() -> usize {
        std::mem::size_of::<D3D12_QUERY_DATA_PIPELINE_STATISTICS>()
    }

    fn get_indirect_command_size(argument_type: IndirectArgumentType) -> usize {
        match argument_type {
            IndirectArgumentType::Draw => std::mem::size_of::<D3D12_DRAW_ARGUMENTS>(),
            IndirectArgumentType::DrawIndexed => std::mem::size_of::<D3D12_DRAW_INDEXED_ARGUMENTS>(),
            IndirectArgumentType::Dispatch => std::mem::size_of::<D3D12_DISPATCH_ARGUMENTS>(),
            _ => panic!("hotline_rs::d3d12:: not implemented")
        }
    }

    fn get_counter_alignment() -> usize {
        D3D12_UAV_COUNTER_PLACEMENT_ALIGNMENT as usize
    }
}

impl SwapChain {
    fn wait_for_frame(&mut self, frame_index: usize) {
        unsafe {
            let mut fv = self.frame_fence_value[frame_index];

            // 0 means no fence was signaled
            if fv != 0 {
                fv = 0;
                self.fence
                    .SetEventOnCompletion(fv, self.fence_event)
                    .expect("hotline_rs::gfx::d3d12: failed to set on completion event!");
                WaitForMultipleObjects(
                    &[self.swap_chain.GetFrameLatencyWaitableObject(), self.fence_event], 
                    true, INFINITE);
            }
            else {
                WaitForMultipleObjects(&[self.swap_chain.GetFrameLatencyWaitableObject()], true, INFINITE);
            }
        }
    }
}

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
        self.wait_for_frame(self.bb_index);
    }

    fn wait_for_last_frame(&self) {
        unsafe {
            self.fence
                .SetEventOnCompletion(self.fence_last_signalled_value, self.fence_event)
                .expect("hotline_rs::gfx::d3d12: failed to set on completion event!");
            WaitForMultipleObjects(&[self.fence_event], true, INFINITE);
        }
    }

    fn get_num_buffers(&self) -> u32 {
        self.num_bb
    }

    fn get_frame_fence_value(&self) -> u64 {
        self.frame_fence_value[self.bb_index]
    }

    fn update<A: os::App>(&mut self, device: &mut Device, window: &A::Window, cmd: &mut CmdBuf) {
        let size = window.get_size();
        if (size.x != self.width || size.y != self.height) && size.x > 0 && size.y > 0 {
            unsafe {
                self.wait_for_frame(self.bb_index);
                
                cmd.drop_complete_in_flight_barriers(cmd.bb_index);

                // add the rtv's to free list
                for tex in &self.backbuffer_textures {
                    for rtv in &tex.rtv {
                        device.rtv_heap.deallocate(rtv.index);
                    }
                }

                // clean up texture resource
                self.backbuffer_textures.clear();

                self.swap_chain
                    .ResizeBuffers(
                        self.num_bb,
                        size.x as u32,
                        size.y as u32,
                        DXGI_FORMAT_UNKNOWN,
                        DXGI_SWAP_CHAIN_FLAG(self.flags as i32),
                    )
                    .expect("hotline_rs::gfx::d3d12: warning: present failed!");

                let data_size = super::slice_pitch_for_format(
                    self.format,
                    self.width as u64,
                    self.height as u64,
                );
                self.backbuffer_textures =
                    create_swap_chain_rtv(&self.swap_chain, device, self.num_bb);
                self.backbuffer_passes = device.create_render_passes_for_swap_chain(
                    self.num_bb,
                    &self.backbuffer_textures,
                    self.clear_col,
                );
                self.backbuffer_passes_no_clear = device.create_render_passes_for_swap_chain(
                    self.num_bb,
                    &self.backbuffer_textures,
                    None,
                );

                self.readback_buffer = create_read_back_buffer(device, data_size);
                self.width = size.x;
                self.height = size.y;
                self.bb_index = 0;
            }
        } else {
            self.new_frame();
        }
    }

    fn get_backbuffer_index(&self) -> u32 {
        self.bb_index as u32
    }

    fn get_backbuffer_texture(&self) -> &Texture {
        &self.backbuffer_textures[self.bb_index]
    }

    fn get_backbuffer_pass(&self) -> &RenderPass {
        &self.backbuffer_passes[self.bb_index]
    }

    fn get_backbuffer_pass_mut(&mut self) -> &mut RenderPass {
        &mut self.backbuffer_passes[self.bb_index]
    }

    fn get_backbuffer_pass_no_clear(&self) -> &RenderPass {
        &self.backbuffer_passes_no_clear[self.bb_index]
    }

    fn get_backbuffer_pass_no_clear_mut(&mut self) -> &mut RenderPass {
        &mut self.backbuffer_passes_no_clear[self.bb_index]
    }

    fn swap(&mut self, device: &Device) {
        unsafe {
            // present
            let hr = self.swap_chain.Present(1, DXGI_PRESENT::default());

            if hr.is_err() {
                panic!("hotline_rs::gfx::d3d12: warning: present failed! {}", hr);
            }
            
            // signal fence
            let fv = self.fence_last_signalled_value + 1;
            device
                .command_queue
                .Signal(&self.fence, fv)
                .expect("hotline_rs::gfx::d3d12: warning: command_queue.Signal failed!");

            // update fence tracking
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.bb_index] = fv;
            self.require_wait[self.bb_index] = true;

            // swap buffers
            self.frame_index += 1;
            self.bb_index = (self.bb_index + 1) % self.num_bb as usize;
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

impl Drop for CmdBuf {
    fn drop(&mut self) {
        // care needs to be taken that all in flight frames have been completed before this drop is called
        // blocking with swap_chain.wait_for_last frame will ensure this is safe
        for bb in 0..self.in_flight_barriers.len() {
            self.drop_complete_in_flight_barriers(bb);
        }
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        let prev_bb = self.bb_index;
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        self.bb_index = bb;
        if swap_chain.frame_fence_value[bb] != 0 && self.needs_reset[bb] {
            unsafe {
                self.command_allocator[bb]
                    .Reset()
                    .expect("hotline_rs::gfx::d3d12: failed to reset command_allocator!");
                self.command_list[bb]
                    .Reset(&self.command_allocator[bb], None)
                    .expect("hotline_rs::gfx::d3d12: failed to reset command_list!");
            }
        }
        self.drop_complete_in_flight_barriers(prev_bb);
    }

    fn close(&mut self) -> result::Result<(), super::Error> {
        let bb = self.bb_index;
        unsafe {
            self.command_list[bb].Close().expect("hotline: d3d12 failed to close command list.");
            self.needs_reset[bb] = true;
        }
        if self.event_stack_count != 0 {
            Err(super::Error {
                msg: "mismatch begin/end events called on cmdbuf!".to_string()
            })
        }
        else {
            Ok(())
        }
    }   

    fn get_backbuffer_index(&self) -> u32 {
        self.bb_index as u32
    }

    fn begin_render_pass(&self, render_pass: &RenderPass) {
        unsafe {
            let cmd4: ID3D12GraphicsCommandList4 = self.cmd().cast().unwrap();
            cmd4.BeginRenderPass(
                Some(render_pass.rt.as_slice()), // TODO_WIN .. could be simplified
                if let Some(ds) = &render_pass.ds {
                    Some(ds) 
                } else {
                    None
                },
                D3D12_RENDER_PASS_FLAG_NONE,
            );
        }
    }

    fn end_render_pass(&self) {
        unsafe {
            let cmd4: ID3D12GraphicsCommandList4 = self.cmd().cast().unwrap();
            cmd4.EndRenderPass();
        }
    }

    fn begin_event(&mut self, colour: u32, name: &str) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().begin_event_on_command_list(cmd, colour as u64, name);
        }
        self.event_stack_count += 1;
    }

    fn end_event(&mut self) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().end_event_on_command_list(cmd);
        }
        assert!(self.event_stack_count > 0, "hotline::gfx::d3d12:: mismatched begin/end event");
        self.event_stack_count -= 1;
    }

    fn timestamp_query(&mut self, heap: &mut QueryHeap, resolve_buffer: &mut Buffer) {
        // alloc and insert the query
        let index = heap.allocate();
        unsafe { 
            let cmd = &self.command_list[self.bb_index];
            cmd.EndQuery(&heap.heap, D3D12_QUERY_TYPE_TIMESTAMP, index as u32);
            let cmd = &self.command_list[self.bb_index];
            cmd.ResolveQueryData(
                &heap.heap,
                D3D12_QUERY_TYPE_TIMESTAMP,
                index as u32, 1, 
                resolve_buffer.resource.as_ref().unwrap(), 
                0
            );
        }
    }

    fn begin_query(&mut self, heap: &mut QueryHeap, query_type: QueryType) -> usize {
        // alloc a new query
        let index = heap.allocate();
        unsafe {
            let cmd = &self.command_list[self.bb_index];
            cmd.BeginQuery(&heap.heap, to_d3d12_query_type(query_type), index as u32);
        }
        // return index for use in end
        index
    }

    fn end_query(&mut self, heap: &mut QueryHeap, query_type: QueryType, index: usize, resolve_buffer: &mut Buffer) {
        unsafe { 
            let cmd = &self.command_list[self.bb_index];
            cmd.EndQuery(&heap.heap, to_d3d12_query_type(query_type), index as u32);
            let cmd = &self.command_list[self.bb_index];
            cmd.ResolveQueryData(
                &heap.heap,
                to_d3d12_query_type(query_type),
                index as u32, 1, 
                resolve_buffer.resource.as_ref().unwrap(), 
                0
            );
        }
    }

    fn transition_barrier(&mut self, barrier: &TransitionBarrier<Device>) {
        let barrier = if let Some(tex) = &barrier.texture {
            transition_barrier(
                tex.resource.as_ref().unwrap(),
                to_d3d12_resource_state(barrier.state_before),
                to_d3d12_resource_state(barrier.state_after),
            )
        }
        else if let Some(buf) = &barrier.buffer {
            transition_barrier(
                buf.resource.as_ref().unwrap(),
                to_d3d12_resource_state(barrier.state_before),
                to_d3d12_resource_state(barrier.state_after),
            )
        }
        else {
            panic!("hotline::gfx::d3d12:: attempting to insert transition barrier with no attached resources");
        };
        unsafe {
            let bb = self.bb_index;
            self.command_list[bb].ResourceBarrier(&[barrier.clone()]);
            self.in_flight_barriers[bb].push(barrier);
        }
    }

    fn transition_barrier_subresource(&mut self, barrier: &TransitionBarrier<Device>, subresource: Subresource) {        
        if let Some(tex) = &barrier.texture {
            let res = match subresource {
                super::Subresource::Resource => tex.resource.as_ref().unwrap(),
                super::Subresource::ResolveResource => tex.resolved_resource.as_ref().unwrap()
            };
            let barrier = transition_barrier(
                res,
                to_d3d12_resource_state(barrier.state_before),
                to_d3d12_resource_state(barrier.state_after),
            );
            unsafe {
                let bb = self.bb_index;
                self.command_list[bb].ResourceBarrier(&[barrier.clone()]);
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
            self.cmd().RSSetViewports(&[d3d12_vp]);
        }
    }

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
        let d3d12_sr = RECT {
            left: scissor_rect.left,
            top: scissor_rect.top,
            right: scissor_rect.right,
            bottom: scissor_rect.bottom,
        };
        let cmd = &self.command_list[self.bb_index];
        unsafe {
            cmd.RSSetScissorRects(&[d3d12_sr]);
        }
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        let cmd = self.cmd();
        if buffer.vbv.is_some() {
            unsafe {
                cmd.IASetVertexBuffers(slot, Some(&[buffer.vbv.unwrap()]));
            }
        }
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
        let cmd = self.cmd();
        if buffer.ibv.is_some() {
            unsafe {
                cmd.IASetIndexBuffer(Some(&buffer.ibv.unwrap()));
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

    fn set_raytracing_pipeline(&self, pipeline: &RaytracingPipeline) {
        let cmd = self.cmd().cast::<ID3D12GraphicsCommandList4>().unwrap();
        unsafe {
            cmd.SetComputeRootSignature(&pipeline.root_signature);
            cmd.SetPipelineState1(&pipeline.state_object);
        }
    }

    fn set_heap<T: SuperPipleline>(&self, pipeline: &T, heap: &Heap) {
        unsafe {
            self.cmd().SetDescriptorHeaps(&[Some(heap.heap.clone())]);
            let slots = pipeline.get_pipeline_slots();
            match T::get_pipeline_type() {
                super::PipelineType::Render => {
                    for slot in slots {
                        self.cmd().SetGraphicsRootDescriptorTable(
                            *slot,
                            heap.heap.GetGPUDescriptorHandleForHeapStart(),
                        );
                    }
                },
                super::PipelineType::Compute => {
                    for slot in slots {
                        self.cmd().SetComputeRootDescriptorTable(
                            *slot,
                            heap.heap.GetGPUDescriptorHandleForHeapStart(),
                        );
                    }
                }

            }
        }
    }

    fn set_binding<T: SuperPipleline>(&self, _: &T, heap: &Heap, slot: u32, offset: usize) {
        unsafe {
            self.cmd().SetDescriptorHeaps(&[Some(heap.heap.clone())]);
            let mut base = heap.heap.GetGPUDescriptorHandleForHeapStart();
            base.ptr += (offset * heap.increment_size) as u64;

            match T::get_pipeline_type() {
                super::PipelineType::Render => {
                    self.cmd().SetGraphicsRootDescriptorTable(slot, base);
                },
                super::PipelineType::Compute => {
                    self.cmd().SetComputeRootDescriptorTable(slot, base);
                }
            }
        }
    }

    fn set_marker(&self, colour: u32, name: &str) {
        let cmd = &self.command_list[self.bb_index];
        if self.pix.is_some() {
            self.pix.unwrap().set_marker_on_command_list(cmd, colour as u64, name);
        }
    }

    fn push_render_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
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

    fn push_compute_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetComputeRoot32BitConstants(
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

    fn dispatch(&self, group_count: Size3, _numthreads: Size3) {
        unsafe {
            self.cmd().Dispatch(group_count.x, group_count.y, group_count.z);
        }
    }

    fn execute_indirect(
        &self, 
        command: &CommandSignature, 
        max_command_count: u32, 
        argument_buffer: &Buffer, 
        argument_buffer_offset: usize,
        counter_buffer: Option<&Buffer>,
        counter_buffer_offset: usize
    ) {
        let counter_buffer_resource = counter_buffer.map(|buf| buf.resource.as_ref().unwrap());
        unsafe {
            self.cmd().ExecuteIndirect(
                &command.command_signature, 
                max_command_count, 
                argument_buffer.resource.as_ref().unwrap(),
                argument_buffer_offset as u64, 
                counter_buffer_resource, 
                counter_buffer_offset as u64
            );
        }
    }

    fn dispatch_rays(&self, sbt: &RaytracingShaderBindingTable, numthreads: Size3) {
        /*
            // Bind the heaps, acceleration structure and dispatch rays.    
            commandList->SetComputeRootShaderResourceView(GlobalRootSignatureParams::AccelerationStructureSlot, m_topLevelAccelerationStructure->GetGPUVirtualAddress());
            
        */
        unsafe {
            let dispatch_desc = D3D12_DISPATCH_RAYS_DESC {
                RayGenerationShaderRecord: D3D12_GPU_VIRTUAL_ADDRESS_RANGE {
                    StartAddress: sbt.ray_generation.buffer.as_ref().map(|x| x.GetGPUVirtualAddress()).unwrap_or(0),
                    SizeInBytes: sbt.ray_generation.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                },
                MissShaderTable: D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE {
                    StartAddress: sbt.miss.buffer.as_ref().map(|x| x.GetGPUVirtualAddress()).unwrap_or(0),
                    SizeInBytes: sbt.miss.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                    StrideInBytes: sbt.miss.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                },
                HitGroupTable: D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE {
                    StartAddress: sbt.hit_group.buffer.as_ref().map(|x| x.GetGPUVirtualAddress()).unwrap_or(0),
                    SizeInBytes: sbt.hit_group.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                    StrideInBytes: sbt.hit_group.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                },
                CallableShaderTable: D3D12_GPU_VIRTUAL_ADDRESS_RANGE_AND_STRIDE {
                    StartAddress: sbt.callable.buffer.as_ref().map(|x| x.GetGPUVirtualAddress()).unwrap_or(0),
                    SizeInBytes: sbt.callable.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                    StrideInBytes: sbt.callable.buffer.as_ref().map(|x| x.GetDesc().Width ).unwrap_or(0),
                },
                Width: numthreads.x,
                Height: numthreads.y,
                Depth: numthreads.z,
            };

            let cmd = self.cmd().cast::<ID3D12GraphicsCommandList4>().unwrap();
            cmd.DispatchRays(&dispatch_desc);
        }
    }

    fn read_back_backbuffer(&mut self, swap_chain: &SwapChain) -> result::Result<ReadBackRequest, super::Error> {
        let bb = self.bb_index;
        let bbz = self.bb_index as u32;
        unsafe {
            let resource : ID3D12Resource = swap_chain.swap_chain.GetBuffer(bbz)?;

            // transition to copy source
            let barrier = transition_barrier(
                &resource,
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            );
            self.command_list[bb].ResourceBarrier(&[barrier.clone()]);
            self.in_flight_barriers[bb].push(barrier);

            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&resource),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };

            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: std::mem::transmute_copy(&swap_chain.readback_buffer),
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

            self.command_list[bb].CopyTextureRegion(&dst, 0, 0, 0, &src, None);

            let barrier = transition_barrier(
                &resource,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );

            // transition back to render target
            self.command_list[bb].ResourceBarrier(&[barrier.clone()]);
            self.in_flight_barriers[bb].push(barrier);

            Ok(ReadBackRequest {
                resource: Some(swap_chain.readback_buffer.clone().unwrap()),
                fence_value: swap_chain.frame_index as u64,
                size: (swap_chain.width * swap_chain.height * 4) as usize,
                row_pitch: (swap_chain.width * 4) as usize,
                slice_pitch: (swap_chain.width * swap_chain.height * 4) as usize,
            })
        }
    }

    fn resolve_texture_subresource(&self, texture: &Texture, subresource: u32) -> result::Result<(), super::Error> {
        unsafe {
            if let Some(resolve) = &texture.resolved_resource {
                self.cmd().ResolveSubresource(
                    &resolve.clone(),
                    subresource,
                    texture.resource.as_ref().unwrap(),
                    subresource,
                    texture.resolved_format
                 );
                 Ok(())
            }
            else {
                Err(super::Error {
                    msg: format!("hotline::gfx::d3d12:: failed to resolve texture subresource {}", subresource)
                })
            }
        }
    }


    fn generate_mip_maps(&mut self, texture: &Texture, device: &Device, heap: &Heap) -> result::Result<(), super::Error> {
        unsafe {
            // select between reslve resource and the main resource
            let desc = if let Some(resolved) = &texture.resolved_resource {
                resolved.GetDesc()
            }
            else {
                texture.resource.as_ref().unwrap().GetDesc()
            };
            
            if desc.MipLevels <= 1 || texture.subresource_uav_index.len() < desc.MipLevels as usize {
                Err(super::Error {
                    msg: "hotline_rs::d3d12:: texture was not created with GENERATE_MIP_MAPS_FLAG".to_string()
                })
            }
            else if let Some(pipeline) = &device.generate_mip_maps_pipeline {
                let bb = self.bb_index;

                self.set_compute_pipeline(pipeline);

                // bind heap
                self.set_heap(pipeline, heap);

                // transition to uav
                let barrier_write = if let Some(resolved) = &texture.resolved_resource {
                    transition_barrier(
                        resolved,
                        to_d3d12_resource_state(super::ResourceState::ShaderResource),
                        to_d3d12_resource_state(super::ResourceState::UnorderedAccess),
                    )
                }
                else {
                    transition_barrier(
                        texture.resource.as_ref().unwrap(),
                        to_d3d12_resource_state(super::ResourceState::ShaderResource),
                        to_d3d12_resource_state(super::ResourceState::UnorderedAccess),
                    )
                };

                // transition to shader_res
                let barrier_read = if let Some(resolved) = &texture.resolved_resource {
                    transition_barrier(
                        resolved,
                        to_d3d12_resource_state(super::ResourceState::UnorderedAccess),
                        to_d3d12_resource_state(super::ResourceState::ShaderResource),
                    )
                }
                else {
                    transition_barrier(
                        texture.resource.as_ref().unwrap(),
                        to_d3d12_resource_state(super::ResourceState::UnorderedAccess),
                        to_d3d12_resource_state(super::ResourceState::ShaderResource),
                    )
                };

                // multi-pass down sample
                let mut mipw = (desc.Width / 2).max(1);
                let mut miph = (desc.Height / 2).max(1);

                for i in 1..desc.MipLevels as usize {
                    // write barrier
                    let bw = barrier_write.clone();
                    self.command_list[bb].ResourceBarrier(&[bw.clone()]);
                    self.in_flight_barriers[bb].push(bw);

                    let using_slot = pipeline.get_pipeline_slot(0, 0, super::DescriptorType::PushConstants);
                    if let Some(slot) = using_slot {
                        self.push_compute_constants(slot.index, 1, 0, super::as_u8_slice(&texture.subresource_uav_index[i-1]));
                        self.push_compute_constants(slot.index, 1, 1, super::as_u8_slice(&texture.subresource_uav_index[i]));
                    }
                    
                    self.dispatch(
                        Size3 {
                            x: (mipw as f32 / 32.0).ceil() as u32,
                            y: (miph as f32 / 32.0).ceil() as u32,
                            z: 1
                        },
                        Size3 {
                            x: 32,
                            y: 32,
                            z: 1
                        }
                    );

                    mipw = (mipw / 2).max(1);
                    miph = (miph / 2).max(1);

                    // read barrier
                    let br = barrier_read.clone();
                    self.command_list[bb].ResourceBarrier(&[br.clone()]);
                    self.in_flight_barriers[bb].push(br);
                }

                Ok(())
            }
            else {
                Err(super::Error {
                    msg: "hotline_rs::d3d12:: unable to generate mip maps".to_string()
                })
            }
        }
    }

    fn copy_buffer_region(
        &mut self, 
        dst_buffer: &Buffer, 
        dst_offset: usize, 
        src_buffer: &Buffer, 
        src_offset: usize,
        num_bytes: usize
    ) {
        unsafe {
            self.cmd().CopyBufferRegion(
                dst_buffer.resource.as_ref().unwrap(), 
                dst_offset as u64,
                src_buffer.resource.as_ref().unwrap(), 
                src_offset  as u64,
                num_bytes as u64
            );
        }
    }

    fn copy_texture_region(
        &mut self,
        dst_texture: &Texture,
        subresource_index: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_texture: &Texture,
        src_region: Option<Region>
    ) {
        unsafe {
            let d3d12_box = if let Some(src_region) = src_region {
                D3D12_BOX {
                    left: src_region.left,
                    top: src_region.top,
                    front: src_region.front,
                    right: src_region.right,
                    bottom: src_region.bottom,
                    back: src_region.back
                }
            }
            else {
                D3D12_BOX {
                    ..Default::default()
                }
            };

            let option_box = if src_region.is_some() {
                Some(&d3d12_box as *const D3D12_BOX)
            }
            else {
                None
            };

            self.cmd().CopyTextureRegion(
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(dst_texture.resource.as_ref().unwrap()),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: subresource_index
                    }
                },
                dst_x,
                dst_y,
                dst_z,
                &D3D12_TEXTURE_COPY_LOCATION {
                    pResource: std::mem::transmute_copy(src_texture.resource.as_ref().unwrap()),
                    Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                    Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                        SubresourceIndex: subresource_index
                    }
                },
                option_box
            );
        }
    }
}

impl super::Buffer<Device> for Buffer {
    fn update<T: Sized>(&mut self, offset: usize, data: &[T]) -> result::Result<(), super::Error> {
        if !self.persistent_mapped_data.is_null() {
            Err(super::Error{
                msg: "hotline_rs::d3d12:: buffer was created with CpuAccessFlags::PERSISTENTLY_MAPPED use write to update it instead of update".to_string()
            })
        } 
        else {
            let update_bytes = data.len() * std::mem::size_of::<T>();
            let range = D3D12_RANGE { Begin: 0, End: 0 };
            let mut map_data = std::ptr::null_mut();
            unsafe {
                self.resource.as_ref().unwrap().Map(0, Some(&range), Some(&mut map_data))?;
                let dst = (map_data as *mut u8).add(offset);
                std::ptr::copy_nonoverlapping(data.as_ptr() as *mut _, dst, update_bytes);
                self.resource.as_ref().unwrap().Unmap(0, None);
            }
            Ok(())
        }
    }

    fn write<T: Sized>(&mut self, offset: usize, data: &[T]) -> result::Result<(), super::Error> {
        unsafe {
            if !self.persistent_mapped_data.is_null() {
                let update_bytes = data.len() * std::mem::size_of::<T>();
                let dst = (self.persistent_mapped_data as *mut u8).add(offset);
                std::ptr::copy_nonoverlapping(data.as_ptr() as *mut _, dst, update_bytes);
                Ok(())
            }
            else {
                Err(super::Error{
                    msg: "hotline_rs::d3d12:: buffer was not created with CpuAccessFlags::PERSISTENTLY_MAPPED".to_string()
                })
            }
        }
    }

    fn get_cbv_index(&self) -> Option<usize> {
        self.cbv_index
    }

    fn get_srv_index(&self) -> Option<usize> {
        self.srv_index
    }

    fn get_uav_index(&self) -> Option<usize> {
        self.uav_index
    }

    fn get_vbv(&self) -> Option<VertexBufferView> {
        self.vbv.map(|vbv| VertexBufferView { 
            location: vbv.BufferLocation, 
            size_bytes: vbv.SizeInBytes, 
            stride_bytes: vbv.StrideInBytes
        })
    }

    fn get_ibv(&self) -> Option<IndexBufferView> {
        self.ibv.map(|ibv| IndexBufferView { 
            location: ibv.BufferLocation, 
            size_bytes: ibv.SizeInBytes, 
            format: ibv.Format.0 as u32
        })
    }

    fn get_counter_offset(&self) -> Option<usize> {
        self.counter_offset
    }

    fn map(&mut self, info: &MapInfo) -> *mut u8 {
        if !self.persistent_mapped_data.is_null() {
            self.persistent_mapped_data as *mut u8
        } 
        else {
            let range = D3D12_RANGE {
                Begin: info.read_start,
                End: info.read_end,
            };
            let mut map_data = std::ptr::null_mut();
            unsafe {
                self.resource.as_ref().unwrap().Map(info.subresource, Some(&range), Some(&mut map_data)).unwrap();
            }
            map_data as *mut u8
        }
    }

    fn unmap(&mut self, info: &UnmapInfo) {
        if self.persistent_mapped_data.is_null() {
            let range = D3D12_RANGE {
                Begin: info.write_start,
                End: info.write_end,
            };
            unsafe {
                self.resource.as_ref().unwrap().Unmap(info.subresource, Some(&range));
            }
        }
    }
}

// public accessors for texture
pub fn get_texture_shared_handle(tex: &Texture) -> &Option<HANDLE> {
    &tex.shared_handle
}

impl super::Texture<Device> for Texture {
    fn get_srv_index(&self) -> Option<usize> {
        if self.resolved_srv_index.is_some() {
            self.resolved_srv_index
        }
        else {
            self.srv_index
        }
    }

    fn get_subresource_uav_index(&self, subresource: u32) -> Option<usize> {
        if subresource < self.subresource_uav_index.len() as u32 {
            Some(self.subresource_uav_index[subresource as usize])
        }
        else {
            None
        }
    }

    fn get_msaa_srv_index(&self) -> Option<usize> {
        self.srv_index
    }

    fn get_uav_index(&self) -> Option<usize> {
        self.uav_index
    }

    fn clone_inner(&self) -> Texture {
        self.clone()
    }

    fn is_resolvable(&self) -> bool {
        self.resolved_resource.is_some()
    }

    fn get_shader_heap_id(&self) -> Option<u16> {
        self.shader_heap_id
    }
}

impl super::Pipeline for RenderPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        let h = get_binding_descriptor_hash(register, space, descriptor_type);
        self.lookup.slot_lookup.get(&h)
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.lookup.descriptor_slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Render
    }
}

impl super::Pipeline for ComputePipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        let h = get_binding_descriptor_hash(register, space, descriptor_type);
        self.lookup.slot_lookup.get(&h)
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.lookup.descriptor_slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Compute
    }
}

impl super::Pipeline for RaytracingPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        let h = get_binding_descriptor_hash(register, space, descriptor_type);
        self.lookup.slot_lookup.get(&h)
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.lookup.descriptor_slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Compute
    }
}

impl super::ReadBackRequest<Device> for ReadBackRequest {
    fn is_complete(&self, swap_chain: &SwapChain) -> bool {
        if swap_chain.frame_index as u64 > self.fence_value + 1 {
            return true;
        }
        false
    }

    fn map(&self, info: &MapInfo) -> result::Result<ReadBackData, super::Error> {
        let range = D3D12_RANGE {
            Begin: info.read_start,
            End: if info.read_end == usize::MAX {
                self.size
            } else {
                info.read_end
            },
        };
        let mut map_data = std::ptr::null_mut();
        unsafe {
            if let Some(res) = &self.resource {
                res.Map(0, Some(&range), Some(&mut map_data))?;
                if !map_data.is_null() {
                    let slice = std::slice::from_raw_parts(map_data as *const u8, self.size);
                    let rb_data = super::ReadBackData {
                        data: slice,
                        size: self.size,
                        format: super::Format::Unknown,
                        row_pitch: self.row_pitch,
                        slice_pitch: self.size,
                    };
                    return Ok(rb_data);
                }
            }
            Err(super::Error {
                msg: "Failed to map readback buffer".to_string(),
            })
        }
    }

    fn unmap(&self) {
        unsafe {
            if let Some(res) = &self.resource {
                res.Unmap(0, None);
            }
        }
    }
}

impl From<os::win32::NativeHandle> for HWND {
    fn from(handle: os::win32::NativeHandle) -> HWND {
        unsafe { std::mem::transmute(handle.hwnd) }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        if MANAGE_DROPS {
            // only grab resources if we have a drop list, this allows the swap chain rtv
            // to manage itself
            let mut res_vec = if self.drop_list.is_some() {
                // swap out the resources for null
                let mut res = None;
                std::mem::swap(&mut res, &mut self.resource);
                let mut res_vec = vec![
                    res.unwrap()
                ];
                if self.resolved_resource.is_some() {
                    let mut res = None;
                    std::mem::swap(&mut res, &mut self.resolved_resource);
                    res_vec.push(res.unwrap());
                }
                res_vec
            }
            else {
                Vec::new()
            };
            // texture resource views
            if let Some(drop_list) = &self.drop_list {
                let mut drop_list = drop_list.list.lock().unwrap();
                let mut drop_res = DropResource {
                    resources: res_vec.to_vec(),
                    frame: 0,
                    heap_allocs: Vec::new()
                };
                res_vec.clear();
                if let Some(srv_index) = self.srv_index {
                    drop_res.heap_allocs.push(srv_index);
                }
                if let Some(uav_index) = self.uav_index {
                    drop_res.heap_allocs.push(uav_index);
                }
                if let Some(resolved_srv) = self.resolved_srv_index {
                    drop_res.heap_allocs.push(resolved_srv);
                }
                // mip chain uav
                for uav in &self.subresource_uav_index {
                    drop_res.heap_allocs.push(*uav);
                }
                self.subresource_uav_index.clear();
                drop_list.push(drop_res);
            }
            // texture target views
            for rtv in &self.rtv {
                let mut drop_list = rtv.drop_list.list.lock().unwrap();
                let drop_res = DropResource {
                    resources: res_vec.to_vec(),
                    frame: 0,
                    heap_allocs: vec![rtv.index]
                };
                drop_list.push(drop_res);
                res_vec.clear();
            }
            for dsv in &self.dsv {
                let mut drop_list = dsv.drop_list.list.lock().unwrap();
                let drop_res = DropResource {
                    resources: res_vec.to_vec(),
                    frame: 0,
                    heap_allocs: vec![dsv.index]
                };
                drop_list.push(drop_res);
                res_vec.clear();
            }
        }

    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if MANAGE_DROPS {
            // swap res with null
            let mut res = None;
            std::mem::swap(&mut res, &mut self.resource);
            // build list of heap allocs and resources to drop later
            if let Some(drop_list) = &self.drop_list {
                let mut drop_list = drop_list.list.lock().unwrap();
                let mut drop_res = DropResource {
                    resources: vec![res.unwrap()],
                    frame: 0,
                    heap_allocs: Vec::new()
                };
                if let Some(srv_index) = self.srv_index {
                    drop_res.heap_allocs.push(srv_index);
                }
                if let Some(uav_index) = self.uav_index {
                    drop_res.heap_allocs.push(uav_index);
                }
                if let Some(cbv_index) = self.cbv_index {
                    drop_res.heap_allocs.push(cbv_index);
                }
                drop_list.push(drop_res);
            }
        }
    }
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}
unsafe impl Send for SwapChain {}
unsafe impl Sync for SwapChain {}
unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}
unsafe impl Send for RenderPipeline {}
unsafe impl Sync for RenderPipeline {}
unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}
unsafe impl Send for RaytracingPipeline {}
unsafe impl Sync for RaytracingPipeline {}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}
unsafe impl Send for CmdBuf {}
unsafe impl Sync for CmdBuf {}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}
unsafe impl Send for Heap {}
unsafe impl Sync for Heap {}
unsafe impl Send for QueryHeap {}
unsafe impl Sync for QueryHeap {}
unsafe impl Send for CommandSignature {}
unsafe impl Sync for CommandSignature {}

impl super::Shader<Device> for Shader {}
impl super::RenderPipeline<Device> for RenderPipeline {}
impl super::ComputePipeline<Device> for ComputePipeline {}
impl super::RaytracingPipeline<Device> for RaytracingPipeline {}
impl super::CommandSignature<Device> for CommandSignature {}
impl super::RaytracingShaderBindingTable<Device> for RaytracingShaderBindingTable {}
impl super::RaytracingBLAS<Device> for RaytracingBLAS {}
impl super::RaytracingTLAS<Device> for RaytracingTLAS {}