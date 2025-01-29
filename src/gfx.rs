use crate::os;
use std::any::Any;
use serde::{Deserialize, Serialize};
use std::hash::Hash;
use maths_rs::max;

/// Implemets this interface with a Direct3D12 backend.
#[cfg(target_os = "windows")]
pub mod d3d12;

type Error = super::Error;

/// Macro to pass data!\[expression\] or data!\[\] (None) to a create function, so you don't have to deduce a 'T'.
#[macro_export]
macro_rules! data {
    () => {
        None::<&[()]>
    };
    ($input:expr) => {
        Some($input)
    }
}

/// Macro to inject debug names into gpu resources
#[cfg(target_os = "windows")]
#[macro_export]
macro_rules! gfx_debug_name {
    ($object:expr, $name:expr) => {
        d3d12_debug_name($object, $name);
    }
}

/// 3-Dimensional struct for compute shader thread count / thread group size.
#[derive(Copy, Clone)]
pub struct Size3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

/// 3-Dimensional region used for copying resources
#[derive(Copy, Clone)]
pub struct Region {
    pub left: u32,
    pub top: u32,
    pub front: u32,
    pub right: u32,
    pub bottom: u32,
    pub back: u32
}

/// Structure to specify viewport coordinates on a `CmdBuf`.
#[derive(Copy, Clone)]
pub struct Viewport {
    /// Top left x coordinate.
    pub x: f32,
    /// Top left y coordinate.
    pub y: f32,
    /// Width of the viewport rectangle.
    pub width: f32,
    /// Height of the viewport rectangle (Y is down).
    pub height: f32,
    /// Minimum depth of the viewport. Ranges between 0 and 1.
    pub min_depth: f32,
    /// Maximum depth of the viewport. Ranges between 0 and 1.
    pub max_depth: f32,
}

/// Structure to specify scissor rect coordinates on a `CmdBuf`.
#[derive(Copy, Clone)]
pub struct ScissorRect {
    // Left x coordinate.
    pub left: i32,
    // Top y coordinate.
    pub top: i32,
    /// Right x coordinate.
    pub right: i32,
    /// Bottom y coordinate.
    pub bottom: i32,
}

/// Format for resource types (textures / buffers).
/// n = normalised unsigned integer,
/// u = unsigned integer,
/// i = signed integer,
/// f = float
#[derive(Copy, Clone, Serialize, Deserialize, Hash, PartialEq)]
pub enum Format {
    Unknown,
    R16n,
    R16u,
    R16i,
    R16f,
    R32u,
    R32i,
    R32f,
    RG16f,
    RG16u,
    RG16i,
    RG32u,
    RG32i,
    RG32f,
    RGB32u,
    RGB32i,
    RGB32f,
    RGBA8nSRGB,
    RGBA8n,
    RGBA8u,
    RGBA8i,
    BGRA8n,
    BGRX8n,
    BGRA8nSRGB,
    BGRX8nSRGB,
    RGBA16u,
    RGBA16i,
    RGBA16f,
    RGBA32u,
    RGBA32i,
    RGBA32f,
    D32fS8X24u,
    D32f,
    D24nS8u,
    D16n,
    BC1n,
    BC1nSRGB,
    BC2n,
    BC2nSRGB,
    BC3n,
    BC3nSRGB,
    BC4n,
    BC5n,
}

/// Information to create a device, it contains default heaps for resource views
/// resources will be automatically allocated into these heaps, you can supply custom heaps if necessary.
#[derive(Default)]
pub struct DeviceInfo {
    /// optional adapter to choose a specific adapter in the scenario of a multi-adapter system
    /// if None is supplied the first non-software emulation adapter would be selected.
    pub adapter_name: Option<String>,
    /// space for shader resource views, constant buffers and unordered access views.
    pub shader_heap_size: usize,
    /// space for colour render targets.
    pub render_target_heap_size: usize,
    /// space for depth stencil targets.
    pub depth_stencil_heap_size: usize,
}

/// Information returned from `Device::get_adapter_info`.
#[derive(Clone)]
pub struct AdapterInfo {
    /// The chosen adapter a device was created with.
    pub name: String,
    /// Description of the device.
    pub description: String,
    /// Dedicated video memory in bytes.
    pub dedicated_video_memory: usize,
    /// Dedicated system memory in bytes.
    pub dedicated_system_memory: usize,
    /// Shared system memory in bytes.
    pub shared_system_memory: usize,
    /// List of available adapter descriptons.
    pub available: Vec<String>,
}

/// Information to create a desciptor heap... `Device` will contain default heaps, but you can create your own if required.
pub struct HeapInfo {
    /// ie: Shader, RenderTarget, DepthStencil, Sampler.
    pub heap_type: HeapType,
    /// Total size of the heap in number of resources.
    pub num_descriptors: usize,
}

/// Options for heap types.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum HeapType {
    /// For shader resource view, constant buffer or unordered access.
    Shader,
    /// For render targets
    RenderTarget,
    /// For depth stencil targets
    DepthStencil,
    /// For sampler states
    Sampler,
}

/// Allows user specified heaps to be used for creating views when creating textures through `create_texture_with_heap`
/// you can supply `None` for the heap types are not applicable and if a view is requested for a `None` heap the
/// default device heaps will be used instead
pub struct TextureHeapInfo<'stack, D: Device> {
    /// Heap to allocate shader resource views and un-ordered access views
    pub shader: Option<&'stack mut D::Heap>,
    /// Heap to allocate render target views
    pub render_target: Option<&'stack mut D::Heap>,
    /// Heap to allocate depth stencil views
    pub depth_stencil: Option<&'stack mut D::Heap>,
}

/// Information to create a query heap.
pub struct QueryHeapInfo {
    /// ie: Timestamp, Occlusion, PipelineStatistics
    pub heap_type: QueryType,
    /// Total size of the heap in number of queries.
    pub num_queries: usize,
}

/// Options for query heap types, and queries
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum QueryType {
    /// Used for occlusion query heap or occlusion queries
    Occlusion,
    /// Can be used in the same heap as occlusion
    BinaryOcclusion,
    /// Create a heap to contain timestamp queries
    Timestamp,
    /// Create a heap to contain a structure of `PipelineStatistics`
    PipelineStatistics,
    /// Create video decoder statistics query and heap
    VideoDecodeStatistics,
}

/// GPU pipeline statistics obtain by using a `PipelineStatistics` query
pub struct PipelineStatistics {
    pub input_assembler_vertices: u64,
    pub input_assembler_primitives: u64,
    pub vertex_shader_invocations: u64,
    pub pixel_shader_primitives: u64,
    pub compute_shader_invocations: u64
}

/// Information to pass to `Device::create_swap_chain`.
pub struct SwapChainInfo {
    /// Number of internal buffers to keep behind the scenes, which are swapped between each frame 
    /// to allow overlapped CPU/GPU command buffer producer / consumer
    pub num_buffers: u32,
    /// Must be BGRA8n, RGBA8n or RGBA16f.
    pub format: Format,
    /// Colour for clearing the window when using the backbuffer pass, use None to not clear.
    pub clear_colour: Option<ClearColour>,
}

/// Information to create a buffer through `Device::create_buffer`.
#[derive(Copy, Clone)]
pub struct BufferInfo {
    /// Indicates how the buffer will be used on the GPU.
    pub usage: BufferUsage,
    /// Used to indicate if we want to read or write from the CPU, use NONE if possible for best performance.
    pub cpu_access: CpuAccessFlags,
    /// Data format of the buffer this is is only required for index buffers and can be `gfx::Format::Unknown` otherwise
    pub format: Format,
    /// The stride of a vertex or structure in bytes.
    pub stride: usize,
    /// The number of array elements.
    pub num_elements: usize,
    /// Initial state to start image transition barriers before state
    pub initial_state: ResourceState,
}

/// Information to create a shader through `Device::create_shader`.
pub struct ShaderInfo {
    /// Type of the shader (Vertex, Fragment, Compute, etc...).
    pub shader_type: ShaderType,
    /// Optional info to compile from source, if this is none then
    /// the shader will be treated as a precompiled byte code blob.
    pub compile_info: Option<ShaderCompileInfo>,
}

/// Information required to compile a shader from source code.
pub struct ShaderCompileInfo {
    /// The name of the entry point function in the shader to compile.
    pub entry_point: String,
    /// The target you wish to compile for, this is paltform specific.
    /// hlsl: (vs_5_0, ps_5_0, vs_6_0, ps_6_0).
    pub target: String,
    /// Flags to pass to the compiler.
    pub flags: ShaderCompileFlags,
}

/// The stage to which a shader will bind itself.
#[derive(Copy, Clone)]
pub enum ShaderType {
    Vertex,
    Fragment,
    Compute,
    RayGen,
    AnyHit,
    ClosestHit,
    Miss,
    Intersection,
    Callable
}

