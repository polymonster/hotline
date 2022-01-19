use hotline::*;

use os::App;
use os::Window;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

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
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("window_set_rect"),
        window: false,
        num_buffers: 0,
    });
    main_index_buffer(app);

    // draw_triangle();
}

fn main_index_buffer(app: os_platform::App) {
    let dev = gfx_platform::Device::create();

    let mut win = app.create_window(os::WindowInfo {
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
            color: [0.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-1.0, 1.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [1.0, 1.0, 0.0],
            color: [1.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [1.0, -1.0, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
    ];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Vertex,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
    };

    let vertex_buffer = dev.create_buffer(info, gfx::as_u8_slice(&vertices));

    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Index,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<Vertex>(),
    };

    let index_buffer = dev.create_buffer(info, gfx::as_u8_slice(&indices));

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let shaders_hlsl_path = asset_path.join("..\\..\\samples\\hello_world\\shaders.hlsl");
    let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain\0"),
            target: String::from("vs_5_1\0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let ps_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain\0"),
            target: String::from("ps_5_1\0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let contents = fs::read_to_string(shaders_hlsl).expect("failed to read file");

    let vs = dev.create_shader(vs_info, contents.as_bytes());
    let ps = dev.create_shader(ps_info, contents.as_bytes());

    let vs_position = gfx::InputElementInfo {
        semantic: String::from("POSITION\0"),
        index: 0,
        format: gfx::Format::RGB32f,
        input_slot: 0,
        aligned_byte_offset: 0,
        input_slot_class: gfx::InputSlotClass::PerVertex,
        step_rate: 0
    };

    let vs_color = gfx::InputElementInfo {
        semantic: String::from("COLOR\0"),
        index: 0,
        format: gfx::Format::RGBA32f,
        input_slot: 0,
        aligned_byte_offset: 12,
        input_slot_class: gfx::InputSlotClass::PerVertex,
        step_rate: 0
    };

    let input_layout = vec![vs_position, vs_color];

    let pso = dev.create_pipeline(gfx::PipelineInfo {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
        input_layout: input_layout,
        descriptor_layout: None,
    });

    // tex
    let nam = String::from("../../samples/hello_world/redchecker01.png");
    let image = image::load_from_file(nam);

    let tex_info = gfx::TextureInfo {
        tex_type: gfx::TextureType::Texture2D,
        width: image.width,
        height: image.height,
        depth: 1,
        array_levels: 1,
        mip_levels: 1,
        samples: 1,
    };

    let texture = dev.create_texture(tex_info, image.data.as_slice());

    let constants: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

    let mut ci = 0;
    while app.run() {
        win.update();
        swap_chain.update(&dev, &win, &mut cmdbuffer);

        let vp_rect = win.get_viewport_rect();

        let viewport = gfx::Viewport::from(vp_rect);
        let scissor = gfx::ScissorRect::from(vp_rect);

        cmdbuffer.reset(&swap_chain);

        cmdbuffer.clear_debug(&swap_chain, magenta.r, magenta.g, magenta.b, magenta.a); //

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_pipeline_state(&pso);

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);

        cmdbuffer.debug_set_descriptor_heap(&dev, &texture);

        cmdbuffer.push_constants(0, 4, 0, constants.as_slice());

        cmdbuffer.draw_indexed_instanced(6, 1, 0, 0, 0);

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);

        ci = (ci + 1) % 4;
    }

    // must wait for the final frame to be completed
    cmdbuffer.reset(&swap_chain);
}

fn draw_triangle() {
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("draw_triangle"),
        window: false,
        num_buffers: 0,
    });
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
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
    };

    let vertex_buffer = dev.create_buffer(info, gfx::as_u8_slice(&vertices));

    let src = "
        struct PSInput
        {
            float4 position : SV_POSITION;
            float4 color : COLOR;
        };

        PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
        {
            PSInput result;

            result.position = position;
            result.color = color;

            return result;
        }

        float4 PSMain(PSInput input) : SV_TARGET
        {
            return input.color;
        }\0";

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain\0"),
            target: String::from("vs_5_0\0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let ps_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain\0"),
            target: String::from("ps_5_0\0"),
            flags: gfx::ShaderCompileFlags::none(),
        }),
    };

    let vs = dev.create_shader(vs_info, src.as_bytes());
    let ps = dev.create_shader(ps_info, src.as_bytes());

    let vs_position = gfx::InputElementInfo {
        semantic: String::from("POSITION\0"),
        index: 0,
        format: gfx::Format::RGB32f,
        input_slot: 0,
        aligned_byte_offset: 0,
        input_slot_class: gfx::InputSlotClass::PerVertex,
        step_rate: 0
    };

    let vs_color = gfx::InputElementInfo {
        semantic: String::from("COLOR\0"),
        index: 0,
        format: gfx::Format::RGBA32f,
        input_slot: 0,
        aligned_byte_offset: 12,
        input_slot_class: gfx::InputSlotClass::PerVertex,
        step_rate: 0
    };

    let input_layout = vec![vs_position, vs_color];

    let pso = dev.create_pipeline(gfx::PipelineInfo {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
        input_layout: input_layout,
        descriptor_layout: None,
    });

    /*
    let mut rbr = gfx_platform::ReadBackRequest {
        fence_value: u64::MAX,
        resource: None,
        size: 0,
        row_pitch: 0,
        slice_pitch: 0,
    };

    let mut written = false;
    let mut ci = 0;
    let mut count = 0;
    while app.run() {
        win.update();
        swap_chain.update(&dev, &win, &mut cmdbuffer);

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

        /*
        if !rbr.resource.is_some() && !written {
            rbr = cmdbuffer.read_back_backbuffer(&swap_chain);
        } else {
            if rbr.is_complete(&swap_chain) && rbr.resource.is_some() {
                let data = rbr.get_data().unwrap();
                image::write_to_file(String::from("my_triangle"), 1280, 720, 4, &data.data)
                    .unwrap();

                rbr.resource = None;
                written = true;
            }
        }
        */

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        ci = (ci + 1) % 4;
        count = count + 1;

        if count > 16 {
            break;
        }
    }

    cmdbuffer.reset(&swap_chain);
    */
}