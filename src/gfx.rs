use crate::os;
use std::any::Any;
use serde::{Deserialize, Serialize};

/// Implemets this interface with a Direct3D12 backend.
pub mod d3d12;

type Error = super::Error;

/// macro to pass data!expression or data! (None) to a create function, so you don't have to deduce a 'T'
#[macro_export]
macro_rules! data {
    () => {
        None::<&[()]>
    };
    ($input:expr) => {
        Some($input)
    }
}

/// 3-Dimensional struct for compute shader thread count / thread group size
pub struct Size3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
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
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Format for resource types (textures / buffers).
/// n = normalised unsigned integer,
/// u = unsigned integer,
/// i = signed integer,
/// f = float
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum Format {
    Unknown,
    R16n,
    R16u,
    R16i,
    R16f,
    R32u,
    R32i,
    R32f,
    RG32u,
    RG32i,
    RG32f,
    RGB32u,
    RGB32i,
    RGB32f,
    RGBA8n,
    RGBA8u,
    RGBA8i,
    BGRA8n,
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
}

/// Information to create a device, it contains default heaps for resource views
/// resources will be automatically allocated into these heaps, you can supply custom heaps if necessary
#[derive(Default)]
pub struct DeviceInfo {
    /// optional adapter to choose a specific adapter in the scenario of a multi-adapter system
    /// if None is supplied the first non-software emulation adapter would be selected
    pub adapter_name: Option<String>,
    /// space for shader resource views, constant buffers and unordered access views
    pub shader_heap_size: usize,
    /// space for colour render targets
    pub render_target_heap_size: usize,
    /// space for depth stencil targets
    pub depth_stencil_heap_size: usize,
}

#[derive(Clone)]
/// Information returned from `Device::get_adapter_info`
pub struct AdapterInfo {
    /// The chosen adapter a device was created with
    pub name: String,
    pub description: String,
    pub dedicated_video_memory: usize,
    pub dedicated_system_memory: usize,
    pub shared_system_memory: usize,
    /// List of available adapter descriptons
    pub available: Vec<String>,
}

/// Information to create a desciptor heap... `Device` will contain default heaps, but you can create your own if required
pub struct HeapInfo {
    pub heap_type: HeapType,
    pub num_descriptors: usize,
}

/// Options for heap types
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum HeapType {
    /// For shader resource view, constant buffer or unordered access
    Shader,
    RenderTarget,
    DepthStencil,
    Sampler,
}

/// Information to pass to `Device::create_swap_chain`
pub struct SwapChainInfo {
    pub num_buffers: u32,
    /// must be BGRA8n, RGBA8n or RGBA16f
    pub format: Format,
    /// colour for clearing the window when using the backbuffer pass, use None to not clear
    pub clear_colour: Option<ClearColour>,
}

/// Information to create a buffer through `Device::create_buffer`.
#[derive(Copy, Clone)]
pub struct BufferInfo {
    /// Indicates how the buffer will be used on the GPU.
    pub usage: BufferUsage,
    /// Used to indicate if we want to read or write from the CPU, use NONE if possible for best performance
    pub cpu_access: CpuAccessFlags,
    /// Data format of the buffer
    pub format: Format,
    /// The stride of a vertex or structure in bytes.
    pub stride: usize,
    /// The number of array elements
    pub num_elements: usize,
}

/// Describes how a buffer will be used on the GPU.
#[derive(Copy, Clone)]
pub enum BufferUsage {
    Vertex,
    Index,
    ConstantBuffer,
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
}

bitflags! {
    /// Shader compilation flags
    pub struct ShaderCompileFlags: u32 {
        /// No flags, default compilation
        const NONE = 0b00000000;
        /// Generates shader with debug info
        const DEBUG = 0b00000001;
        /// Skips optimization for easier debuggability, deterministic results and faster compilation
        const SKIP_OPTIMIZATION = 0b00000010;
    }

    /// Render target write mask flags
    pub struct WriteMask : u8 {
        const RED = 1<<0;
        const GREEN = 1<<1;
        const BLUE = 1<<2;
        const ALPHA = 1<<3;
        const ALL = (1<<4)-1;
    }

    /// CPU Access flags for buffers or textures
    pub struct CpuAccessFlags: u8 {
        const NONE = 1<<0;
        const READ = 1<<1;
        const WRITE = 1<<2;
    }
}


/// Descriptor layout is required to create a pipeline it describes the layout of resources for access on the GPU.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct DescriptorLayout {
    pub bindings: Option<Vec<DescriptorBinding>>,
    pub push_constants: Option<Vec<PushConstantInfo>>,
    pub static_samplers: Option<Vec<SamplerBinding>>,
}