bitflags! {
    /// Device feature flags.
    pub struct DeviceFeatureFlags: u32 {
        const NONE = 0;
        const RAYTRACING = 1<<0;
        const MESH_SAHDER = 1<<1;
    }

    /// Shader compilation flags.
    pub struct ShaderCompileFlags: u32 {
        /// No flags, default compilation.
        const NONE = 0b00000000;
        /// Generates shader with debug info
        const DEBUG = 0b00000001;
        /// Skips optimization for easier debuggability, deterministic results and faster compilation.
        const SKIP_OPTIMIZATION = 0b00000010;
    }

    /// Render target write mask flags.
    #[derive(Serialize, Deserialize)]
    pub struct WriteMask : u8 {
        // Write no colour channels
        const NONE = 0;
        /// Write the red colour channel
        const RED = 1<<0;
        /// Write the green colour channel
        const GREEN = 1<<1;
        /// Write the blue colour channel
        const BLUE = 1<<2;
        /// Write the alpha channel
        const ALPHA = 1<<3;
        /// Write (RED|GREEN|BLUE|ALPHA)
        const ALL = (1<<4)-1;
    }

    /// CPU Access flags for buffers or textures.
    pub struct CpuAccessFlags: u8 {
        /// No CPUT access required, use this for best performance if you do not need to write data to a resource
        const NONE = 1<<0;
        /// CPU will read data from the resource
        const READ = 1<<1;
        /// CPU will write data to the resourc
        const WRITE = 1<<2;
        /// Must be used in conjunction with READ or WRITE, the resource will mapped once and never un-mapped
        const PERSISTENTLY_MAPPED = 1<<3;
    }

    /// Textures can be used in one or more of the following ways
    #[derive(Serialize, Deserialize)]
    pub struct TextureUsage: u32 {
        /// Texture will be only used for data storage and not used on any GPU pipeline stages
        const NONE = 0;
        /// Texture will be sampled in a shader
        const SHADER_RESOURCE = (1 << 0);
        /// Used as a read-writable resource in compute shaders
        const UNORDERED_ACCESS = (1 << 1);
        /// Used as a colour render target
        const RENDER_TARGET = (1 << 2);
        /// Used as a depth stencil buffer
        const DEPTH_STENCIL = (1 << 3);
        /// Used as a target for hardware assisted video decoding operations
        const VIDEO_DECODE_TARGET = (1 << 4);
        /// Indicates the texture will have mip-maps generated at run time
        const GENERATE_MIP_MAPS = (1 << 5);
    }

    /// Describes how a buffer will be used on the GPU.
    //#[derive(Copy, Clone, PartialEq)]
    pub struct BufferUsage : u32 {
        /// Used to simply store data (query results, copy buffers etc)
        const NONE = 0;
        /// Used as a Vertex buffer binding
        const VERTEX = (1 << 0);
        /// Used as a Vertex buffer binding
        const INDEX = (1 << 1);
        /// Used as constant buffer for shader data
        const CONSTANT_BUFFER = (1 << 2);
        /// Texture will be sampled in a shader
        const SHADER_RESOURCE = (1 << 3);
        /// Used as a read-writable resource in compute shaders
        const UNORDERED_ACCESS = (1 << 4);
        /// Used as indirect arguments for `execute_indirect`
        const INDIRECT_ARGUMENT_BUFFER = (1 << 5);
        /// Used in shader as `AppendStructuredBuffer` and contains a counter element
        const APPEND_COUNTER = (1 << 6);
        /// Upload only buffer, can be used for acceleration structure geometry or to copy data
        const UPLOAD = (1 << 7);
        /// Only create the buffer and no views
        const BUFFER_ONLY = (1 << 8);
    }

    /// Flags for raytracing geometry
    pub struct RaytracingGeometryFlags : u8 {
        /// No flags
        const NONE = 0;
        /// Specifies the implementation must only call the any-hit shader a single time for each primitive in this geometry
        const NO_DUPLICATE_ANY_HIT = (1 << 0);
        /// Opque Geometry specifies no anyhit shader is called
        const OPAQUE = (1<<1);
    }

    // Flags for building ray tracing acelleration structures
    pub struct AccelerationStructureBuildFlags : u8 {
        const NONE = 0;
        const ALLOW_COMPACTION = (1<<1);
        const ALLOW_UPDATE = (1<<2);
        const MINIMIZE_MEMORY = (1<<3);
        const PERFORM_UPDATE = (1<<4);
        const PREFER_FAST_BUILD = (1<<5);
        const PREFER_FAST_TRACE = (1<<6);
    }
}

/// `PipelineLayout` is required to create a pipeline it describes the layout of resources for access on the GPU.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PipelineLayout {
    /// Vector of `DescriptorBinding` which are arrays of textures, samplers or structured buffers, etc
    pub bindings: Option<Vec<DescriptorBinding>>,
    /// Small amounts of data that can be pushed into a command buffer and available as data in shaders
    pub push_constants: Option<Vec<PushConstantInfo>>,
    /// Static samplers that come along with the pipeline, 
    pub static_samplers: Option<Vec<SamplerBinding>>,
}

/// Describes a range of resources for access on the GPU.
#[derive(Clone, Serialize, Deserialize)]
pub struct DescriptorBinding {
    /// The shader stage the resources will be accessible to.
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader).
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader).
    pub register_space: u32,
    /// Type of resources in this descriptor binding.
    pub binding_type: DescriptorType,
    /// Number of descriptors in this table, use `None` for unbounded.
    pub num_descriptors: Option<u32>,
}

/// Describes the type of descriptor binding to create.
#[derive(Clone, Copy, Serialize, Deserialize, Hash)]
pub enum DescriptorType {
    /// Used for textures or structured buffers.
    ShaderResource,
    /// Used for cbuffers.
    ConstantBuffer,
    /// Used for read-write textures.
    UnorderedAccess,
    /// Used for texture samplers.
    Sampler,
    /// Used for push constants
    PushConstants
}

/// Describes the visibility of which shader stages can access a descriptor.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShaderVisibility {
    #[default]
    All,
    Vertex,
    Fragment,
    Compute,
}

/// Describes space in the shader to send data to via `CmdBuf::push_constants`. 
#[derive(Clone, Serialize, Deserialize)]
pub struct PushConstantInfo {
    /// The shader stage the constants will be accessible to.
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader).
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader).
    pub register_space: u32,
    /// Number of 32-bit values to push.
    pub num_values: u32,
}

/// You can request this based on resource type, register and space (as specified in shader)
#[derive(Clone)]
pub struct PipelineSlotInfo {
    /// The slot in the pipeline layout to bind to
    pub index: u32,
    /// The number of descriptors or the number of 32-bit push constant values, if `None` the table is unbounded
    pub count: Option<u32>
}

/// Input layout describes the layout of vertex buffers bound to the input assembler.
pub type InputLayout = Vec<InputElementInfo>;

/// Describe a single element of an `InputLayoutInfo`.
#[derive(Clone, Serialize, Deserialize)]
pub struct InputElementInfo {
    /// Element semantic ie. POSITION, TEXCOORD, COLOR etc.
    pub semantic: String,
    /// Index of the semantic ie. TEXCOORD0, TEXCOORD1 etc.
    pub index: u32,
    /// Format of the element size and width.
    pub format: Format,
    /// The vertex buffer slot this buffer will be bound to.
    pub input_slot: u32,
    /// Aligned byte offset of this element from the start of the struct.
    pub aligned_byte_offset: u32,
    /// Vertex or Instance stride.
    pub input_slot_class: InputSlotClass,
    /// Rate at which to step vertices.
    pub step_rate: u32,
}

/// Describes the frequency of which elements are fetched from a vertex input element.
#[derive(Clone, Serialize, Deserialize)]
pub enum InputSlotClass {
    PerVertex,
    PerInstance,
}

/// Individual sampler state binding for use in static samplers in a `PipelineLayout`.
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct SamplerBinding {
    /// The shader stage the sampler will be accessible to
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader)
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader)
    pub register_space: u32,
    /// Sampler Info
    pub sampler_info: SamplerInfo
}

/// Info to create a sampler state object to sample textures in shaders.
#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct SamplerInfo {
    pub filter: SamplerFilter,
    pub address_u: SamplerAddressMode,
    pub address_v: SamplerAddressMode,
    pub address_w: SamplerAddressMode,
    pub comparison: Option<ComparisonFunc>,
    /// Colour is rgba8 packed into a u32
    pub border_colour: Option<u32>,
    pub mip_lod_bias: f32,
    pub max_aniso: u32,
    pub min_lod: f32,
    pub max_lod: f32,
}

/// Filtering mode for the sampler (controls bilinear and trilinear interpolation).
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum SamplerFilter {
    Point,
    Linear,
    Anisotropic,
}

/// Address mode for the sampler (controls wrapping and clamping).
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum SamplerAddressMode {
    Wrap,
    Mirror,
    Clamp,
    Border,
    MirrorOnce,
}

