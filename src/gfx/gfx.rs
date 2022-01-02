use std::{any::Any};

#[cfg(target_os = "windows")]
use win32 as platform;

pub struct Viewport {
    pub x : f32,
    pub y : f32,
    pub width : f32,
    pub height : f32,
    pub min_depth : f32,
    pub max_depth : f32
}

pub struct ScissorRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32
}

pub trait Graphics: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
}

// TODO: needs? + Send + Sync
pub trait Device<G: Graphics>: 'static + Sized + Any {
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> G::SwapChain;
    fn create_cmd_buf(&self) -> G::CmdBuf;
    fn execute(&self, cmd: &G::CmdBuf);

    // tests
    fn test_mutate(&mut self);
    fn print_mutate(&self);
}

// TODO: needs? + Send + Sync
pub trait SwapChain <G: Graphics>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn update(&mut self, device: &G::Device, window: &platform::Window);
    fn get_frame_index(&self) -> i32;
    fn swap(&mut self, device: &G::Device);
}

pub trait CmdBuf <G: Graphics>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &G::SwapChain);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn draw_instanced(&self, vertex_count: u32, instance_count: u32, start_vertex: u32, start_instance: u32);

    // debug funcs 
    fn clear_debug(&self, swap_chain: &G::SwapChain, r: f32, g: f32, b: f32, a: f32);
    fn set_state_debug(&self);
    fn close_debug(&self, swap_chain: &G::SwapChain);
}

// TODO:
// x Viewport
// x Scissor
// x Bind Viewport
// x Bind Scissor
// x Draw Call
// - Resize Swap Chain
// - Vsync not working?

// TODO:
// - Buffer
// - Create Buffer
// - Bind Vertex Buffer

// TODO:
// - PSO
// - Shaders
// - Input Layout
// - Raster State
// - Depth Stencil State
// - Blend State
// - Topology
// - Sample

// TODO:
// - pmfx Shaders
// - pmfx Input Layout