/// Describes a range of resources for access on the GPU.
#[derive(Clone, Serialize, Deserialize)]
pub struct DescriptorBinding {
    /// The shader stage the resources will be accessible to
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader)
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader)
    pub register_space: u32,
    /// Type of resources in this descriptor binding
    pub binding_type: DescriptorType,
    /// Number of descriptors in this table, use `None` for unbounded
    pub num_descriptors: Option<u32>,
}

/// Describes the type of descriptor binding to create.
#[derive(Clone, Serialize, Deserialize)]
pub enum DescriptorType {
    /// Used for textures or structured buffers
    ShaderResource,
    /// Used for cbuffers
    ConstantBuffer,
    /// Used for read-write textures
    UnorderedAccess,
    /// Used for texture samplers
    Sampler,
}

/// Describes the visibility of which shader stages can access a descriptor.
#[derive(Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderVisibility {
    All,
    Vertex,
    Fragment,
    Compute,
}

/// Describes space in the shader to send data to via `CmdBuf::push_constants`.
#[derive(Clone, Serialize, Deserialize)]
pub struct PushConstantInfo {
    /// The shader stage the constants will be accessible to
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader)
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader)
    pub register_space: u32,
    /// Number of 32-bit values to push
    pub num_values: u32,
}

/// Input layout describes the layout of vertex buffers bound to the input assembler.
pub type InputLayout = Vec<InputElementInfo>;

/// Describe a single element of an `InputLayoutInfo`
#[derive(Clone, Serialize, Deserialize)]
pub struct InputElementInfo {
    pub semantic: String,
    pub index: u32,
    pub format: Format,
    pub input_slot: u32,
    pub aligned_byte_offset: u32,
    pub input_slot_class: InputSlotClass,
    pub step_rate: u32,
}

/// Describes the frequency of which elements are fetched from a vertex input element.
#[derive(Clone, Serialize, Deserialize)]
pub enum InputSlotClass {
    PerVertex,
    PerInstance,
}

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
    pub descriptor_layout: DescriptorLayout,
    /// Control rasterisation of primitives
    pub raster_info: RasterInfo,
    /// Control depth test and stencil oprations
    pub depth_stencil_info: DepthStencilInfo,
    /// Control blending settings for the output merge stage
    pub blend_info: BlendInfo,
    /// Primitive topolgy oof the input assembler
    pub topology: Topology,
    /// only required for Topology::PatchList use 0 as default
    pub patch_index: u32,
    /// A valid render pass, you can share pipelines across passes providing the render target
    /// formats and sample count are the same of the passes you wish to use the pipeline on
    pub pass: &'stack D::RenderPass,
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
pub enum FillMode {
    Wireframe,
    Solid,
}

/// Polygon cull mode
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
pub enum BlendOp {
    Add,
    Subtract,
    RevSubtract,
    Min,
    Max,
}

/// The logical operation to configure for a render target blend with logic op enabled
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
    pub descriptor_layout: DescriptorLayout,
}

/// Information to create a pipeline through `Device::create_texture`.
#[derive(Copy, Clone)]
pub struct TextureInfo {
    pub tex_type: TextureType,
    pub format: Format,
    pub width: u64,
    pub height: u64,
    pub depth: u32,
    pub array_levels: u32,
    pub mip_levels: u32,
    pub samples: u32,
    pub usage: TextureUsage,
    /// Initial state to start image transition barriers before state
    pub initial_state: ResourceState,
}

/// Describes the dimension of a texture
#[derive(Copy, Clone)]
pub enum TextureType {
    Texture1D,
    Texture2D,
    Texture3D,
}

bitflags! {
    /// Textures can be used in one or more of the following ways
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
    }
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
pub struct RenderPassInfo<'a, D: Device> {
    /// Array of textures which have been created with render target flags
    pub render_targets: Vec<&'a D::Texture>,
    /// Colour to clear render target when the pass starts, use None to preserve previous contents
    pub rt_clear: Option<ClearColour>,
    /// A texture which was created with depth stencil flags
    pub depth_stencil: Option<&'a D::Texture>,
    /// Depth value (in view) to clear depth stencil, use None to preserve previous contents
    pub ds_clear: Option<ClearDepthStencil>,
    /// Choose to resolve multi-sample AA targets,
    pub resolve: bool,
    /// (must also specify None to clear). This can save having to Load conents from main memory
    pub discard: bool,
}