/// Used for comparison ops in depth testing, samplers.
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum ComparisonFunc {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Information to create a pipeline through `Device::create_render_pipeline`.
pub struct RenderPipelineInfo<'stack, D: Device> {
    /// Vertex Shader
    pub vs: Option<&'stack D::Shader>,
    /// Fragment Shader
    pub fs: Option<&'stack D::Shader>,
    /// Vertex shader input layout
    pub input_layout: InputLayout,
    /// Layout of shader resources (constant buffers, structured buffers, textures, etc)
    pub pipeline_layout: PipelineLayout,
    /// Control rasterisation of primitives
    pub raster_info: RasterInfo,
    /// Control depth test and stencil oprations
    pub depth_stencil_info: DepthStencilInfo,
    /// Control blending settings for the output merge stage
    pub blend_info: BlendInfo,
    /// Primitive topolgy oof the input assembler
    pub topology: Topology,
    /// Only required for Topology::PatchList use 0 as default
    pub patch_index: u32,
    /// Sample mask for which MSAA samples to write
    pub sample_mask: u32,
    /// A valid render pass, you can share pipelines across passes providing the render target
    /// formats and sample count are the same of the passes you wish to use the pipeline on
    pub pass: Option<&'stack D::RenderPass>,
}

/// Indicates how the pipeline interprets vertex data at the input assembler stage
/// This will be also used to infer primitive topology types for geometry or hull shaders
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Topology {
    Undefined,
    PointList,
    LineList,
    LineStrip,
    TriangleList,
    TriangleStrip,
    LineListAdj,
    LineStripAdj,
    TriangleListAdj,
    TriangleStripAdj,
    PatchList,
}

/// Information to control the rasterisation mode of primitives when using a `RenderPipeline`
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct RasterInfo {
    pub fill_mode: FillMode,
    pub cull_mode: CullMode,
    pub front_ccw: bool,
    pub depth_bias: i32,
    pub depth_bias_clamp: f32,
    pub slope_scaled_depth_bias: f32,
    pub depth_clip_enable: bool,
    pub multisample_enable: bool,
    pub antialiased_line_enable: bool,
    pub forced_sample_count: u32,
    pub conservative_raster_mode: bool,
}

/// Polygon fillmode
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum FillMode {
    Wireframe,
    Solid,
}

/// Polygon cull mode
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// Information to control the depth and stencil testing of primitves when using a `RenderPipeline`
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct DepthStencilInfo {
    /// Enable depth testing
    pub depth_enabled: bool,
    /// Choose to write or not write to the depth buffer
    pub depth_write_mask: DepthWriteMask,
    pub depth_func: ComparisonFunc,
    /// Enable stencil testing
    pub stencil_enabled: bool,
    pub stencil_read_mask: u8,
    pub stencil_write_mask: u8,
    pub front_face: StencilInfo,
    pub back_face: StencilInfo,
}

/// Write to the depth buffer, or omit writes and just perform depth testing
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum DepthWriteMask {
    Zero,
    All,
}

/// Stencil info for various outcomes of the depth stencil test
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct StencilInfo {
    pub fail: StencilOp,
    pub depth_fail: StencilOp,
    pub pass: StencilOp,
    pub func: ComparisonFunc,
}

/// Stencil operations
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum StencilOp {
    Keep,
    Zero,
    Replace,
    IncrSat,
    DecrSat,
    Invert,
    Incr,
    Decr,
}

/// Information to control blending operations on render targets
#[derive(Default)]
pub struct BlendInfo {
    pub alpha_to_coverage_enabled: bool,
    /// Separate blending on colour and alpha channels
    pub independent_blend_enabled: bool,
    /// Separate blend operations for each bout render targets
    pub render_target: Vec<RenderTargetBlendInfo>,
}

/// Blending operations for a single render target
#[derive(Clone, Serialize, Deserialize)]
pub struct RenderTargetBlendInfo {
    pub blend_enabled: bool,
    pub logic_op_enabled: bool,
    pub src_blend: BlendFactor,
    pub dst_blend: BlendFactor,
    pub blend_op: BlendOp,
    pub src_blend_alpha: BlendFactor,
    pub dst_blend_alpha: BlendFactor,
    pub blend_op_alpha: BlendOp,
    pub logic_op: LogicOp,
    pub write_mask: WriteMask,
}

/// Controls how the source and destination terms in blend equation are derrived
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum BlendFactor {
    Zero,
    One,
    SrcColour,
    InvSrcColour,
    SrcAlpha,
    InvSrcAlpha,
    DstAlpha,
    InvDstAlpha,
    DstColour,
    InvDstColour,
    SrcAlphaSat,
    BlendFactor,
    InvBlendFactor,
    Src1Colour,
    InvSrc1Colour,
    Src1Alpha,
    InvSrc1Alpha,
}

/// Controls how the source and destination terms are combined: final = src (op) dest
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum BlendOp {
    Add,
    Subtract,
    RevSubtract,
    Min,
    Max,
}

/// The logical operation to configure for a render target blend with logic op enabled
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum LogicOp {
    Clear,
    Set,
    Copy,
    CopyInverted,
    NoOp,
    Invert,
    And,
    Nand,
    Or,
    Nor,
    Xor,
    Equiv,
    AndReverse,
    AndInverted,
    OrReverse,
    OrInverted,
}

/// Information to create a compute pipeline through `Device::create_compute_pipeline`
pub struct ComputePipelineInfo<'stack, D: Device> {
    /// Compute Shader
    pub cs: &'stack D::Shader,
    /// Describe the layout of resources we bind on the pipeline
    pub pipeline_layout: PipelineLayout,
}

pub struct RaytracingShader<'stack, D: Device> {
    /// Reference to shader
    pub shader: &'stack D::Shader,
    /// Entry point name within shader
    pub entry_point: String
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum RaytracingHitGeometry {
    Triangles,
    ProceduralPrimitive
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RaytracingHitGroup {
    pub name: String,
    pub any_hit: Option<String>,
    pub closest_hit: Option<String>,
    pub intersection: Option<String>,
    pub geometry: RaytracingHitGeometry
}

/// Information to create a raytracing pipeline through `Device::create_raytracing_pipeline`
pub struct RaytracingPipelineInfo<'stack, D: Device> {
    pub shaders: Vec<RaytracingShader<'stack, D>>,
    pub hit_groups: Option<Vec<RaytracingHitGroup>>,
    pub pipeline_layout: PipelineLayout,
}

/// Information to create a raytracing shader binding table from a raytracing pipeline, for use with `Device::dispatch_rays`
pub struct RaytracingShaderBindingTableInfo<'stack, D: Device> {
    /// The entry point name of the ray generation shader within the pipeline
    pub ray_generation_shader: String,
    /// The entry point names of the miss shaders to use in the dispatch rays
    pub miss_shaders: Vec<String>,
    /// The entry point names of the callable shaders to use in the dispatch rays
    pub callable_shaders: Vec<String>,
    /// The names of the hit groups to use in the dispatch rays
    pub hit_groups: Vec<String>,
    // The raytracing pipeline to bind shaders from within
    pub pipeline: &'stack D::RaytracingPipeline
}

/// Information to create a raytracing bottom level acceleration structure from traingle geometry index and vertex buffers
pub struct RaytracingTrianglesInfo<'stack, D: Device> {
    pub index_buffer: &'stack D::Buffer,
    pub vertex_buffer: &'stack D::Buffer,
    pub transform3x4: Option<&'stack D::Buffer>,
    pub index_count: usize,
    pub vertex_count: usize,
    pub index_format: Format,
    pub vertex_format: Format,
}

/// Information to create a raytracing acceleration structure from aabbs
pub struct RaytracingAABBsInfo<'stack, D: Device> {
    pub aabbs: Option<&'stack D::Buffer>,
    pub aabb_count: usize,
}

/// Information to specify geometry for a raytracing bottom level acceleration structure
pub enum RaytracingGeometryInfo<'stack, D: Device> {
    Triangles(RaytracingTrianglesInfo<'stack, D>),
    AABBs(RaytracingAABBsInfo<'stack, D>)
}

/// Information to create a top level raytracing acceleration structure
pub struct RaytracingInstanceInfo<'stack, D: Device> {
    /// A 3x4 transform matrix in row-major layout
    pub transform: [f32; 12],
    pub instance_id: u32,
    pub instance_mask: u32,
    pub hit_group_index: u32,
    pub instance_flags: u32,
    pub blas: &'stack D::RaytracingBLAS
}

pub struct RaytracingBLASInfo<'stack, D: Device> {
    pub geometry: RaytracingGeometryInfo<'stack, D>,
    pub geometry_flags: RaytracingGeometryFlags,
    pub build_flags: AccelerationStructureBuildFlags,
}

pub struct RaytracingTLASInfo<'stack, D: Device> {
    pub instances: &'stack Vec<RaytracingInstanceInfo<'stack, D>>,
    pub build_flags: AccelerationStructureBuildFlags,
}

