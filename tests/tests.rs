use hotline::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;
use os::App;
use os::Window;

use std::env;
use std::fs;

use gfx::d3d12 as gfx_platform;
#[cfg(target_os = "windows")]
use os::win32 as os_platform;

pub struct ClearCol {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[test]
fn create_instance() {
    let _inst = os_platform::App::create();
}

#[test]
fn create_d3d12_device() {
    let _inst = os_platform::App::create();
    let _dev = gfx_platform::Device::create();
}

#[test]
fn create_window() {
    let inst = os_platform::App::create();
    let win = inst.create_window(os::WindowInfo {
        title: String::from("hello world!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
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
    let inst = os_platform::App::create();
    let mut win = inst.create_window(os::WindowInfo {
        title: String::from("hello world!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    win.set_rect(os::Rect {
        x: 200,
        y: 0,
        width: 1280,
        height: 720,
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
    let inst = os_platform::App::create();
    let dev = gfx_platform::Device::create();
    let mut win = inst.create_window(os::WindowInfo {
        title: String::from("swap chain buffering"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    win.bring_to_front();

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

    let clears_colours: [ClearCol; 4] = [
        ClearCol {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        ClearCol {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        ClearCol {
            r: 0.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        ClearCol {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    ];

    let mut i = 0;
    while inst.run() {
        win.update();
        swap_chain.update(&dev, &win);

        cmdbuffer.reset(&swap_chain);

        let col = &clears_colours[i];
        cmdbuffer.clear_debug(&swap_chain, col.r, col.g, col.b, col.a);
        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        i = (i + 1) % clears_colours.len();
    }
}

#[test]
fn draw_triangle() {
    let app = os_platform::App::create();
    let dev = gfx_platform::Device::create();

    let mut win = app.create_window(os::WindowInfo {
        title: String::from("triangle!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

    let magenta = ClearCol {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    let yellow = ClearCol {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    let cyan = ClearCol {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    let green = ClearCol {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    let clears: [ClearCol; 4] = [magenta, yellow, cyan, green];

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
        stride: std::mem::size_of::<Vertex>(),
    };

    let vertex_buffer = dev.create_buffer(info, gfx::as_u8_slice(&vertices));

    let mut ci = 0;
    let mut incr = 0;

    let path = env::current_dir().unwrap();
    let shaders_hlsl_path = path.join("src\\shaders.hlsl");

    println!("The current directory is {}", path.display());
    println!("hlsl directory is {}", shaders_hlsl_path.display());

    let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain"),
            target: String::from("vs_5_0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let ps_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain"),
            target: String::from("ps_5_0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let contents = fs::read_to_string(shaders_hlsl).expect("failed to read file");

    let vs = dev.create_shader(vs_info, contents.as_bytes());
    let ps = dev.create_shader(ps_info, contents.as_bytes());

    let pso = dev.create_pipeline(gfx::PipelineInfo {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
    });

    while app.run() {
        win.update();
        swap_chain.update(&dev, &win);

        let window_rect = win.get_rect();

        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        cmdbuffer.reset(&swap_chain);

        let col = &clears[ci];
        cmdbuffer.clear_debug(&swap_chain, col.r, col.g, col.b, col.a); //

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_pipeline_state(&pso);

        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
        cmdbuffer.draw_instanced(3, 1, 0, 0);

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        ci = (ci + 1) % 4;
        incr = incr + 1;
    }
}
