use hotline::*;

use os::Instance;
use os::Window;
use gfx::Device;
use gfx::SwapChain;
use gfx::CmdBuf;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

pub struct ClearCol {
    r: f32,
    g: f32,
    b: f32,
    a: f32
}

#[test]
fn create_instance() {
    let _inst = os_platform::Instance::create();
}

#[test]
fn create_d3d12_device() {
    let _inst = os_platform::Instance::create();
    let _dev = gfx_platform::Device::create();
}

#[test]
fn create_window() {
    let inst = os_platform::Instance::create();
    let win = inst.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    win.bring_to_front();
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 0);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn window_set_rect() {
    let inst = os_platform::Instance::create();
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
    win.bring_to_front();
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 200);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn swap_chain_buffer() {
    let inst = os_platform::Instance::create();
    let dev = gfx_platform::Device::create();
    let mut win = inst.create_window(os::WindowInfo { 
        title : String::from("swap chain buffering"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    win.bring_to_front();

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