/// Information to create a pipeline through `Device::create_texture`.
#[derive(Copy, Clone)]
pub struct TextureInfo {
    /// Texture type
    pub tex_type: TextureType,
    /// Texture format
    pub format: Format,
    /// Width of the image in texels
    pub width: u64,
    /// Height of the image in texels for `TextureType::Texture2D` and `Texture3D` use 1 for `Texture1D`
    pub height: u64,
    /// Depth of the image in slices of (`width` x `height`) for `TextureType::Texture3D` only (use 1 other wise)
    pub depth: u32,
    /// Number of array levels or slices for `Texture1D` or `Texture2D` arrays. use 1 otherwise
    pub array_layers: u32,
    /// Number of mip levels in the image
    pub mip_levels: u32,
    /// Number of MSAA samples
    pub samples: u32,
    /// Indicate how this texture will be used on the GPU
    pub usage: TextureUsage,
    /// Initial state to start image transition barriers before state
    pub initial_state: ResourceState,
}

/// Describes the dimension of a texture
#[derive(Copy, Clone, Debug)]
pub enum TextureType {
    Texture1D,
    Texture1DArray,
    Texture2D,
    Texture2DArray,
    Texture3D,
    TextureCube,
    TextureCubeArray
}

/// Values to clear colour render targets at the start of a `RenderPass`
#[derive(Copy, Clone)]
pub struct ClearColour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// Values to clear depth stencil buffers during a `RenderPass`
pub struct ClearDepthStencil {
    /// Clear value for the depth buffer. Use `None` to preserve existing contents.
    pub depth: Option<f32>,
    /// Clear value for the stencil buffer. Use `None` to preserve existing contents.
    pub stencil: Option<u8>,
}

/// Information to create a render pass
pub struct RenderPassInfo<'stack, D: Device> {
    /// Array of textures which have been created with render target flags
    pub render_targets: Vec<&'stack D::Texture>,
    /// Colour to clear render target when the pass starts, use None to preserve previous contents
    pub rt_clear: Option<ClearColour>,
    /// A texture which was created with depth stencil flags
    pub depth_stencil: Option<&'stack D::Texture>,
    /// Depth value (in view) to clear depth stencil, use None to preserve previous contents
    pub ds_clear: Option<ClearDepthStencil>,
    /// Choose to resolve multi-sample AA targets,
    pub resolve: bool,
    /// (must also specify None to clear). This can save having to Load conents from main memory
    pub discard: bool,
    /// Array layer, depth slice or cubemap face to render to
    pub array_slice: usize
}

/// Transitions are required to be performed to switch resources from reading to writing or into different formats
pub struct TransitionBarrier<'stack, D: Device> {
    /// A texture to perform the transition on, either `texture` xor `buffer` must be `Some`
    pub texture: Option<&'stack D::Texture>,
    /// A buffer to perform the transition on, either `buffer` xor `texture` must be `Some`
    pub buffer: Option<&'stack D::Buffer>,
    /// The state of the resource before the transition is made, this must be correct otherwise it will throw validation warnings
    pub state_before: ResourceState,
    /// The state we want to transition into
    pub state_after: ResourceState,
}

/// All possible resource states, some for buffers and some for textures
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum ResourceState {
    /// Used for texture only to be written to from fragment shaders
    RenderTarget,
    /// Used for a texture to be used as a depth stencil buffer
    DepthStencil,
    /// Used for when depth testing is enabled, but depth writes are disabled
    DepthStencilReadOnly,
    /// Used for swap chain textures only, required before calling swap
    Present,
    /// Access for read/write from shaders
    UnorderedAccess,
    /// Readable from shaders
    ShaderResource,
    /// Bindable as a vertex or constant buffer for use in shaders
    VertexConstantBuffer,
    /// Bindable as an index buffer
    IndexBuffer,
    /// Used as a source msaa texture to resolve into a non-msaa resource
    ResolveSrc,
    /// Used as a destination sngle sample texture to be resolved into by an msaa resource
    ResolveDst,
    /// Used as a source for copies from into other resources
    CopySrc,
    /// Used as a destination for copies from other resources or queries
    CopyDst,
    /// Used as destination to read back data from buffers / queries
    GenericRead,
    /// Used for argument buffer in `execute_indirect` calls 
    IndirectArgument,
    /// Used for destination acceleration structure buffers
    AccelerationStructure
}

/// ome resources may contain subresources for resolving
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum Subresource {
    /// The resource itself for example a multi-sample texture has x number of MSAA samples
    Resource,
    /// The sub resource for example an MSAA texture will also create a non-MSAA subresource for resolving in to.
    ResolveResource
}

/// Info to control mapping of resources for read/write access
#[derive(Default)]
pub struct MapInfo {
    /// Sub resource to map ie. mip level, cubemap face, array slice
    pub subresource: u32,
    /// Range start of data we wish to read, for write-only access supply 0
    pub read_start: usize,
    /// Range end of data we wish to read, for write only access supply 0, to read the whole resource supply usize::MAX
    pub read_end: usize,
}

/// Info to control writing of mapped resources
pub struct UnmapInfo {
    /// Sub resource to map ie. mip level, cubemap face, array slice
    pub subresource: u32,
    /// Range start of data we have written to the buffer, supply 0 for read-only
    pub write_start: usize,
    /// Range end of data we have written to the buffer, supply 0 for read only
    pub write_end: usize,
}

/// Enum to differentiate between render and compute pipelines but also still work on them generically
pub enum PipelineType {
    Render,
    Compute
}

/// An opaque Shader type
pub trait Shader<D: Device>: Send + Sync {}

/// An opaque render pipeline type set blend, depth stencil, raster states on a pipeline, and bind with `CmdBuf::set_pipeline_state`
pub trait RenderPipeline<D: Device>: Send + Sync  {}

/// An opaque RenderPass containing an optional set of colour render targets and an optional depth stencil target
pub trait RenderPass<D: Device>: Send + Sync  {
    /// Returns a hash based on the render target format so that pipelines can be shared amonst compatible passes
    /// hash is based on render target format, depth stencil format and MSAA sample count
    fn get_format_hash(&self) -> u64;
}

/// An opaque compute pipeline type..
pub trait ComputePipeline<D: Device>: Send + Sync  {}

/// An opaque compute pipeline type..
pub trait RaytracingPipeline<D: Device>: Send + Sync  {}

/// An opaque shader table binding type..
pub trait RaytracingShaderBindingTable<D: Device>: Send + Sync  {}

/// An opaque top level acceleration structure for ray tracing geometry
pub trait RaytracingTLAS<D: Device>: Send + Sync  {}

/// An opaque bottom level acceleration structure for ray tracing geometry
pub trait RaytracingBLAS<D: Device>: Send + Sync  {}

/// A pipeline trait for shared functionality between Compute and Render pipelines
pub trait Pipeline {
    /// Returns the `PipelineSlotInfo` of which slot to bind a heap to based on the reequested `register` and `descriptor_type`
    /// if `None` is returned the pipeline does not contain bindings for the requested information
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&PipelineSlotInfo>;
    /// Returns a vec of all pipeline slot indices 
    fn get_pipeline_slots(&self) -> &Vec<u32>;
    /// Returns the pipeline type
    fn get_pipeline_type() -> PipelineType;
}

/// A command signature is used to `execute_indirect` commands
pub trait CommandSignature<D: Device>: Send + Sync {}

/// Different types of arguments which can be changed through execute indirect calls
#[derive(Clone, Copy)]
pub enum IndirectArgumentType {
    /// Used to issue indirect `draw` calls
    Draw,
    /// Used to issue indirect `draw_indexed` calls
    DrawIndexed,
    /// Used to issue indirect compute `dispatch` calls
    Dispatch,
    /// Used to change a vertex buffer binding
    VertexBuffer,
    /// Used to change an index buffer binding
    IndexBuffer,
    /// Used to change push constants
    PushConstants,
    /// Used to change constant buffer binding
    ConstantBuffer,
    /// Used to change a shader resource view binding
    ShaderResource,
    /// Userd to change an unordered access view binding
    UnorderedAccess
}

/// Arguments to change push constants during an `execute_indirect` call when `Constant` is the `IndirectArgumentType`
#[derive(Clone, Copy)]
pub struct IndirectPushConstantsArguments {
    /// The pipeline slot to modify
    pub slot: u32,
    /// Offset in 32bit values
    pub offset: u32,
    /// Number of 32bit values
    pub num_values: u32,
}

/// Arguments to change a buffer during an `execute_indirect` call when `ConstantBuffer`, `ShaderResource` or `UnorderedAccess`
/// are the `IndirectArgumentType`
#[derive(Clone, Copy)]
pub struct IndirectBufferArguments {
    /// The pipeline layout slot or the vertex buffer / index buffer slot
    pub slot: u32
}