/// Transitions are required to be performed to switch resources from reading to writing or into different formats
pub struct TransitionBarrier<'a, D: Device> {
    pub texture: Option<&'a D::Texture>,
    pub buffer: Option<&'a D::Buffer>,
    pub state_before: ResourceState,
    pub state_after: ResourceState,
}

/// All possible resource states, some for buffers and some for textures
#[derive(Copy, Clone)]
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

/// An opaque Shader type
pub trait Shader<D: Device>: Send + Sync {}
/// An opaque render pipeline type set blend, depth stencil, raster states on a pipeline, and bind with `CmdBuf::set_pipeline_state`
pub trait RenderPipeline<D: Device>: Send + Sync  {}
/// An opaque RenderPass containing an optional set of colour render targets and an optional depth stencil target
pub trait RenderPass<D: Device>: Send + Sync  {}
/// An opaque compute pipeline type..
pub trait ComputePipeline<D: Device>: Send + Sync  {}

/// A GPU device is used to create GPU resources, the device also contains a single a single command queue
/// to which all command buffers will submitted and executed each frame.
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
    type ComputePipeline: ComputePipeline<Self>;
    fn create(info: &DeviceInfo) -> Self;
    fn create_heap(&self, info: &HeapInfo) -> Self::Heap;
    fn create_swap_chain<A: os::App>(
        &mut self,
        info: &SwapChainInfo,
        window: &A::Window,
    ) -> Result<Self::SwapChain, Error>;
    fn create_cmd_buf(&self, num_buffers: u32) -> Self::CmdBuf;
    fn create_shader<T: Sized>(&self, info: &ShaderInfo, src: &[T]) -> Result<Self::Shader, Error>;
    fn create_buffer<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Buffer, Error>;
    fn create_texture<T: Sized>(
        &mut self,
        info: &TextureInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Texture, Error>;
    fn create_render_pipeline(
        &self,
        info: &RenderPipelineInfo<Self>,
    ) -> Result<Self::RenderPipeline, Error>;
    fn create_render_pass(&self, info: &RenderPassInfo<Self>) -> Result<Self::RenderPass, Error>;
    fn create_compute_pipeline(
        &self,
        info: &ComputePipelineInfo<Self>,
    ) -> Result<Self::ComputePipeline, Error>;
    fn execute(&self, cmd: &Self::CmdBuf);
    fn report_live_objects(&self) -> Result<(), Error>;
    fn get_shader_heap(&self) -> &Self::Heap;
    fn get_shader_heap_mut(&mut self) -> &mut Self::Heap;
    fn get_adapter_info(&self) -> &AdapterInfo;
    fn as_ptr(&self) -> *const Self;
    fn as_mut_ptr(&mut self) -> *mut Self;
}

/// A swap chain is connected to a window, controls fences and signals as we swap buffers.
pub trait SwapChain<D: Device>: 'static + Sized + Any + Send + Sync + Clone {
    fn new_frame(&mut self);
    fn update<A: os::App>(&mut self, device: &mut D, window: &A::Window, cmd: &mut D::CmdBuf);
    fn wait_for_last_frame(&mut self);
    fn get_num_buffers(&self) -> u32;
    fn get_backbuffer_index(&self) -> u32;
    fn get_backbuffer_texture(&self) -> &D::Texture;
    fn get_backbuffer_pass(&self) -> &D::RenderPass;
    fn get_backbuffer_pass_mut(&mut self) -> &mut D::RenderPass;
    fn get_backbuffer_pass_no_clear(&self) -> &D::RenderPass;
    fn get_backbuffer_pass_no_clear_mut(&mut self) -> &mut D::RenderPass;
    fn swap(&mut self, device: &D);
    fn as_ptr(&self) -> *const Self;
    fn as_mut_ptr(&mut self) -> *mut Self;
}

