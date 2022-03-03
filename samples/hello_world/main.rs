use hotline::*;

use os::App;
use os::Window;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use std::fs;

#[cfg(target_os = "windows")]
use hotline::os::win32 as os_platform;
use hotline::gfx::d3d12 as gfx_platform;

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
    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100
    });
    print!("{}", dev.get_adapter_info());

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
    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n
    };
    let mut swap_chain = dev.create_swap_chain(&swap_chain_info, &win);

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();

    // cmd buffer
    let mut cmdbuffer = dev.create_cmd_buf(2);

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
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 4,
    };

    let vertex_buffer = dev.create_buffer(&info, Some(gfx::as_u8_slice(&vertices))).unwrap();

    // index buffer
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::Index,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<u16>(),
        num_elements: 6,
    };

    let index_buffer = dev.create_buffer(&info, Some(gfx::as_u8_slice(&indices))).unwrap();

    // shaders
    let shaders_hlsl_path = asset_path.join("..\\..\\samples\\hello_world\\shaders.hlsl");
    let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain"),
            target: String::from("vs_5_1"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let fs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain"),
            target: String::from("ps_5_1"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let cs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("CSMain"),
            target: String::from("cs_5_1"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let contents = fs::read_to_string(shaders_hlsl).expect("failed to read file");

    let vs = dev.create_shader(&vs_info, contents.as_bytes()).unwrap();
    let fs = dev.create_shader(&fs_info, contents.as_bytes()).unwrap();
    let cs = dev.create_shader(&cs_info, contents.as_bytes()).unwrap();

    let num_descriptors = 10;

    // pipeline
    let pso = dev.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: Some(vs),
        fs: Some(fs),
        input_layout: vec![
            gfx::InputElementInfo {
                semantic: String::from("POSITION"),
                index: 0,
                format: gfx::Format::RGB32f,
                input_slot: 0,
                aligned_byte_offset: 0,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
            gfx::InputElementInfo {
                semantic: String::from("COLOR"),
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
            bindings: Some(vec![
                gfx::DescriptorBinding {
                    visibility: gfx::ShaderVisibility::Fragment,
                    binding_type: gfx::DescriptorType::ShaderResource,
                    num_descriptors: Some(num_descriptors),
                    shader_register: 0,
                    register_space: 0,
                },
                gfx::DescriptorBinding {
                    visibility: gfx::ShaderVisibility::Fragment,
                    binding_type: gfx::DescriptorType::ConstantBuffer,
                    num_descriptors: Some(num_descriptors),
                    shader_register: 1,
                    register_space: 0,
                },
            ]),
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
        raster_info: gfx::RasterInfo::default(),
        depth_stencil_info: gfx::DepthStencilInfo::default(),
        blend_info: gfx::BlendInfo {
            render_target: vec![
                gfx::RenderTargetBlendInfo::default()
            ],
            ..Default::default() 
        },
        topology: gfx::Topology::TriangleList,
        patch_index: 0,
        pass: swap_chain.get_backbuffer_pass()
    }).expect("failed to create pipeline!");

    let mut textures: Vec<gfx::d3d12::Texture> = Vec::new();
    let files = vec![
        asset_path.join("..\\..\\samples\\hello_world\\redchecker01.png"),
        asset_path.join("..\\..\\samples\\hello_world\\blend_test_fg.png"),
        asset_path.join("..\\..\\samples\\hello_world\\bear_stomp_anim_001.png"),
        asset_path.join("..\\..\\samples\\hello_world\\bluechecker01.png"),
    ];
    for file in files {
        let image = image::load_from_file(String::from(file.to_str().unwrap()));
        let tex_info = gfx::TextureInfo {
            format: gfx::Format::RGBA8n,
            tex_type: gfx::TextureType::Texture2D,
            width: image.width,
            height: image.height,
            depth: 1,
            array_levels: 1,
            mip_levels: 1,
            samples: 1,
            usage: gfx::TextureUsage::SHADER_RESOURCE,
            initial_state: gfx::ResourceState::ShaderResource
        };
        let tex = dev.create_texture(&tex_info, Some(image.data.as_slice())).unwrap();
        textures.push(tex);
    }

    // push constants
    let constants: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

    // constant buffer
    let mut cbuffer: [f32; 64] = [0.0; 64];
    cbuffer[0] = 1.0;
    cbuffer[1] = 1.0;
    cbuffer[2] = 1.0;
    cbuffer[3] = 1.0;

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::ConstantBuffer,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: cbuffer.len() * 4,
        num_elements: 1,
    };

    let _constant_buffer = dev.create_buffer(&info, Some(gfx::as_u8_slice(&cbuffer)));

    // render target
    let rt_info = gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_levels: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::RENDER_TARGET,
        initial_state: gfx::ResourceState::ShaderResource
    };
    let render_target = dev.create_texture::<u8>(&rt_info, None).unwrap();

    // depth stencil target
    let ds_info = gfx::TextureInfo {
        format: gfx::Format::D24nS8u,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_levels: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::DEPTH_STENCIL,
        initial_state: gfx::ResourceState::DepthStencil
    };
    let depth_stencil = dev.create_texture::<u8>(&ds_info, None).unwrap();

    // pass for render target with depth stencil
    let mut render_target_pass = dev.create_render_pass(&gfx::RenderPassInfo {
        render_targets: vec![render_target.clone()],
        rt_clear: Some(gfx::ClearColour {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        }),
        depth_stencil: Some(depth_stencil.clone()),
        ds_clear: Some( gfx::ClearDepthStencil {
            depth: Some(1.0),
            stencil: None
        }),
        resolve: false,
        discard: false,
    }).unwrap();

    // unordered access rw texture
    let rw_info = gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_levels: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::UNORDERED_ACCESS,
        initial_state: gfx::ResourceState::ShaderResource
    };
    let rw_tex = dev.create_texture::<u8>(&rw_info, None).unwrap();

    let compute_pipeline = dev.create_compute_pipeline(&gfx::ComputePipelineInfo{
        cs: cs,
        descriptor_layout: gfx::DescriptorLayout {
            static_samplers: None,
            push_constants: None,
            bindings: Some(vec![
                gfx::DescriptorBinding {
                    visibility: gfx::ShaderVisibility::Compute,
                    binding_type: gfx::DescriptorType::UnorderedAccess,
                    num_descriptors: Some(num_descriptors),
                    shader_register: 0,
                    register_space: 0,
                },
            ]),
        }
    }).unwrap();

    //
    imgui::setup(&imgui::ImGuiInfo {
        main_window: win.as_mut_ptr(),
        swap_chain: swap_chain.as_mut_ptr(),
        device: dev.as_mut_ptr(),
        fonts: vec![
            asset_path.join("..\\..\\samples\\hello_world\\Roboto-Medium.ttf").to_str().unwrap().to_string()
        ]
    });

    // ..
    let mut ci = 0;
    while app.run() {
        win.update();
        swap_chain.update(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // compute pass
        cmdbuffer.set_marker(0xff00ffff, "START!!!");

        cmdbuffer.begin_event(0xff0000ff, "Compute Pass");
        cmdbuffer.set_compute_pipeline(&compute_pipeline);
        cmdbuffer.set_compute_heap(0, dev.get_shader_heap());
        cmdbuffer.dispatch(
            gfx::Size3 {
                x:512/16, y:512/16, z:1
            },
            gfx::Size3 {
                x:512, y:512, z:1
            },
        );
        cmdbuffer.end_event();

        // render target pass
        cmdbuffer.begin_event(0xff0000ff, "Render Target Pass");
        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(render_target.clone()),
            buffer: None,
            state_before: gfx::ResourceState::ShaderResource,
            state_after: gfx::ResourceState::RenderTarget,
        });
        
        cmdbuffer.begin_render_pass(&mut render_target_pass);
        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(render_target.clone()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::ShaderResource,
        });
        cmdbuffer.end_event();

        // main pass
        cmdbuffer.begin_event(0xff0000ff, "Main Pass");
        let vp_rect = win.get_viewport_rect();
        let viewport = gfx::Viewport::from(vp_rect);
        let scissor = gfx::ScissorRect::from(vp_rect);

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let mut pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(&mut pass);

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_render_pipeline(&pso);
        cmdbuffer.set_render_heap(1, dev.get_shader_heap(), 0);

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);

        cmdbuffer.push_constants(0, 4, 0, constants.as_slice());

        cmdbuffer.draw_indexed_instanced(6, 1, 0, 0, 0);

        // ig
        imgui::new_frame();
        imgui::demo();
        imgui::render(&mut cmdbuffer);

        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        cmdbuffer.end_event();

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);

        ci = (ci + 1) % 4;
    }

    // must wait for the final frame to be completed
    cmdbuffer.reset(&swap_chain);
}