/// This can be used for `Draw`, `DrawIndexed`, or `Dispatch` `IndirectArgumentType`
#[derive(Clone, Copy)]
pub struct IndirectNoArguments;

/// Union of `IndirectArguments` where data can be selected by the `IndirectArgumentType`
pub union IndirectTypeArguments {
    pub push_constants: IndirectPushConstantsArguments,
    pub buffer: IndirectBufferArguments
}

/// Pair of `IndirectArgumentType` and `IndirectTypeArguments` where the type selects the union member of data
pub struct IndirectArgument {
    pub argument_type: IndirectArgumentType,
    pub arguments: Option<IndirectTypeArguments>
}

/// Structure of arguments which can be used to execute `draw_instanced` calls indirectly 
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DrawArguments {
    pub vertex_count_per_instance: u32,
    pub instance_count: u32,
    pub start_vertex_location: u32,
    pub start_instance_location: u32
}

/// Structure of arguments which can be used to execute `draw_indexed_instanced` calls indirectly 
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DrawIndexedArguments {
    pub index_count_per_instance: u32,
    pub instance_count: u32,
    pub start_index_location: u32,
    pub base_vertex_location: i32,
    pub start_instance_location: u32,
}

/// Structure of arguments which can be used to execute `dispatch` calls indirectly
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DispatchArguments {
    pub thread_group_count_x: u32,
    pub thread_group_count_y: u32,
    pub thread_group_count_z: u32,
}

/// Structure of arguments which can be used to change a vertex buffer during `execute_indirect`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexBufferView {
    pub location: u64,
    pub size_bytes: u32,
    pub stride_bytes: u32,
}

/// Structure of arguments which can be used to change an index buffer during `execute_indirect`
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IndexBufferView {
    pub location: u64,
    pub size_bytes: u32,
    pub format: u32,
}

