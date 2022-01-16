/// Implemets this interface with Direct3d12 backend
pub mod d3d12;

use crate::os;

use std::any::Any;

#[cfg(target_os = "windows")]
use os::win32 as platform;

/// Structure to specify viewport coordinates on a `CmdBuf`.
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
pub struct ScissorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Format for resource types (textures / buffers)
pub enum Format {
    Unknown,
    R16n,
    R16u,
    R16i,
    R16f,
    RGBA32u,
    RGBA32i,
    RGBA32f,
}

/// Information to create a buffer through `Device::create_buffer`.
pub struct BufferInfo {
    /// Indicates how the buffer will be used on the GPU.
    pub usage: BufferUsage,
    /// Data format of the buffer
    pub format: Format,
    /// The stride of a vertex or structure in bytes.
    pub stride: usize,
}

/// Signifies how this buffer will be used on the GPU.
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

/// Information required to compile a shader from source.
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
pub enum ShaderType {
    Vertex,
    Fragment,
    Compute,
}

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

/// Information to create a pipeline through `Device::create_pipeline`.
pub struct PipelineInfo<D: Device> {
    // optional vertex shader
    pub vs: Option<D::Shader>,
    // optional fragment shader
    pub fs: Option<D::Shader>,
    // optional compute shader
    pub cs: Option<D::Shader>,
}

/// Information to create a pipeline through `Device::create_texture`.
pub struct TextureInfo {
    pub tex_type: TextureType,
    pub width: u64,
    pub height: u64,
    /// Supply only for 3D textures
    pub depth: u32,
    pub array_levels: u32,
    pub mip_levels: u32,
    /// Number of MSAA samples
    pub samples: u32,
}

pub enum TextureType {
    Texture1D,
    Texture2D,
    Texture3D,
}

/// A GPU Buffer (Vertex, Index, Constant, etc...).
pub trait Buffer<D: Device>: 'static + Sized + Any {}

/// A GPU Shader (Vertex, Fragment, Compute, etc...).
pub trait Shader<D: Device>: 'static + Sized + Any {}

/// A GPU Pipeline
pub trait Pipeline<D: Device>: 'static + Sized + Any {}

/// A GPU Texture
pub trait Texture<D: Device>: 'static + Sized + Any {}

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
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> Self::SwapChain;
    fn create_cmd_buf(&self) -> Self::CmdBuf;
    fn create_buffer(&self, info: BufferInfo, data: &[u8]) -> Self::Buffer;
    fn create_texture(&self, info: TextureInfo, data: &[u8]) -> Self::Texture;
    fn create_shader(&self, info: ShaderInfo, data: &[u8]) -> Self::Shader;
    fn create_pipeline(&self, info: PipelineInfo<Self>) -> Self::Pipeline;
    fn execute(&self, cmd: &Self::CmdBuf);
}

/// A swap chain is connected to a window and controls fences and signals as we swap buffers.
pub trait SwapChain<D: Device>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn update(&mut self, device: &D, window: &platform::Window);
    fn get_backbuffer_index(&self) -> i32;
    fn swap(&mut self, device: &D);
}

/// Responsible for buffering graphics commands. Internally it will contain a platform specific
/// command list for each buffer in the associated swap chain.
/// At the start of each frame `reset` must be called with an associated swap chain to internally switch
/// which buffer we are writing to. At the end of each frame `close` must be called
/// and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU.
pub trait CmdBuf<D: Device>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &D::SwapChain);
    fn close(&self, swap_chain: &D::SwapChain);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn set_index_buffer(&self, buffer: &D::Buffer);
    fn set_vertex_buffer(&self, buffer: &D::Buffer, slot: u32);
    fn set_pipeline_state(&self, pipeline: &D::Pipeline);
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
    fn read_back_backbuffer(&self, swap_chain: &D::SwapChain) -> D::ReadBackRequest;

    /// debug funcs will be removed
    fn clear_debug(&mut self, swap_chain: &D::SwapChain, r: f32, g: f32, b: f32, a: f32);
}

#[derive(Debug)]
pub enum ReadBackError {
    ResultNotRready,
    MapFailed,
    NullData
}

impl std::error::Error for ReadBackError {}

impl std::fmt::Display for ReadBackError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
        ReadBackError::ResultNotRready => write!(f, "Result Not Ready"),
        ReadBackError::MapFailed => write!(f, "Map Failed"),
        ReadBackError::NullData => write!(f, "Map Data is Null"),
      }
  }
}

pub struct ReadBackData {
    pub data: &'static [u8],
    pub format: Format,
    pub size: usize,
    pub row_pitch: usize,
    pub slice_pitch: usize,
}

pub trait ReadBackRequest<D: Device>: 'static + Sized + Any {
    fn is_complete(&self, swap_chain: &D::SwapChain) -> bool;
    fn get_data(&self) -> Result<ReadBackData, ReadBackError>;
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
        Format::RGBA32u => 16,
        Format::RGBA32i => 16,
        Format::RGBA32f => 16,
    }
}

pub fn align(value: u64, align: u64) -> u64 {
    value + (align - 1) & !(align - 1)
}

// TODO: lingering
// - window bring to front ?? (during tests)

// TODO: current
// - Track transitions and manually drop
// - Texture
// - Constant Buffer
// - Root Signature
// - Input Layout
// - Shaders from IR
// - Triangle as test (fix shader compile issue)

// TODO: maybe
// - pmfx Shaders
// - pmfx Input Layout

// TODO: later
// - Enumerate adapters
// - Raster State
// - Depth Stencil State
// - Blend State
// - Topology
// - docs on website

// DONE:
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
