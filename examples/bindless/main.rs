// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain, RenderPass};

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("bindless"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // device
    let device = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });
    print!("{}", device.get_adapter_info());

    let mut dev = device;

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("bindless"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1024,
            height: 1024,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;

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
        usage: gfx::BufferUsage::VERTEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 4,
        initial_state: gfx::ResourceState::VertexConstantBuffer
    };

    let vertex_buffer = dev.create_buffer(&info, Some(gfx::as_u8_slice(&vertices)))?;

    // index buffer
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::INDEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<u16>(),
        num_elements: 6,
        initial_state: gfx::ResourceState::IndexBuffer
    };

    let index_buffer = dev.create_buffer(&info, Some(gfx::as_u8_slice(&indices)))?;

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut dev, 0);

    pmfx.load(&hotline_rs::get_data_path("shaders/bindless"))?;
    pmfx.create_compute_pipeline(&dev, "compute_rw")?;
    pmfx.create_render_pipeline(&dev, "bindless", swap_chain.get_backbuffer_pass())?;
    
    let fmt = swap_chain.get_backbuffer_pass().get_format_hash();
    let pso_pmfx = pmfx.get_render_pipeline_for_format("bindless", fmt)?;
    let pso_compute = pmfx.get_compute_pipeline("compute_rw")?;

    let mut textures: Vec<gfx_platform::Texture> = Vec::new();
    let files = vec![
        hotline_rs::get_src_data_path("textures/monsters/netrunner.jpg"),
        hotline_rs::get_src_data_path("textures/monsters/cereal_3.png"),
        hotline_rs::get_src_data_path("textures/monsters/octo.jpg"),
        hotline_rs::get_src_data_path("textures/monsters/laptop.jpg"),
    ];
    for file in files {
        let image = image::load_from_file(&file)?;
        let tex = dev.create_texture(&image.info, data![image.data.as_slice()])?;
        textures.push(tex);
    }

    // push constants
    let constants: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

    // constant buffer
    let mut cbuffer: [f32; 64] = [0.0; 64];
    cbuffer[0] = 1.0;
    cbuffer[1] = 0.0;
    cbuffer[2] = 1.0;
    cbuffer[3] = 1.0;

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::CONSTANT_BUFFER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: cbuffer.len() * 4,
        num_elements: 1,
        initial_state: gfx::ResourceState::VertexConstantBuffer
    };

    let _constant_buffer = dev.create_buffer(&info, data![gfx::as_u8_slice(&cbuffer)]);

    // render target
    let rt_info = gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::RENDER_TARGET,
        initial_state: gfx::ResourceState::ShaderResource,
    };
    let render_target = dev.create_texture(&rt_info, data![]).unwrap();

    // depth stencil target
    let ds_info = gfx::TextureInfo {
        format: gfx::Format::D24nS8u,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::DEPTH_STENCIL,
        initial_state: gfx::ResourceState::DepthStencil,
    };
    let depth_stencil = dev.create_texture::<u8>(&ds_info, None).unwrap();

    // pass for render target with depth stencil
    let render_target_pass = dev
        .create_render_pass(&gfx::RenderPassInfo {
            render_targets: vec![&render_target],
            rt_clear: Some(gfx::ClearColour {
                r: 1.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            }),
            depth_stencil: Some(&depth_stencil),
            ds_clear: Some(gfx::ClearDepthStencil {
                depth: Some(1.0),
                stencil: None,
            }),
            resolve: false,
            discard: false,
            array_slice: 0
        })
        .unwrap();

    // unordered access rw texture
    let rw_info = gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: 512,
        height: 512,
        depth: 1,
        array_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::UNORDERED_ACCESS,
        initial_state: gfx::ResourceState::ShaderResource,
    };
    let _rw_tex = dev.create_texture::<u8>(&rw_info, None).unwrap();

    // ..
    let mut ci = 0;
    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // compute pass
        cmdbuffer.set_marker(0xff00ffff, "Frame Start");

        cmdbuffer.begin_event(0xff0000ff, "Compute Pass");
        cmdbuffer.set_compute_pipeline(pso_compute);
        cmdbuffer.set_heap(pso_compute, dev.get_shader_heap());
        cmdbuffer.dispatch(
            gfx::Size3 {
                x: 512 / 16,
                y: 512 / 16,
                z: 1,
            },
            gfx::Size3 {
                x: 512,
                y: 512,
                z: 1,
            },
        );
        cmdbuffer.end_event();

        // render target pass
        cmdbuffer.begin_event(0xff0000ff, "Render Target Pass");
        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&render_target),
            buffer: None,
            state_before: gfx::ResourceState::ShaderResource,
            state_after: gfx::ResourceState::RenderTarget,
        });

        cmdbuffer.begin_render_pass(&render_target_pass);
        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&render_target),
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
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);

        cmdbuffer.set_render_pipeline(pso_pmfx);
        
        cmdbuffer.set_heap(pso_pmfx, dev.get_shader_heap());

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);

        cmdbuffer.push_render_constants(0, 4, 0, constants.as_slice());

        cmdbuffer.draw_indexed_instanced(6, 1, 0, 0, 0);

        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        cmdbuffer.end_event();

        cmdbuffer.close()?;

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);
        ci = (ci + 1) % 4;
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    dev.cleanup_dropped_resources(&swap_chain);

    Ok(())
}
