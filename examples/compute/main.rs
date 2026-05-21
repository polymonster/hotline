use hotline_rs::{*, prelude::*};

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain, RenderPass, Texture};

#[repr(C)]
struct Vertex {
    position: [f32; 2],
    texcoord: [f32; 2],
}

// matches `julia_constants` in shaders/julia.hlsl (5x 32-bit values)
#[repr(C)]
struct JuliaConstants {
    output_index: u32,
    width: u32,
    height: u32,
    cr: f32,
    ci: f32,
}

fn main() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("compute"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });
    print!("{}", dev.get_adapter_info());

    let mut win = app.create_window(os::WindowInfo {
        title: String::from("compute - animated julia set"),
        rect: os::Rect { x: 100, y: 100, width: 1280, height: 720 },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmdbuffer = dev.create_cmd_buf(2);

    // fullscreen quad (NDC) with texcoords flipped so (0,0) is top-left of the image
    let vertices = [
        Vertex { position: [-1.0, -1.0], texcoord: [0.0, 1.0] },
        Vertex { position: [-1.0,  1.0], texcoord: [0.0, 0.0] },
        Vertex { position: [ 1.0,  1.0], texcoord: [1.0, 0.0] },
        Vertex { position: [ 1.0, -1.0], texcoord: [1.0, 1.0] },
    ];
    let vertex_buffer = dev.create_buffer(&gfx::BufferInfo {
        usage: gfx::BufferUsage::VERTEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 4,
        initial_state: gfx::ResourceState::VertexConstantBuffer
    }, Some(gfx::as_u8_slice(&vertices)))?;

    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let index_buffer = dev.create_buffer(&gfx::BufferInfo {
        usage: gfx::BufferUsage::INDEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<u16>(),
        num_elements: 6,
        initial_state: gfx::ResourceState::IndexBuffer
    }, Some(gfx::as_u8_slice(&indices)))?;

    // compute output texture - written by the julia kernel, sampled by the blit pass
    let vp_rect = win.get_viewport_rect();
    let tex_width = vp_rect.width as u64;
    let tex_height = vp_rect.height as u64;
    let output_texture = dev.create_texture::<u8>(&gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: tex_width,
        height: tex_height,
        depth: 1,
        array_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::UNORDERED_ACCESS,
        initial_state: gfx::ResourceState::UnorderedAccess,
    }, None)?;
    let uav_index = output_texture.get_uav_index().unwrap() as u32;
    let srv_index = output_texture.get_srv_index().unwrap() as u32;

    // load shaders and create the compute + blit pipelines via pmfx
    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut dev, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/julia"))?;
    pmfx.create_compute_pipeline(&dev, "julia")?;
    pmfx.create_render_pipeline(&dev, "blit", swap_chain.get_backbuffer_pass())?;

    let blit_fmt = swap_chain.get_backbuffer_pass().get_format_hash();

    let mut frame = 0u32;
    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // animate the complex constant around a circle for a classic morphing julia set
        let theta = frame as f32 * 0.0125;
        let radius = 0.7885;
        let constants = JuliaConstants {
            output_index: uav_index,
            width: tex_width as u32,
            height: tex_height as u32,
            cr: radius * theta.cos(),
            ci: radius * theta.sin(),
        };

        // compute pass - dispatch the julia kernel into the rw texture
        cmdbuffer.begin_event(0xff00ff00, "Julia Compute");
        let julia = pmfx.get_compute_pipeline("julia")?;
        cmdbuffer.set_compute_pipeline(julia);
        cmdbuffer.set_heap(julia, dev.get_shader_heap());
        cmdbuffer.push_compute_constants(julia, 0, 0, 5, 0, gfx::as_u8_slice(&constants));
        cmdbuffer.dispatch(
            gfx::Size3 { x: (tex_width as u32 + 7) / 8, y: (tex_height as u32 + 7) / 8, z: 1 },
            gfx::Size3 { x: 8, y: 8, z: 1 }
        );
        cmdbuffer.end_event();

        // blit pass - draw the compute output to the back buffer
        cmdbuffer.begin_event(0xff0000ff, "Blit Pass");
        let vp = gfx::Viewport::from(win.get_viewport_rect());
        let sc = gfx::ScissorRect::from(win.get_viewport_rect());

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let blit = pmfx.get_render_pipeline_for_format("blit", blit_fmt)?;
        cmdbuffer.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmdbuffer.set_viewport(&vp);
        cmdbuffer.set_scissor_rect(&sc);
        cmdbuffer.set_render_pipeline(blit);
        cmdbuffer.set_heap(blit, dev.get_shader_heap());
        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
        let srv = [srv_index, 0, 0, 0];
        cmdbuffer.push_render_constants(blit, 0, 0, 4, 0, gfx::as_u8_slice(&srv));
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
        swap_chain.swap(&mut dev);
        frame += 1;
    }

    swap_chain.wait_for_last_frame();
    dev.cleanup_dropped_resources(&swap_chain);
    Ok(())
}