/// A GPU device is used to create GPU resources, the device also contains a single a single command queue
/// to which all command buffers will submitted and executed each frame. Default heaps for shader resources,
/// render targets and depth stencils are also provided
pub trait Device: 'static + Send + Sync + Sized + Any + Clone {
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
    type Buffer: Buffer<Self>;
    type Shader: Shader<Self>;
    type RenderPipeline: RenderPipeline<Self>;
    type Texture: Texture<Self>;
    type ReadBackRequest: ReadBackRequest<Self>;
    type RenderPass: RenderPass<Self>;
    type Heap: Heap<Self>;
    type QueryHeap: QueryHeap<Self>;
    type ComputePipeline: ComputePipeline<Self>;
    type RaytracingPipeline: RaytracingPipeline<Self>;
    type CommandSignature: CommandSignature<Self>;
    type RaytracingShaderBindingTable: RaytracingShaderBindingTable<Self>;
    type RaytracingBLAS: RaytracingBLAS<Self>;
    type RaytracingTLAS: RaytracingTLAS<Self>;
    /// Create a new GPU `Device` from `Device Info`
    fn create(info: &DeviceInfo) -> Self;
    /// Create a new resource `Heap` from `HeapInfo`
    fn create_heap(&mut self, info: &HeapInfo) -> Self::Heap;
    /// Create a new `QueryHeap` from `QueryHeapInfo`
    fn create_query_heap(&self, info: &QueryHeapInfo) -> Self::QueryHeap;
    /// Create a new `SwapChain` from `SwapChainInfo` and bind it to the specified `window`
    fn create_swap_chain<A: os::App>(
        &mut self,
        info: &SwapChainInfo,
        window: &A::Window,
    ) -> Result<Self::SwapChain, Error>;
    /// Create a new `CmdBuf` with `num_buffers` internal buffers, the buffers can be swapped and syncronised
    /// with a new `SwapChain` to allow in-flight gpu/cpu overlapped prodicer consumers 
    fn create_cmd_buf(&self, num_buffers: u32) -> Self::CmdBuf;
    /// Create a new `Shader` from `ShaderInfo`
    fn create_shader<T: Sized>(&self, info: &ShaderInfo, src: &[T]) -> Result<Self::Shader, Error>;
    /// Create a new `Buffer` from `BufferInfo` with any resource views allocated on the devices `shader_heap`
    fn create_buffer<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Buffer, Error>;
    /// Create a new `Buffer` from `BufferInfo` with any resource views allocated on the specified `Heap` that must be of `HeapType::Shader`
    fn create_buffer_with_heap<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
        heap: &mut Self::Heap
    ) -> Result<Self::Buffer, Error>;
    /// Create a `Buffer` specifically for reading back data from the GPU mainly for `Query` use
    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> Result<Self::Buffer, Error>;
    /// Create a new texture from `TextureInfo` and initialise it with optional data which can be any slice of a sized `T`
    fn create_texture<T: Sized>(
        &mut self,
        info: &TextureInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Texture, Error>;
    /// Create a new texture from `TextureInfo` and initialise it with optional data which can be any slice of a sized `T`
    /// allocates requested views into the supplied heaps, if the heaps are `None` this will use the default device heaps.
    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> Result<Self::Texture, Error>;
    /// Create a new render pipeline state object from the supplied `RenderPipelineInfo`
    fn create_render_pipeline(
        &self,
        info: &RenderPipelineInfo<Self>,
    ) -> Result<Self::RenderPipeline, Error>;
    /// Create a new render pass from `RenderPassInfo`
    fn create_render_pass(&self, info: &RenderPassInfo<Self>) -> Result<Self::RenderPass, Error>;
    /// Create a new compute pipeline state object from `ComputePipelineInfo`
    fn create_compute_pipeline(
        &self,
        info: &ComputePipelineInfo<Self>,
    ) -> Result<Self::ComputePipeline, Error>;
    /// Create a new raytracing pipeline state object from `RaytracingPipelineInfo`
    fn create_raytracing_pipeline(
        &self,
        info: &RaytracingPipelineInfo<Self>,
    ) -> Result<Self::RaytracingPipeline, Error>;
    /// Create a new raytracing shader binding table from `RaytracingShaderBindingTableInfo`
    fn create_raytracing_shader_binding_table(
        &self,
        info: &RaytracingShaderBindingTableInfo<Self>
    ) -> Result<Self::RaytracingShaderBindingTable, Error>;
    /// Create a bottom level acceleration structure from `RaytracingGeometryInfo`
    fn create_raytracing_blas(
        &mut self,
        info: &RaytracingBLASInfo<Self>
    ) -> Result<Self::RaytracingBLAS, Error>;
    /// Create a top level acceleration structure from array of `RaytracingInstanceInfo`
    fn create_raytracing_tlas(
        &mut self,
        info: &RaytracingTLASInfo<Self>
    ) -> Result<Self::RaytracingTLAS, Error>;
    /// Create a command signature for `execute_indirect` commands associated on the `RenderPipeline`
    fn create_indirect_render_command<T: Sized>(
        &mut self, 
        arguments: Vec<IndirectArgument>,
        pipeline: Option<&Self::RenderPipeline>
    ) -> Result<Self::CommandSignature, super::Error>;
    /// Execute a command buffer on the internal device command queue which still hold references
    fn execute(&self, cmd: &Self::CmdBuf);
    /// Borrow the internally managed shader resource heap the device creates, for binding buffers / textures in shaders
    fn get_shader_heap(&self) -> &Self::Heap;
    /// Mutably borrow the internally managed shader resource heap the device creates, for binding buffers / textures in shaders
    fn get_shader_heap_mut(&mut self) -> &mut Self::Heap;
    /// Cleans up resources which have been dropped associated with the device heaps, safeley waiting for
    /// any in-flight GPU operations to complete
    fn cleanup_dropped_resources(&mut self, swap_chain: &Self::SwapChain);
    /// Returns an `AdapterInfo` struct (info about GPU vendor, and HW statistics)
    fn get_adapter_info(&self) -> &AdapterInfo;
    /// Returns a `DeviceFeatureFlags` struct containing flags for supported hardware features
    fn get_feature_flags(&self) -> &DeviceFeatureFlags;
    /// Read data back from GPU buffer into CPU `ReadBackData` assumes the `Buffer` is created with `create_read_back_buffer`
    /// None is returned if the buffer has yet to br written on the GPU
    fn read_buffer(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Option<ReadBackData>;
    /// Read back u64 timestamp values as values in seconds, the vector will be empty if the buffer is yet to be written
    /// on the GPU
    fn read_timestamps(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Vec<f64>;
    /// Read back a single pipeline statistics query, assuming `buffer` was created with `create_read_back_buffer` 
    /// and is of size `get_pipeline_statistics_size_bytes()`. None is returned if the buffer is not ready
    fn read_pipeline_statistics(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, frame_written_fence: u64) -> Option<PipelineStatistics>;
    /// Reorts internal graphics api backend resources
    fn report_live_objects(&self) -> Result<(), Error>;
    /// Retrieve messages in the info queue since they were last drained
    fn get_info_queue_messages(&self) -> Result<Vec<String>, Error>;
    /// Size of a single timestamp query result in bytes
    fn get_timestamp_size_bytes() -> usize;
    /// Size of a single pipeline statistics query result in bytes
    fn get_pipeline_statistics_size_bytes() -> usize;
    /// Size of the indirect draw command in bytes
    fn get_indirect_command_size(argument_type: IndirectArgumentType) -> usize;
    /// Returns the alignment requirement size in bytes for counters (append buffers / uavs)
    fn get_counter_alignment() -> usize;
}

/// A swap chain is connected to a window, controls fences and signals as we swap buffers.
pub trait SwapChain<D: Device>: 'static + Sized + Any + Send + Sync + Clone {
    /// Call to begin a new frame, to synconise with v-sync and internally swap buffers
    fn new_frame(&mut self);
    /// Update to syncornise with the window, this may require the backbuffer to resize
    fn update<A: os::App>(&mut self, device: &mut D, window: &A::Window, cmd: &mut D::CmdBuf);
    /// Waits on the CPU for the last frame that was submitted with `swap` to be completed by the GPU
    fn wait_for_last_frame(&self);
    /// Returns the fence value for the current frame, you can use this to syncronise reads
    fn get_frame_fence_value(&self) -> u64;
    /// Returns the number of buffers in the swap chain
    fn get_num_buffers(&self) -> u32;
    /// Returns the current backbuffer index, this is the buffer that will be written during
    /// the current frame
    fn get_backbuffer_index(&self) -> u32;
    /// Returns the current backbuffer texture
    fn get_backbuffer_texture(&self) -> &D::Texture;
    /// Returns the current backbuffer pass this is the one
    /// we want to render to during the current frame
    fn get_backbuffer_pass(&self) -> &D::RenderPass;
    /// Returns the current backbuffer pass mutuably
    fn get_backbuffer_pass_mut(&mut self) -> &mut D::RenderPass;
    /// Returns the current backbuffer pass without a clear
    fn get_backbuffer_pass_no_clear(&self) -> &D::RenderPass;
    /// Returns the current backbuffer pass without a clear mutably
    fn get_backbuffer_pass_no_clear_mut(&mut self) -> &mut D::RenderPass;
    /// Call swap at the end of the frame to swap the back buffer, we rotate through n-buffers
    fn swap(&mut self, device: &D);
}
    
/// Responsible for buffering graphics commands. Internally it will contain a platform specific
/// command list for each buffer in the associated swap chain.
/// At the start of each frame `reset` must be called with an associated swap chain to internally switch
/// which buffer we are writing to. At the end of each frame `close` must be called
/// and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU.
pub trait CmdBuf<D: Device>: Send + Sync + Clone {
    /// Reset the `CmdBuf` for use on a new frame, it will be syncronised with the `SwapChain` so that
    /// in-flight command buffers are not overwritten
    fn reset(&mut self, swap_chain: &D::SwapChain);
    /// Call close to the command buffer after all commands have been added and before passing to `Device::execute` 
    fn close(&mut self) -> Result<(), Error>;
    /// Internally the `CmdBuf` contains a set of buffers which it rotates through to allow inflight operations
    /// to complete, this value indicates the buffer number you should `write` to during the current frame 
    fn get_backbuffer_index(&self) -> u32;
    /// Begins a render pass, end must be called
    fn begin_render_pass(&self, render_pass: &D::RenderPass);
    /// End a render pass must be called after `begin_render_pass` has been called
    fn end_render_pass(&self);
    /// Begin a names marker event which will be visible in tools such as PIX or RenderDoc
    fn begin_event(&mut self, colour: u32, name: &str);
    /// End an event that was started with `begin_event`
    fn end_event(&mut self);
    /// Similar to `begin_event/end_event` except it inserts a single marker point instead of a range
    fn set_marker(&self, colour: u32, name: &str);
    /// Function to specifically insert a timestamp query and request readback into the `Buffer`
    /// read back the rsult with `Device::read_timestamps`
    fn timestamp_query(&mut self, heap: &mut D::QueryHeap, resolve_buffer: &mut D::Buffer);
    /// Begin a new query in the heap, it will allocate an index which is returned as `usize`
    fn begin_query(&mut self, heap: &mut D::QueryHeap, query_type: QueryType) -> usize;
    /// End a query that was made on the heap results will be pushed into the `resolve_buffer` 
    /// the data can be read by `Device::read_buffer` or specialisations such as `read_pipeline_statistics`
    fn end_query(&mut self, heap: &mut D::QueryHeap, query_type: QueryType, index: usize, resolve_buffer: &mut D::Buffer);
    /// Add a transition barrier for resources to change states based on info supplied in `TransitionBarrier`
    fn transition_barrier(&mut self, barrier: &TransitionBarrier<D>);
    /// Add a transition barrier for a sub resource (ie. resolve texture)
    fn transition_barrier_subresource(&mut self, barrier: &TransitionBarrier<D>, subresource: Subresource);
    /// Set the viewport on the rasterizer stage
    fn set_viewport(&self, viewport: &Viewport);
    /// Set the scissor rect on the rasterizer stage
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    /// Set the index `buffer` to use for draw calls, the buffer should be created with `BufferUsage::INDEX`
    fn set_index_buffer(&self, buffer: &D::Buffer);
    /// Set the index `buffer` on `slot` to use for draw calls, the buffer should be created with `BufferUsage::VERTEX`
    fn set_vertex_buffer(&self, buffer: &D::Buffer, slot: u32);
    /// Set render pipeline for `draw` commands
    fn set_render_pipeline(&self, pipeline: &D::RenderPipeline);
    /// Set a compute pipeline for `dispatch`
    fn set_compute_pipeline(&self, pipeline: &D::ComputePipeline);
    /// Set's the active shader heap for the pipeline (srv, uav and cbv) and sets all descriptor tables to the root of the heap
    fn set_heap<T: Pipeline>(&self, pipeline: &T, heap: &D::Heap);
    /// Binds the heap with offset (texture srv, uav) on to the `slot` of a pipeline.
    /// this is like a traditional bindful render architecture `cmd.set_binding(pipeline, heap, 0, texture1_id)`
    fn set_binding<T: Pipeline>(&self, pipeline: &T, heap: &D::Heap, slot: u32, offset: usize);
    /// Push a small amount of data into the command buffer for a render pipeline, num values and dest offset are the numbr of 32bit values
    fn push_render_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]);
    /// Push a small amount of data into the command buffer for a compute pipeline, num values and dest offset are the numbr of 32bit values
    fn push_compute_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]);
    /// Make a non-indexed draw call supplying vertex and instance counts
    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    );
    /// Make an indexed draw call supplying index and instance counts, an index buffer should be bound
    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    );
    /// Thread count is required for metal, in hlsl it is specified in the shader
    fn dispatch(&self, group_count: Size3, numthreads: Size3);
    /// Issue indirect commands with signature created from `create_indirect_render_command`
    fn execute_indirect(
        &self,
        command: &D::CommandSignature, 
        max_command_count: u32, 
        argument_buffer: &D::Buffer, 
        argument_buffer_offset: usize,
        counter_buffer: Option<&D::Buffer>,
        counter_buffer_offset: usize
    );
    /// Resolves the `subresource` (mip index, 3d texture slice or array slice)
    fn resolve_texture_subresource(&self, texture: &D::Texture, subresource: u32) -> Result<(), Error>;
    /// Generates a full mip chain for the specified `texture` where `heap` is the shader heap the texture was created on 
    fn generate_mip_maps(&mut self, texture: &D::Texture, device: &D, heap: &D::Heap) -> Result<(), Error>;
    /// Read back the swapchains contents to CPU
    fn read_back_backbuffer(&mut self, swap_chain: &D::SwapChain) -> Result<D::ReadBackRequest, Error>;
    /// Copy from one buffer to another with offsets
    fn copy_buffer_region(
        &mut self, 
        dst_buffer: &D::Buffer, 
        dst_offset: usize, 
        src_buffer: &D::Buffer, 
        src_offset: usize,
        num_bytes: usize
    );
    /// Copy from one texture to another with offsets, if `None` is specified for `src_region`
    /// it will copy the full size of src 
    fn copy_texture_region(
        &mut self,
        dst_texture: &D::Texture,
        subresource_index: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_texture: &D::Texture,
        src_region: Option<Region>
    );
}

