use hotline_rs::{*, prelude::*};

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain};

use std::fs;

#[repr(C)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("imgui"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // device
    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("imgui!"),
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
    let mut cmdbuffer = dev.create_cmd_buf(2);

    let roboto = get_data_path("fonts/roboto_medium.ttf");
    let mut imgui_info = imgui::ImGuiInfo {
        device: &mut dev,
        swap_chain: &mut swap_chain,
        main_window: &win,
        fonts: vec![imgui::FontInfo {
            filepath: roboto,
            glyph_ranges: None
        }],
        monitors: app.enumerate_display_monitors()
    };
    let mut imgui = imgui::ImGui::create(&mut imgui_info).unwrap();

    // dummy vb
    let vertices = [
        Vertex {
            position: [-1.0, -1.0],
            uv: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [-1.0, 1.0],
            uv: [0.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [1.0, 1.0],
            uv: [0.0, 0.0],
            color: [1.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [1.0, -1.0],
            uv: [0.0, 0.0],
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

    // create imgui shader
    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut dev, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/imgui"))?;
    pmfx.create_render_pipeline(&dev, "default", swap_chain.get_backbuffer_pass())?;
    let fmt = swap_chain.get_backbuffer_pass().get_format_hash();
    let pso_pmfx = pmfx.get_render_pipeline_for_format("default", fmt)?;

    // create pipeline manually
    let vsc_filepath = crate::get_data_path("shaders/imgui/vs_main.vsc");
    let psc_filepath = crate::get_data_path("shaders/imgui/ps_main.psc");

    let vsc_data = std::fs::read(vsc_filepath)?;
    let psc_data = std::fs::read(psc_filepath)?;

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: None
    };

    let ps_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: None
    };

    let vs = dev.create_shader(&vs_info, &vsc_data)?;
    let fs = dev.create_shader(&ps_info, &psc_data)?;

    // ..
    let mut ci = 0;
    while app.run() {

        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // main pass
        cmdbuffer.begin_event(0xff0000ff, "Main Pass");

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);

        // imgui
        imgui.new_frame(&mut app, &mut win, &mut dev);
        imgui.demo();
        imgui.render(&mut app, &mut win, &mut dev, &mut cmdbuffer, &Vec::new());

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
        ci = (ci + 1) % 4;
    }

    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    dev.cleanup_dropped_resources(&swap_chain);

    Ok(())
}
