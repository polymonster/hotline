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

/// Signifies how this buffer will be used on the GPU.
pub enum BufferUsage {
    Vertex,
    Index,
}

/// Information to create a buffer through `Device::create_buffer`.
pub struct BufferInfo {
    /// Indicates how the buffer will be used on the GPU.
    pub usage: BufferUsage,
    /// The stride of a vertex or structure in bytes.
    pub stride: usize,
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

/// Information to create a shader through `Device::create_shader`.
pub struct ShaderInfo {
    /// Type of the shader (Vertex, Fragment, Compute, etc...).
    pub shader_type: ShaderType,
    /// Optional info to compile from source, if this is none then
    /// the shader will be treated as a precompiled byte code blob.
    pub compile_info: Option<ShaderCompileInfo>,
}

/// Information to create a pipeline through `Device::create_pipeline`.
pub struct PipelineInfo<G: Graphics> {
    // optional vertex shader
    pub vs: Option<G::Shader>,
    // optional fragment shader
    pub fs: Option<G::Shader>,
    // optional compute shader
    pub cs: Option<G::Shader>
}

/// Graphics backends are required to implement these concrete types.
pub trait Graphics: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
    type Buffer: Buffer<Self>;
    type Shader: Shader<Self>;
    type Pipeline: Pipeline<Self>;
}

/// A GPU Buffer (Vertex, Index, Constant, etc...).
pub trait Buffer<G: Graphics>: 'static + Sized + Any {}

/// A GPU Shader (Vertex, Fragment, Compute, etc...).
pub trait Shader<G: Graphics>: 'static + Sized + Any {}

/// A GPU Pipeline 
pub trait Pipeline<G: Graphics>: 'static + Sized + Any {}

/// A GPU device is used to create GPU resources, the device also
/// contains a single a single command queue to which all command buffers will
/// submitted and executed each frame.
pub trait Device<G: Graphics>: 'static + Sized + Any {
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> G::SwapChain;
    fn create_cmd_buf(&self) -> G::CmdBuf;
    fn create_buffer(&self, info: BufferInfo, data: &[u8]) -> G::Buffer;
    fn create_shader(&self, info: ShaderInfo, data: &[u8]) -> G::Shader;
    fn create_pipeline(&self, info: PipelineInfo<G>) -> G::Pipeline;
    fn execute(&self, cmd: &G::CmdBuf);

    // tests
    fn test_mutate(&mut self);
    fn print_mutate(&self);
}

/// A swap chain is connected to a window and controls fences and signals
/// as we swap buffers.
pub trait SwapChain<G: Graphics>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn update(&mut self, device: &G::Device, window: &platform::Window);
    fn get_backbuffer_index(&self) -> i32;
    fn swap(&mut self, device: &G::Device);
}

/// Responsible for buffering graphics commands. Internally it will contain a platform specific
/// command list for each buffer in the associated swap chain.
/// At the start of each frame `reset` must be called with an associated swap chain to internally switch
/// which buffer we are writing to. At the end of each frame `close` must be called
/// and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU.
pub trait CmdBuf<G: Graphics>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &G::SwapChain);
    fn close(&self, swap_chain: &G::SwapChain);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn set_vertex_buffer(&self, buffer: &G::Buffer, slot: u32);
    fn set_pipeline_state(&self, pipeline: &G::Pipeline);
    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    );

    /// debug funcs will be removed
    fn clear_debug(&mut self, swap_chain: &G::SwapChain, r: f32, g: f32, b: f32, a: f32);
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

// TODO:
// - rust fmt line length
// - window bring to front ??
// - Enumerate adapters
// - Input Layout
// - Raster State
// - Depth Stencil State
// - Blend State
// - Topology
// - Shaders from IR
// - docs on website

// TODO:
// - pmfx Shaders
// - pmfx Input Layout

// DONE:
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