/// An opaque Buffer type used for vertex, index, constant or unordered access.
pub trait Buffer<D: Device>: Send + Sync {
    /// updates the buffer by mapping and copying memory, if you update while a buffer is in use on the GPU you may see tearing
    /// multi-buffer updates to buffer so that a buffer is never written to while in flight on the GPU.
    /// this function internally will map and unmap
    fn update<T: Sized>(&mut self, offset: usize, data: &[T]) -> Result<(), Error>; // TODO: should be mut surely?
    // write data directly to the buffer, the buffer is required to be persistently mapped
    fn write<T: Sized>(&mut self, offset: usize, data: &[T]) -> Result<(), Error>; 
    /// maps the entire buffer for reading or writing... see MapInfo
    fn map(&mut self, info: &MapInfo) -> *mut u8;
    /// unmap buffer... see UnmapInfo
    fn unmap(&mut self, info: &UnmapInfo);
    /// Return the index to access in a shader as a structured buffer
    fn get_srv_index(&self) -> Option<usize>;
    /// Return the index to access in a shader as a cbuffer
    fn get_cbv_index(&self) -> Option<usize>;
    /// Return the index to unorder access view for read/write from shaders...
    fn get_uav_index(&self) -> Option<usize>;
    /// Return a vertex buffer view
    fn get_vbv(&self) -> Option<VertexBufferView>;
    /// Return an index buffer view
    fn get_ibv(&self) -> Option<IndexBufferView>;
    /// Returns the offset in bytes of a counter element for an append structured buffer
    /// `None` is returned if the buffer was not created with `BufferUsage::APPEND_COUNTER`
    fn get_counter_offset(&self) -> Option<usize>;
}

/// An opaque Texture type
pub trait Texture<D: Device>: Send + Sync {
    /// Return the index to access in a shader (if the resource has msaa this is the resolved view)
    fn get_srv_index(&self) -> Option<usize>;
    /// Return the index to unorderd access view for read/write from shaders...
    fn get_uav_index(&self) -> Option<usize>;
    /// Return the subresource index unorderd access view for read/write from shaders
    /// where subresource is the array slice * num mips + mip you want to access
    fn get_subresource_uav_index(&self, subresource: u32) -> Option<usize>;
    /// Return the index of an msaa resource to access in a shader
    fn get_msaa_srv_index(&self) -> Option<usize>;
    /// Return a clone of the internal (platform specific) resource
    fn clone_inner(&self) -> Self;
    /// Returns true if this texture has a subresource which can be resolved into
    fn is_resolvable(&self) -> bool;
    /// Return the id of the shader heap
    fn get_shader_heap_id(&self) -> Option<u16>;
}

/// An opaque shader heap type, use to create views of resources for binding and access in shaders
pub trait Heap<D: Device>: Send + Sync {
    /// Deallocate a resource from the heap and mark space in free list for re-use
    fn deallocate(&mut self, index: usize);
    /// Cleans up resources which have been dropped associated with this heap, safeley waiting for
    /// any in-flight GPU operations to complete
    fn cleanup_dropped_resources(&mut self, swap_chain: &D::SwapChain);
    /// Returns the id of the heap to verify and correlate with resources
    fn get_heap_id(&self) -> u16;
}

/// An opaque query heap type, use to create queries
pub trait QueryHeap<D: Device>: Send + Sync {
    /// Reset queries at the start of the frame, each query requested will bump the allocation index
    fn reset(&mut self);
}

/// Used to readback data from the GPU, once the request is issued `is_complete` needs to be waited on for completion
/// you must poll this every frame and not block so the GPU can flush the request. Once the result is ready the
/// data can be obtained using `get_data`
pub trait ReadBackRequest<D: Device> {
    /// Returns true when a reload request has completed and it is safe to call map
    fn is_complete(&self, swap_chain: &D::SwapChain) -> bool;
    /// Maps the buffer to allow the CPU to read GPU mapped data
    fn map(&self, info: &MapInfo) -> Result<ReadBackData, Error>;
    /// Balance with a call to map. note: it is possible to leave buffers persitently mapped
    fn unmap(&self);
}

/// Results from an issued ReadBackRequest
#[derive(Clone)]
pub struct ReadBackData {
    /// Slice of data bytes
    pub data: &'static [u8],
    /// GPU format to interperet the data
    pub format: Format,
    /// Total size of data (should be == data.len())
    pub size: usize,
    /// Pitch of a row of data
    pub row_pitch: usize,
    /// Pitch of a slice (3D texture or array level, cubemap face etc)
    pub slice_pitch: usize,
}

/// Take any sized type and return a u8 slice. This can be useful to pass `data` to `Device::create_buffer`.
pub fn as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}

/// Take any sized silce and convert to a slice of u8
pub fn slice_as_u8_slice<T: Sized>(p: &[T]) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p.as_ptr() as *const T) as *const u8,
            ::std::mem::size_of::<T>() * p.len(),
        )
    }
}

/// Returns the 'block size' (texel, compressed block of texels or single buffer element) for a given format
pub const fn block_size_for_format(format: Format) -> u32 {
    match format {
        Format::Unknown => 0,
        Format::R16n => 2,
        Format::R16u => 2,
        Format::R16i => 2,
        Format::R16f => 2,
        Format::R32u => 4,
        Format::R32i => 4,
        Format::R32f => 4,
        Format::RG16u => 4,
        Format::RG16i => 4,
        Format::RG16f => 4,
        Format::RG32u => 8,
        Format::RG32i => 8,
        Format::RG32f => 8,
        Format::RGBA8nSRGB => 4,
        Format::RGBA8n => 4,
        Format::RGBA8u => 4,
        Format::RGBA8i => 4,
        Format::BGRA8n => 4,
        Format::BGRX8n => 4,
        Format::BGRA8nSRGB => 4,
        Format::BGRX8nSRGB => 4,
        Format::RGB32u => 12,
        Format::RGB32i => 12,
        Format::RGB32f => 12,
        Format::RGBA16u => 8,
        Format::RGBA16i => 8,
        Format::RGBA16f => 8,
        Format::RGBA32u => 16,
        Format::RGBA32i => 16,
        Format::RGBA32f => 16,
        Format::D32fS8X24u => 8,
        Format::D32f => 16,
        Format::D24nS8u => 32,
        Format::D16n => 2,
        Format::BC1n => 8,
        Format::BC1nSRGB => 8,
        Format::BC2n => 4,
        Format::BC2nSRGB => 4,
        Format::BC3n => 16,
        Format::BC3nSRGB => 16,
        Format::BC4n => 8,
        Format::BC5n => 16,
    }
}

/// Returns the number of texels (texel x texel) in each block for the specified texture format
pub const fn texels_per_block_for_format(format: Format) -> u64 {
    match format {
        Format::BC1n => 4,
        Format::BC1nSRGB => 4,
        Format::BC2n => 4,
        Format::BC2nSRGB => 4,
        Format::BC3n => 4,
        Format::BC3nSRGB => 4,
        Format::BC4n => 4,
        Format::BC5n => 4,
        _ => 1,
    }
}

/// Returns the number of components for a given format. ie RGBA = 4 and RGB = 3
pub const fn components_for_format(format: Format) -> u32 {
    match format {
        Format::Unknown => 0,
        Format::R16n => 1,
        Format::R16u => 1,
        Format::R16i => 1,
        Format::R16f => 1,
        Format::R32u => 1,
        Format::R32i => 1,
        Format::R32f => 1,
        Format::RG16u => 2,
        Format::RG16i => 2,
        Format::RG16f => 2,
        Format::RG32u => 2,
        Format::RG32i => 2,
        Format::RG32f => 2,
        Format::RGBA8nSRGB => 4,
        Format::RGBA8n => 4,
        Format::RGBA8u => 4,
        Format::RGBA8i => 4,
        Format::BGRA8n => 4,
        Format::BGRX8n => 4,
        Format::BGRA8nSRGB => 4,
        Format::BGRX8nSRGB => 4,
        Format::RGB32u => 3,
        Format::RGB32i => 3,
        Format::RGB32f => 3,
        Format::RGBA16u => 4,
        Format::RGBA16i => 4,
        Format::RGBA16f => 4,
        Format::RGBA32u => 4,
        Format::RGBA32i => 4,
        Format::RGBA32f => 4,
        Format::D32fS8X24u => 2,
        Format::D32f => 1,
        Format::D24nS8u => 2,
        Format::D16n => 1,
        Format::BC1n => 4,
        Format::BC1nSRGB => 4,
        Format::BC2n => 3,
        Format::BC2nSRGB => 3,
        Format::BC3n => 4,
        Format::BC3nSRGB => 4,
        Format::BC4n => 1,
        Format::BC5n => 2,
    }
}

/// Returns the row pitch of an image in bytes: width * block size
pub fn row_pitch_for_format(format: Format, width: u64) -> u64 {
    let tpb = texels_per_block_for_format(format);
    block_size_for_format(format) as u64 * (width / tpb).max(1)
}

/// Returns the slice pitch of an image in bytes: width * height * block size, a slice is a single 2D image
/// or a single slice of a 3D texture or texture array
pub fn slice_pitch_for_format(format: Format, width: u64, height: u64) -> u64 {
    let tpb = texels_per_block_for_format(format);
    block_size_for_format(format) as u64 * (width / tpb).max(1) * (height / tpb).max(1)
}

/// Return the size in bytes of a 3 dimensional resource: width * height * depth block size
pub fn size_for_format(format: Format, width: u64, height: u64, depth: u32) -> u64 {
    let tpb = texels_per_block_for_format(format);

    block_size_for_format(format) as u64 * (width / tpb).max(1) * (height / tpb).max(1) * depth as u64
}

