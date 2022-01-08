pub mod d3d12;

use std::{any::Any};
use crate::os;

#[cfg(target_os = "windows")]
use os::win32 as platform;

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

/*
#[macro_use]
extern crate bitmask;
bitmask! {
    pub mask CpuAccessFlags: u8 where flags Access {
        none = 0b00000000,
        read = 0b00000001,
        write = 0b00000010
    }
}
*/

pub enum BufferUsage {
    Vertex,
    Index
}

pub struct BufferInfo {
    pub usage: BufferUsage,
    pub stride: usize
}

pub trait Graphics: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
    type Buffer: Buffer<Self>;
}

pub trait Device<G: Graphics>: 'static + Sized + Any {
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> G::SwapChain;
    fn create_cmd_buf(&self) -> G::CmdBuf;
    fn create_buffer(&self, info: BufferInfo, data: &[u8]) -> G::Buffer;
    fn execute(&self, cmd: &G::CmdBuf);

    // tests
    fn test_mutate(&mut self);
    fn print_mutate(&self);
}

pub trait SwapChain <G: Graphics>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn update(&mut self, device: &G::Device, window: &platform::Window);
    fn get_frame_index(&self) -> i32;
    fn swap(&mut self, device: &G::Device);
}

pub trait CmdBuf <G: Graphics>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &G::SwapChain);
    fn reset_all(&mut self);
    fn set_viewport(&self, viewport: &Viewport);
    fn set_scissor_rect(&self, scissor_rect: &ScissorRect);
    fn set_vertex_buffer(&self, buffer: &G::Buffer, slot: u32);
    fn draw_instanced(&self, vertex_count: u32, instance_count: u32, start_vertex: u32, start_instance: u32);

    // debug funcs 
    fn clear_debug(&mut self, swap_chain: &G::SwapChain, r: f32, g: f32, b: f32, a: f32);
    fn set_state_debug(&self);
    fn close_debug(&self, swap_chain: &G::SwapChain);
}

pub trait Buffer <G: Graphics>: 'static + Sized + Any {

}

impl From<os::Rect<i32>> for Viewport {
    fn from(rect: os::Rect<i32>) -> Viewport{
        Viewport {
            x: rect.x as f32,
            y: rect.y as f32,
            width: rect.width as f32,
            height: rect.height as f32,
            min_depth: 0.0,
            max_depth: 1.0
        } 
    }
}

impl From<os::Rect<i32>> for ScissorRect {
    fn from(rect: os::Rect<i32>) -> ScissorRect{
        ScissorRect {
            left: rect.x,
            top: rect.y,
            right: rect.width,
            bottom: rect.height
        }
    }
}

pub fn as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p as *const T) as *const u8,
            ::std::mem::size_of::<T>(),
        )
    }
}


// TODO:
// - rust fmt
// - window bring to front

// TODO:
// - Shaders
// - Input Layout
// - Raster State
// - Depth Stencil State
// - Blend State
// - Topology
// - PSO

// TODO:
// - pmfx Shaders
// - pmfx Input Layout

// TODO:
// - samples 

// TODO:
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

