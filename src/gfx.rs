use crate::os;
use std::any::Any;

/// Implemets this interface with Direct3d12 backend
pub mod d3d12;

#[cfg(target_os = "windows")]
use os::win32 as platform;

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

/// Format for resource types (textures / buffers)
#[derive(Copy, Clone)]
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
    RGBA32u,
    RGBA32i,
    RGBA32f,
}

/// Information to create a buffer through `Device::create_buffer`.
#[derive(Copy, Clone)]
pub struct BufferInfo {
    /// Indicates how the buffer will be used on the GPU.
    pub usage: BufferUsage,
    /// Data format of the buffer
    pub format: Format,
    /// The stride of a vertex or structure in bytes.
    pub stride: usize,
}

/// Describes how a buffer will be used on the GPU.
#[derive(Copy, Clone)]
pub enum BufferUsage {
    Vertex,
    Index,
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

// TODO: bitflags! bitmask is the wrong thing
bitmask! {
    /// Flags for shaders compiled at run time.
    pub mask ShaderCompileFlags: u8 where flags CompileFlags {
        /// No flags.
        None = 0b00000000,
        /// Generate debuggable shader.
        Debug = 0b00000001,
        /// Do not perform optimization to aid debugging.
        SkipOptimization = 0b00000010
    }
}

/// Descriptor layout is required to create a pipeline it describes the layout of resources for access on the GPU.
pub struct DescriptorLayout {
    pub tables: Option<Vec<DescriptorTableInfo>>,
    pub push_constants: Option<Vec<PushConatntInfo>>,
    pub static_samplers: Option<Vec<SamplerInfo>>,
}

/// Describes a range of resources for access on the GPU.
pub struct DescriptorTableInfo {
    /// The shader stage the resources will be accessible to
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader)
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader)
    pub register_space: u32,
    /// Type of resources in this table
    pub table_type: DescriptorTableType,
    /// Number of descriptors in this table, use `None` for unbounded
    pub num_descriptors: Option<u32>,
}

/// Describes the type of descriptor table to create.
pub enum DescriptorTableType {
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
#[derive(Copy, Clone)]
pub enum ShaderVisibility {
    All,
    Vertex,
    Fragment,
}

/// Describes space in the shader to send data to via `CmdBuf::push_constants`.
pub struct PushConatntInfo {
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
type InputLayout = Vec<InputElementInfo>;

/// Describe a single element of an `InputLayoutInfo`
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
#[derive(Copy, Clone)]
pub enum InputSlotClass {
    PerVertex,
    PerInstance,
}

/// Info to create a sampler state object to sample textures in shaders.
#[derive(Copy, Clone)]
pub struct SamplerInfo {
    /// The shader stage the sampler will be accessible to
    pub visibility: ShaderVisibility,
    /// Register index to bind to (supplied in shader)
    pub shader_register: u32,
    /// Register space to bind to (supplied in shader)
    pub register_space: u32,
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
#[derive(Copy, Clone)]
pub enum SamplerFilter {
    Point,
    Linear,
    Anisotropic,
}

/// Address mode for the sampler (controls wrapping and clamping).
#[derive(Copy, Clone)]
pub enum SamplerAddressMode {
    Wrap,
    Mirror,
    Clamp,
    Border,
    MirrorOnce,
}

/// Used for comparison ops in depth testing, samplers.
#[derive(Copy, Clone)]
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

/// Information to create a pipeline through `Device::create_pipeline`.
pub struct PipelineInfo<D: Device> {
    /// Vertex Shader
    pub vs: Option<D::Shader>,
    /// Fragment Shader
    pub fs: Option<D::Shader>,
    /// Compute Shader
    pub cs: Option<D::Shader>,
    pub input_layout: InputLayout,
    pub descriptor_layout: DescriptorLayout,
}

/// Information to create a pipeline through `Device::create_texture`.
#[derive(Copy, Clone)]
pub struct TextureInfo {
    pub tex_type: TextureType,
    pub width: u64,
    pub height: u64,
    pub depth: u32,
    pub array_levels: u32,
    pub mip_levels: u32,
    pub samples: u32,
}

/// Describes the dimension of a texture
#[derive(Copy, Clone)]
pub enum TextureType {
    Texture1D,
    Texture2D,
    Texture3D,
}

#[derive(Copy, Clone)]
pub struct ClearColour {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32
}

type ClearDepth = f32;

pub struct RenderPassInfo<D: Device> {
    /// Array of textures which have been created with render target flags
    pub render_targets: Vec<D::Texture>,
    /// Colour to clear render target when the pass starts, use None to preserve previous contents
    pub rt_clear: Option<ClearColour>,
    /// A texture which was created with depth stencil flags
    pub depth_stencil_target: Option<D::Texture>,
    /// Depth value (in view) to clear depth stencil, use None to preserve previous contents
    pub ds_clear: Option<ClearDepth>,
    /// Choose to resolve multi-sample AA targets,
    pub resolve: bool,
    /// (must also specify None to clear). This can save having to Load conents from main memory
    pub discard: bool
}

/// An opaque Buffer type
pub trait Buffer<D: Device>: 'static + Sized + Any {}
/// An opaque Shader type
pub trait Shader<D: Device>: 'static + Sized + Any {}
/// An opaque Pipeline type
pub trait Pipeline<D: Device>: 'static + Sized + Any {}
/// An opaque Texture type
pub trait Texture<D: Device>: 'static + Sized + Any {}
/// An opaque RenderPass type
pub trait RenderPass<D: Device>: 'static + Sized + Any {}

/// A GPU device is used to create GPU resources, the device also contains a single a single command queue
/// to which all command buffers will submitted and executed each frame.
pub trait Device: 'static + Sized + Any {
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
    type Buffer: Buffer<Self>;
    type Shader: Shader<Self>;
    type Pipeline: Pipeline<Self>;
    type Texture: Texture<Self>;
    type ReadBackRequest: ReadBackRequest<Self>;
    type RenderPass: RenderPass<Self>;
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> Self::SwapChain;
    fn create_cmd_buf(&self) -> Self::CmdBuf;
    fn create_buffer<T: Sized>(&self, info: BufferInfo, data: &[T]) -> Self::Buffer;
    fn create_texture<T: Sized>(&self, info: TextureInfo, data: &[T]) -> Self::Texture;
    fn create_shader<T: Sized>(&self, info: ShaderInfo, data: &[T]) -> Self::Shader;
    fn create_pipeline(&self, info: PipelineInfo<Self>) -> Self::Pipeline;
    fn create_render_pass(&self, info: RenderPassInfo<Self>) -> Self::RenderPass;
    fn execute(&self, cmd: &Self::CmdBuf);
}

/// A swap chain is connected to a window, controls fences and signals as we swap buffers.
pub trait SwapChain<D: Device>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn update(&mut self, device: &D, window: &platform::Window, cmd: &mut D::CmdBuf);
    fn get_backbuffer_index(&self) -> i32;
    fn get_backbuffer_texture(&self) -> &D::Texture;
    fn swap(&mut self, device: &D);
}

/// Responsible for buffering graphics commands. Internally it will contain a platform specific
/// command list for each buffer in the associated swap chain.
/// At the start of each frame `reset` must be called with an associated swap chain to internally switch
/// which buffer we are writing to. At the end of each frame `close` must be called
/// and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU.
pub trait CmdBuf<D: Device>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &D::SwapChain);
    fn close(&mut self, swap_chain: &D::SwapChain);
    fn begin_render_pass(&self, render_pass: &mut D::RenderPass);
    fn end_render_pass(&self);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn set_index_buffer(&self, buffer: &D::Buffer);
    fn set_vertex_buffer(&self, buffer: &D::Buffer, slot: u32);
    fn set_pipeline_state(&self, pipeline: &D::Pipeline);
    /// pushes constants directly to the root signature
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
    fn read_back_backbuffer(&mut self, swap_chain: &D::SwapChain) -> D::ReadBackRequest;

    /// debug funcs will be removed
    fn clear_debug(&mut self, swap_chain: &D::SwapChain, r: f32, g: f32, b: f32, a: f32);
    fn debug_set_descriptor_heap(&self, device: &D, tex: &D::Texture);
}