/// Return the size in bytes of up to dimensional resource: width * height * depth block size
/// for each mip level and account for array layers
pub fn size_for_format_mipped(format: Format, width: u64, height: u64, depth: u32, array_layers: u32, mips: u32) -> u64 {
    let mut total = 0;
    let mut mip_width = width;
    let mut mip_height = height;
    let mut mip_depth = depth;
    for _ in 0..mips {
        total += size_for_format(format, mip_width, mip_height, mip_depth) * array_layers as u64;
        mip_width = max(mip_width / 2, 1);
        mip_height = max(mip_height / 2, 1);
        mip_depth = max(mip_depth / 2, 1);
    }
    total
}

/// Returns the number of mip levels required for a 2D texture
pub fn mip_levels_for_dimension(width: u64, height: u64) -> u32 {
    f32::log2(width.max(height) as f32) as u32 + 1
}

/// Aligns value to the alignment specified by align. value must be a power of 2
pub fn align_pow2(value: u64, align: u64) -> u64 {
    (value + (align - 1)) & !(align - 1)
}

/// Aligns value to the alignment specified by align. value can be non-power of 2
pub fn align(value: u64, align: u64) -> u64 {
    let div = value / align;
    let rem = value % align;
    if rem != 0 {
        return (div + 1) * align;
    }
    value
}

/// For the supplied sized struct `&_` returns the number of 32bit constants required for use as `push_constants`
pub const fn num_32bit_constants<T: Sized>(_: &T) -> u32 {
    (std::mem::size_of::<T>() / 4) as u32
}

/// Trait for sized types where num constants is the number of 32-bit constants in type
trait NumConstants {
    fn num_constants() -> u32;
}

/// Blanket implmenetation for sized `T`
impl<T> NumConstants for T where T: Sized {
    fn num_constants() -> u32 {
        (std::mem::size_of::<T>() / 4) as u32
    }
}

impl From<os::Rect<i32>> for Viewport {
    fn from(rect: os::Rect<i32>) -> Viewport {
        Viewport {
            x: rect.x as f32,
            y: rect.y as f32,
            width: rect.width as f32,
            height: rect.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }
}

impl From<os::Rect<i32>> for ScissorRect {
    fn from(rect: os::Rect<i32>) -> ScissorRect {
        ScissorRect {
            left: rect.x,
            top: rect.y,
            right: rect.width,
            bottom: rect.height,
        }
    }
}

/// Convert from WritMask bit mask to raw u8
impl From<WriteMask> for u8 {
    fn from(mask: WriteMask) -> u8 {
        mask.bits
    }
}

/// Display for `AdapterInfo` displays as so:
/// hotline_rs::d3d12::Device:
///   NVIDIA GeForce GTX 1060 6GB
///   Video Memory: 6052(mb)
///   System Memory: 0(mb)
///   Shared System Memory: 8159(mb)
/// Available Adapters:
///   NVIDIA GeForce GTX 1060 6GB
///   Microsoft Basic Render Driver
impl std::fmt::Display for AdapterInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut available = String::from("");
        for adapter in &self.available {
            available += "  ";
            available += adapter;
            available += "\n";
        }
        write!(
            f,
            "{}:
  {}
  Video Memory: {}(mb)
  System Memory: {}(mb)
  Shared System Memory: {}(mb)
Available Adapters:
{}",
            self.name,
            self.description,
            self.dedicated_video_memory / 1024 / 1024,
            self.dedicated_system_memory / 1024 / 1024,
            self.shared_system_memory / 1024 / 1024,
            available
        )
    }
}

/// Useful defaults for quick creation of `TextureInfo`
impl Default for TextureInfo {
    fn default() -> Self {
        TextureInfo {
            tex_type: TextureType::Texture2D,
            format: Format::RGBA8n,
            width: 1,
            height: 1,
            depth: 1,
            array_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: TextureUsage::SHADER_RESOURCE,
            initial_state: ResourceState::ShaderResource
        }
    }
}

/// Useful defaults for raster state on a pipeline state object, efetively means no culling, solid fill
impl Default for RasterInfo {
    fn default() -> Self {
        RasterInfo {
            fill_mode: FillMode::Solid,
            cull_mode: CullMode::None,
            front_ccw: false,
            depth_bias: 0,
            depth_bias_clamp: 0.0,
            slope_scaled_depth_bias: 0.0,
            depth_clip_enable: false,
            multisample_enable: false,
            antialiased_line_enable: false,
            forced_sample_count: 0,
            conservative_raster_mode: false,
        }
    }
}

///  Useful defaults for smample states, wrap linear
impl Default for SamplerInfo {
    fn default() -> Self {
        SamplerInfo {
            filter: SamplerFilter::Linear,
            address_u: SamplerAddressMode::Wrap,
            address_v: SamplerAddressMode::Wrap,
            address_w: SamplerAddressMode::Wrap,
            comparison: None,
            border_colour: None,
            mip_lod_bias: 0.0,
            max_aniso: 0,
            min_lod: -1.0,
            max_lod: -1.0,
        }
    }
}

///  Useful defaults for depth stencil state on a pipeline state object, no depth test or write
impl Default for DepthStencilInfo {
    fn default() -> Self {
        DepthStencilInfo {
            depth_enabled: false,
            depth_write_mask: DepthWriteMask::Zero,
            depth_func: ComparisonFunc::Always,
            stencil_enabled: false,
            stencil_read_mask: 0,
            stencil_write_mask: 0,
            front_face: StencilInfo {
                fail: StencilOp::Keep,
                depth_fail: StencilOp::Keep,
                pass: StencilOp::Keep,
                func: ComparisonFunc::Always,
            },
            back_face: StencilInfo {
                fail: StencilOp::Keep,
                depth_fail: StencilOp::Keep,
                pass: StencilOp::Keep,
                func: ComparisonFunc::Always,
            },
        }
    }
}

/// Useful defaults for blend state on a pipeline state object, no blending
impl Default for RenderTargetBlendInfo {
    fn default() -> Self {
        RenderTargetBlendInfo {
            blend_enabled: false,
            logic_op_enabled: false,
            src_blend: BlendFactor::Zero,
            dst_blend: BlendFactor::Zero,
            blend_op: BlendOp::Add,
            src_blend_alpha: BlendFactor::Zero,
            dst_blend_alpha: BlendFactor::Zero,
            blend_op_alpha: BlendOp::Add,
            logic_op: LogicOp::Clear,
            write_mask: WriteMask::ALL,
        }
    }
}

/// Defaults for a render pipline, which would do nothing
impl<'stack, D> Default for RenderPipelineInfo<'stack, D> where D: Device {
    fn default() -> Self {
        Self {
            vs: None,
            fs: None,
            input_layout: Vec::new(),
            pipeline_layout: PipelineLayout::default(),
            raster_info: RasterInfo::default(),
            depth_stencil_info: DepthStencilInfo::default(),
            blend_info: BlendInfo::default(),
            topology: Topology::TriangleList,
            patch_index: 0,
            sample_mask: u32::max_value(),
            pass: None
        }
    }
}

/// Pipeline stats initialised to zero
impl Default for PipelineStatistics {
    fn default() -> Self {
        PipelineStatistics {
            input_assembler_vertices: 0,
            input_assembler_primitives: 0,
            vertex_shader_invocations: 0,
            pixel_shader_primitives: 0,
            compute_shader_invocations: 0
        }
    }
}


/// Pipeline stats initialised to zero
impl<'stack, D> Default for TextureHeapInfo<'stack, D> where D: Device {
    fn default() -> Self {
        Self {
            shader: None,
            render_target: None,
            depth_stencil: None
        }
    }
}

/// Ability to add 2 pipeline stats to accumulate
impl std::ops::Add for PipelineStatistics {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            input_assembler_vertices: self.input_assembler_vertices + other.input_assembler_vertices,
            input_assembler_primitives: self.input_assembler_primitives + other.input_assembler_primitives,
            vertex_shader_invocations: self.vertex_shader_invocations + other.vertex_shader_invocations,
            pixel_shader_primitives: self.pixel_shader_primitives + other.pixel_shader_primitives,
            compute_shader_invocations: self.compute_shader_invocations + other.compute_shader_invocations,
        }
    }
}

/// Ability to add_assign 2 pipeline stats to accumulate
impl std::ops::AddAssign for PipelineStatistics {
    fn add_assign(&mut self, other: Self) {
        self.input_assembler_vertices += other.input_assembler_vertices;
        self.input_assembler_primitives += other.input_assembler_primitives;
        self.vertex_shader_invocations += other.vertex_shader_invocations;
        self.pixel_shader_primitives += other.pixel_shader_primitives;
        self.compute_shader_invocations += other.compute_shader_invocations;
    }
}

/// Display for resource state enums
impl std::fmt::Display for ResourceState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}