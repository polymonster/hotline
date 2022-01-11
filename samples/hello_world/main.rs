use hotline::*;

use os::Instance;
use os::Window;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use std::env;
use std::fs;

#[cfg(target_os = "windows")]
use hotline::os::win32 as os_platform;

#[cfg(target_os = "windows")]
use hotline::gfx::d3d12 as gfx_platform;

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

fn main() {
    let instarc = os_platform::Instance::create();
    main_index_buffer(instarc);
}

fn main_index_buffer(instarc: os_platform::Instance) {
    let dev = gfx_platform::Device::create();

    let mut win = instarc.create_window(os::WindowInfo {
        title: String::from("index buffer!"),
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

    let vertices = [
        Vertex {
            position: [-1.0, -1.0, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-1.0, 1.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
        Vertex {
            position: [1.0, -1.0, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Vertex,
        stride: std::mem::size_of::<Vertex>(),
    };

    let vertex_buffer = dev.create_buffer(info, gfx::as_u8_slice(&vertices));

    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Index,
        stride: std::mem::size_of::<Vertex>(),
    };

    let index_buffer = dev.create_buffer(info, gfx::as_u8_slice(&indices));

    let mut ci = 0;
    let mut incr = 0;

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let shaders_hlsl_path = asset_path.join("..\\..\\src\\shaders.hlsl");
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

    let pso_info = gfx::PipelineInfo::<gfx::d3d12::Graphics> {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
    };

    let pso = dev.create_pipeline(pso_info);

    // tex

    let tex_info = gfx::TextureInfo {
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_levels: 1,
        mip_levels: 1,
        samples: 1,
    };
    dev.create_texture(tex_info, contents.as_bytes());

    while instarc.run() {
        win.update();
        swap_chain.update(&dev, &win);

        let window_rect = win.get_rect();

        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        cmdbuffer.reset(&swap_chain);

        cmdbuffer.clear_debug(&swap_chain, magenta.r, magenta.g, magenta.b, magenta.a); //

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_pipeline_state(&pso);

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);

        cmdbuffer.draw_indexed_instanced(6, 1, 0, 0, 0);

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        ci = (ci + 1) % 4;
        incr = incr + 1;
    }
}

fn main_test(instarc: os_platform::Instance) {
    let dev = gfx_platform::Device::create();

    let mut win = instarc.create_window(os::WindowInfo {
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

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let shaders_hlsl_path = asset_path.join("..\\..\\src\\shaders.hlsl");
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

    let pso_info = gfx::PipelineInfo::<gfx::d3d12::Graphics> {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
    };

    let pso = dev.create_pipeline(pso_info);

    while instarc.run() {
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
