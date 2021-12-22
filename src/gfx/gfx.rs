use std::{any::Any};

#[cfg(target_os = "windows")]
use win32 as platform;

pub trait Graphics: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
    type Queue: Queue<Self>;
    //type CmdBuf: CmdBuf<Self>;
}

// needs? + Send + Sync
pub trait Device<G: Graphics>: 'static + Sized + Any {
    fn create() -> Self;
    fn create_queue(&self) -> G::Queue;
    // fn create_cmd_buf(&self) -> G::CmdBuf;
}

// needs? + Send + Sync
pub trait Queue <G: Graphics>: 'static + Sized + Any {
    fn create_swap_chain(&self, device: G::Device, window: platform::Window);
    //fn execute(&self, cmd: G::CmdBuf);
}

pub trait CmdBuf <G: Graphics>: 'static + Sized + Any {
}