/// Used to readback data from the GPU, once the request is issued `is_complete` needs to be waited on for completion
/// you must poll this every frame and not block so the GPU can flush the request. Once the result is ready the
/// data can be obtained using `get_data`
pub trait ReadBackRequest<D: Device>: 'static + Sized + Any {
    fn is_complete(&self, swap_chain: &D::SwapChain) -> bool;
    fn get_data(&self) -> Result<ReadBackData, &str>;
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

/// Utility function to take any sized type and return a u8 slice.
/// This can be useful to pass `data` to `Device::create_buffer`.
pub fn as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>())
    }
}

/// Returns the block size (texel, compressed block of texels or single buffer element) for a given format
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
        Format::RGB32u => 12,
        Format::RGB32i => 12,
        Format::RGB32f => 12,
        Format::RGBA32u => 16,
        Format::RGBA32i => 16,
        Format::RGBA32f => 16,
    }
}

/// Aligns value to the alignment specified by align. value must be a power of 2
pub fn align_pow2(value: u64, align: u64) -> u64 {
    value + (align - 1) & !(align - 1)
}

/// Aligns value to the alignment specified by align. valu can be non-power of 2
pub fn align(value: u64, align: u64) -> u64 {
    let div = value / align;
    let rem = value % align;
    if rem != 0 {
        return (div + 1) * align;
    }
    value
}

// TODO:
// - validation checks on buffer and texture data used in create functions

// - Render Passes
// - Topology
// - Bindless texture array
// - Sampler
// - Constant Buffer
// - Raster State
// - Depth Stencil State
// - Blend State
// - docs on website
// - Enumerate adapters

// - Shaders from IR
// - pmfx Shaders
// - pmfx Input Layout
// - pmfx Descriptor Layout

// DONE:
// x Root Signature == DescriptorLayout
// x Pipeline->RootSignature
// x    Input Layout
// x    Static Samplers
// x    Push Constants
// x Track transitions and manually drop
// x Push constants
// x viewport rect position must be stomped to 0
// x Triangle as test (fix shader compile issue)
// x Texture
// x Backbuffer readback / resource readback
// x how to properly use bitmask and flags?
// x remove "Graphics" and move "Instance" to "App"
// x Index Buffer
// x rust fmt line length
// x samples
// x PSO
// x Shaders from source
// x Viewport
// x Scissor
// x Bind Viewport
// x Bind Scissor
// x Draw Call
// x Resize Swap Chain
// x Vsync not working?
// x Buffer
// x Create Buffer
// x Bind Vertex Buffer
// x move tests
// x move files / modules / libs
// x docs
