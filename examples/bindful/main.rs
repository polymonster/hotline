use hotline_rs::{*, prelude::{Pipeline, Texture}};

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain, RenderPass};

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

use std::fs;

fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("bindful"),
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
        title: String::from("bindful"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
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
    pmfx.load(&hotline_rs::get_data_path("shaders/bindful"))?;
    pmfx.create_render_pipeline(&dev, "bindful", swap_chain.get_backbuffer_pass())?;

    let fmt = swap_chain.get_backbuffer_pass().get_format_hash();
    let pso_pmfx = pmfx.get_render_pipeline_for_format("bindful", fmt)?;

    let mut textures: Vec<gfx_platform::Texture> = Vec::new();
    let files = vec![
        hotline_rs::get_src_data_path("textures/bear/bear_stomp_anim_001.png"),
        hotline_rs::get_src_data_path("textures/bear/bear_stomp_anim_004.png"),
        hotline_rs::get_src_data_path("textures/bear/bear_stomp_anim_008.png"),
        hotline_rs::get_src_data_path("textures/bear/bear_stomp_anim_012.png"),
    ];
    for file in files {
        let image = image::load_from_file(&file)?;
        let tex = dev.create_texture(&image.info, data![image.data.as_slice()])?;
        textures.push(tex);
    }
    // ..
    let mut ci = 0;
    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

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

        //cmdbuffer.set_heap(pso_pmfx, dev.get_shader_heap());
        cmdbuffer.set_heap_render(pso_pmfx, dev.get_shader_heap());

        // set bindings
        let srv3 = textures[0].get_srv_index().unwrap();
        let srv2 = textures[1].get_srv_index().unwrap();
        let srv1 = textures[2].get_srv_index().unwrap();
        let srv0 = textures[3].get_srv_index().unwrap();

        // this looks up register t0, space0
        if let Some(t0) = pso_pmfx.get_pipeline_slot(0, 0, gfx::DescriptorType::ShaderResource) {
            cmdbuffer.set_binding(pso_pmfx, dev.get_shader_heap(), t0.index, srv0);
        }

        // this looks up register t1, space0
        if let Some(t1) = pso_pmfx.get_pipeline_slot(1, 0, gfx::DescriptorType::ShaderResource) {
            cmdbuffer.set_binding(pso_pmfx, dev.get_shader_heap(), t1.index, srv1);
        }

        // this looks up register t2, space0
        if let Some(t2) = pso_pmfx.get_pipeline_slot(2, 0, gfx::DescriptorType::ShaderResource) {
            cmdbuffer.set_binding(pso_pmfx, dev.get_shader_heap(), t2.index, srv2);
        }

        // this looks up register t3, space0
        if let Some(t3) = pso_pmfx.get_pipeline_slot(3, 0, gfx::DescriptorType::ShaderResource) {
            cmdbuffer.set_binding(pso_pmfx, dev.get_shader_heap(), t3.index, srv3);
        }

        // ..
        cmdbuffer.set_texture(&textures[0], 0);
        cmdbuffer.set_texture(&textures[1], 1);
        cmdbuffer.set_texture(&textures[2], 2);
        cmdbuffer.set_texture(&textures[3], 3);

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
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
