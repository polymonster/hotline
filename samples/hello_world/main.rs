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
    // app
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("window_set_rect"),
        window: false,
        num_buffers: 0,
    });

    // device
    let mut dev = gfx_platform::Device::create();

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("bindless texture!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    let mut swap_chain = dev.create_swap_chain(&win);

    // cmd buffer
    let mut cmdbuffer = dev.create_cmd_buf();

    // vertex buffer
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

    let vertex_buffer = dev.create_buffer(&info, gfx::as_u8_slice(&vertices));

    // index buffer
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Index,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<Vertex>(),
    };

    let index_buffer = dev.create_buffer(&info, gfx::as_u8_slice(&indices));

    // shaders
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

    let vs = dev.create_shader(&vs_info, contents.as_bytes());
    let ps = dev.create_shader(&ps_info, contents.as_bytes());

    // pipeline
    let pso = dev.create_pipeline(&gfx::PipelineInfo {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
        input_layout: vec![
            gfx::InputElementInfo {
                semantic: String::from("POSITION\0"),
                index: 0,
                format: gfx::Format::RGB32f,
                input_slot: 0,
                aligned_byte_offset: 0,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
            gfx::InputElementInfo {
                semantic: String::from("COLOR\0"),
                index: 0,
                format: gfx::Format::RGBA32f,
                input_slot: 0,
                aligned_byte_offset: 12,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
        ],
        descriptor_layout: gfx::DescriptorLayout {
            push_constants: Some(vec![gfx::PushConstantInfo {
                visibility: gfx::ShaderVisibility::Fragment,
                num_values: 4,
                shader_register: 0,
                register_space: 0,
            }]),
            tables: Some(vec![gfx::DescriptorTableInfo {
                visibility: gfx::ShaderVisibility::Fragment,
                table_type: gfx::DescriptorTableType::ShaderResource,
                num_descriptors: Some(4),
                shader_register: 0,
                register_space: 0,
            }]),
            static_samplers: Some(vec![gfx::SamplerInfo {
                visibility: gfx::ShaderVisibility::Fragment,
                filter: gfx::SamplerFilter::Linear,
                address_u: gfx::SamplerAddressMode::Wrap,
                address_v: gfx::SamplerAddressMode::Wrap,
                address_w: gfx::SamplerAddressMode::Wrap,
                comparison: None,
                border_colour: None,
                mip_lod_bias: 0.0,
                max_aniso: 0,
                min_lod: -1.0,
                max_lod: -1.0,
                shader_register: 0,
                register_space: 0,
            }]),
        },
    });

    let mut textures: Vec<gfx::d3d12::Texture> = Vec::new();
    let files = vec![
        "../../samples/hello_world/redchecker01.png",
        "../../samples/hello_world/blend_test_fg.png",
        "../../samples/hello_world/bear_stomp_anim_001.png",
        "../../samples/hello_world/bluechecker01.png",
    ];
    for file in files {
        let image = image::load_from_file(String::from(file));
        let tex_info = gfx::TextureInfo {
            format: gfx::Format::RGBA8n,
            tex_type: gfx::TextureType::Texture2D,
            width: image.width,
            height: image.height,
            depth: 1,
            array_levels: 1,
            mip_levels: 1,
            samples: 1,
        };
        textures.push(dev.create_texture(&tex_info, image.data.as_slice()));
    }

    // push constants
    let constants: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

    let mut ci = 0;
    while app.run() {
        win.update();
        swap_chain.update(&dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        let vp_rect = win.get_viewport_rect();
        let viewport = gfx::Viewport::from(vp_rect);
        let scissor = gfx::ScissorRect::from(vp_rect);

        // TODO: passes in swap chain
        let mut pass = dev.create_render_pass(&gfx::RenderPassInfo {
            render_targets: vec![swap_chain.get_backbuffer_texture().clone()],
            rt_clear: Some(gfx::ClearColour {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }),
            depth_stencil_target: None,
            ds_clear: None,
            resolve: false,
            discard: false,
        });

        cmdbuffer.transition_barrier( &gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        cmdbuffer.begin_render_pass(&mut pass);

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_pipeline_state(&pso);

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);

        cmdbuffer.debug_set_descriptor_heap(&dev);

        cmdbuffer.push_constants(0, 4, 0, constants.as_slice());

        cmdbuffer.draw_indexed_instanced(6, 1, 0, 0, 0);

        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier( &gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);

        ci = (ci + 1) % 4;
    }

    // must wait for the final frame to be completed
    cmdbuffer.reset(&swap_chain);
}
