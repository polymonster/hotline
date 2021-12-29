use std::{any::Any};

#[cfg(target_os = "windows")]
use win32 as platform;

pub trait Graphics: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
    type SwapChain: SwapChain<Self>;
    type CmdBuf: CmdBuf<Self>;
}

// needs? + Send + Sync
pub trait Device<G: Graphics>: 'static + Sized + Any {
    fn create() -> Self;
    fn create_swap_chain(&self, window: &platform::Window) -> G::SwapChain;
    fn create_cmd_buf(&self) -> G::CmdBuf;
    fn execute(&self, cmd: &G::CmdBuf);

    // tests
    fn test_mutate(&mut self);
    fn print_mutate(&self);
}

// needs? + Send + Sync
pub trait SwapChain <G: Graphics>: 'static + Sized + Any {
    fn new_frame(&mut self);
    fn get_frame_index(&self) -> i32;
    fn swap(&mut self, device: &G::Device);
}

pub trait CmdBuf <G: Graphics>: 'static + Sized + Any {
    fn reset(&mut self, swap_chain: &G::SwapChain);
    fn clear_debug(&self, swap_chain: &G::SwapChain, r: f32, g: f32, b: f32, a: f32);
}