/// Responsible for buffering graphics commands. Internally it will contain a platform specific
/// command list for each buffer in the associated swap chain.
/// At the start of each frame `reset` must be called with an associated swap chain to internally switch
/// which buffer we are writing to. At the end of each frame `close` must be called
/// and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU.
pub trait CmdBuf<D: Device>: Send + Sync + Clone {
    fn reset(&mut self, swap_chain: &D::SwapChain);
    fn close(&mut self, swap_chain: &D::SwapChain) -> Result<(), Error>;
    fn get_backbuffer_index(&self) -> u32;
    fn begin_render_pass(&self, render_pass: &mut D::RenderPass);
    fn end_render_pass(&self);
    fn begin_event(&mut self, colour: u32, name: &str);
    fn end_event(&mut self);
    fn transition_barrier(&mut self, barrier: &TransitionBarrier<D>);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn set_index_buffer(&self, buffer: &D::Buffer);
    fn set_vertex_buffer(&self, buffer: &D::Buffer, slot: u32);
    fn set_render_pipeline(&self, pipeline: &D::RenderPipeline);
    fn set_compute_pipeline(&self, pipeline: &D::ComputePipeline);
    fn set_compute_heap(&self, slot: u32, heap: &D::Heap);
    fn set_render_heap(&self, slot: u32, heap: &D::Heap, offset: usize);
    fn set_marker(&self, colour: u32, name: &str);
    fn push_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]);
    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    );
    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    );
    /// Thread count is required for metal, in hlsl it is specified in the shader
    fn dispatch(&self, group_count: Size3, thread_count: Size3);
    fn read_back_backbuffer(&mut self, swap_chain: &D::SwapChain) -> D::ReadBackRequest;
}

/// An opaque Buffer type used for vertex, index, constant or unordered access.
pub trait Buffer<D: Device>: Send + Sync {
    /// updates the buffer by mapping and copying memory, if you update while a buffer is in use on the GPU you may see tearing
    /// multi-buffer updates to buffer so that a buffer is never written to while in flight on the GPU.
    fn update<T: Sized>(&self, offset: isize, data: &[T]) -> Result<(), Error>;
    /// maps the entire buffer for reading or writing... see MapInfo
    fn map(&self, info: &MapInfo) -> *mut u8;
    /// unmap buffer... see UnmapInfo
    fn unmap(&self, info: &UnmapInfo);
    /// Return the index to access in a shader
    fn get_srv_index(&self) -> Option<usize>;
    /// Return the index to unorder access view for read/write from shaders...
    fn get_uav_index(&self) -> Option<usize>;
}

/// An opaque Texture type
pub trait Texture<D: Device>: Send + Sync {
    /// Return the index to access in a shader
    fn get_srv_index(&self) -> Option<usize>;
    /// Return the index to unorder access view for read/write from shaders...
    fn get_uav_index(&self) -> Option<usize>;
    /// return a clone
    fn clone_inner(&self) -> Self;
}

/// An opaque shader heap type, use to create views of resources for binding and access in shaders
pub trait Heap<D: Device>: Send + Sync {
    /// Deallocate a resource from the heap and mark space in free list for re-use
    fn deallocate(&mut self, index: usize);
}

/// Used to readback data from the GPU, once the request is issued `is_complete` needs to be waited on for completion
/// you must poll this every frame and not block so the GPU can flush the request. Once the result is ready the
/// data can be obtained using `get_data`
pub trait ReadBackRequest<D: Device> {
    fn is_complete(&self, swap_chain: &D::SwapChain) -> bool;
    fn map(&self, info: &MapInfo) -> Result<ReadBackData, Error>;
    fn unmap(&self);
}

/// Results from an issued ReadBackRequest
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
pub fn block_size_for_format(format: Format) -> u32 {
    match format {
        Format::Unknown => 0,
        Format::R16n => 2,
        Format::R16u => 2,
        Format::R16i => 2,
        Format::R16f => 2,
        Format::R32u => 4,
        Format::R32i => 4,
        Format::R32f => 4,
        Format::RG32u => 8,
        Format::RG32i => 8,
        Format::RG32f => 8,
        Format::RGBA8n => 4,
        Format::RGBA8u => 4,
        Format::RGBA8i => 4,
        Format::BGRA8n => 4,
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
    }
}

/// Returns the row pitch of an image in bytes: width * block size
pub fn row_pitch_for_format(format: Format, width: u64) -> u64 {
    block_size_for_format(format) as u64 * width
}

/// Returns the slice pitch of an image in bytes: width * height * block size, a slice is a single 2D image
/// or a single slice of a 3D texture or texture array
pub fn slice_pitch_for_format(format: Format, width: u64, height: u64) -> u64 {
    block_size_for_format(format) as u64 * width * height
}

/// Return the size in bytes of a 3 dimensional resource: width * height * depth block size
pub fn size_for_format(format: Format, width: u64, height: u64, depth: u32) -> u64 {
    block_size_for_format(format) as u64 * width * height * depth as u64
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

impl From<std::ffi::NulError> for Error {
    fn from(err: std::ffi::NulError) -> Error {
        let v = err.into_vec();
        Error {
            msg: String::from_utf8(v).unwrap(),
        }
    }
}

impl From<WriteMask> for u8 {
    fn from(mask: WriteMask) -> u8 {
        mask.bits
    }
}

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