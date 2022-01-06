use os::Instance;
use os::Window;

use gfx::Device;
use gfx::SwapChain;
use gfx::CmdBuf;

use std::sync::Arc;
use std::thread;
use std::time;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
use win32 as platform;

pub struct Toot {
    value: Mutex<i32>
}

pub struct ClearCol {
    r: f32,
    g: f32,
    b: f32,
    a: f32
}

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    unsafe {
        ::std::slice::from_raw_parts(
            (p as *const T) as *const u8,
            ::std::mem::size_of::<T>(),
        )
    }
}

fn main() {
    let instarc = platform::Instance::create();
    let dev = d3d12::Device::create();

    let mut win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

    let magenta = ClearCol {
        r: 1.0,
        g: 0.0, 
        b: 1.0,
        a: 1.0
    };

    let yellow = ClearCol {
        r: 1.0,
        g: 1.0, 
        b: 0.0,
        a: 1.0
    };

    let cyan = ClearCol {
        r: 0.0,
        g: 1.0, 
        b: 1.0,
        a: 1.0
    };

    let green = ClearCol {
        r: 0.0,
        g: 1.0, 
        b: 0.0,
        a: 1.0
    };

    let clears : [ClearCol; 4] = [
        magenta, yellow, cyan, green
    ];

    let vertices = [
        Vertex {
            position: [0.0, 0.25, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.25, -0.25, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.25, -0.25, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Vertex,
        stride: std::mem::size_of::<Vertex>()
    };

    let vertex_buffer = dev.create_buffer(
        info,
        any_as_u8_slice(&vertices)
    );

    let mut ci = 0;
    let mut incr = 0;

    while instarc.run() {
        win.update();
        swap_chain.update(&dev, &win);

        let window_rect = win.get_rect();

        let viewport = gfx::create_viewporti(window_rect, 0.0, 1.0);
        let scissor = gfx::create_scissor_rect(window_rect);
        
        cmdbuffer.reset(&swap_chain);

        let col = &clears[ci];
        cmdbuffer.clear_debug(&swap_chain, col.r, col.g, col.b, col.a); //

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);

        cmdbuffer.set_state_debug(); //
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
        cmdbuffer.draw_instanced(3, 1, 0, 0);

        cmdbuffer.close_debug(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        ci = (ci + 1) % 4;
        incr = incr + 1;
    }
}

#[test]
fn create_instance() {
    let _inst = platform::Instance::create();
}

#[test]
fn create_d3d12_device() {
    let _inst = platform::Instance::create();
    let _dev = d3d12::Device::create();
}

#[test]
fn create_window() {
    let inst = platform::Instance::create();
    let win = inst.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 0);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn window_set_rect() {
    let inst = platform::Instance::create();
    let mut win = inst.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    win.set_rect(os::Rect {
        x : 200,
        y : 0,
        width : 1280,
        height : 720
    });
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 200);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn swap_chain_buffer() {
    let inst = platform::Instance::create();
    let dev = d3d12::Device::create();
    let mut win = inst.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

    let clears_colours : [ClearCol; 4] = [
        ClearCol {r: 1.0, g: 0.0, b: 1.0, a: 1.0}, 
        ClearCol {r: 1.0,g: 1.0, b: 0.0,a: 1.0}, 
        ClearCol {r: 0.0,g: 1.0, b: 1.0,a: 1.0}, 
        ClearCol {r: 0.0,g: 1.0, b: 0.0,a: 1.0}
    ];

    let mut i = 0;
    while inst.run() {
        win.update();
        swap_chain.update(&dev, &win);

        cmdbuffer.reset(&swap_chain);

        let col = &clears_colours[i];
        cmdbuffer.clear_debug(&swap_chain, col.r, col.g, col.b, col.a);
        cmdbuffer.close_debug(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        i = (i + 1) % clears_colours.len();
    }
}