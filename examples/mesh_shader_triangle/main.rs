use hotline_rs::gfx::MeshPipelineInfo;
use hotline_rs::*;
use hotline_rs::prelude::{gfx_platform, os_platform};

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;
use gfx::Size3;

use os::App;
use os::Window;

use std::fs;

fn main() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("mesh_shader_triangle"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    let num_buffers : u32 = 2;

    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        ..Default::default()
    });
    println!("{}", device.get_adapter_info());

    let mut window = app.create_window(os::WindowInfo {
        title: String::from("mesh_shader_triangle!"),
        ..Default::default()
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(num_buffers);

    // create mesh / fragment shaders
    let msc_filepath = hotline_rs::get_data_path("shaders/mesh_shader_example/ms_main.msc");
    let psc_filepath = hotline_rs::get_data_path("shaders/mesh_shader_example/ps_main.psc");

    let msc_data = fs::read(msc_filepath)?;
    let psc_data = fs::read(psc_filepath)?;

    let msc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Mesh,
        compile_info: None
    };
    let ms = device.create_shader(&msc_info, &msc_data)?;

    let psc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: None
    };
    let fs = device.create_shader(&psc_info, &psc_data)?;

    // create mesh pipeline
    let mesh_pipeline = device.create_mesh_pipeline(&gfx::MeshPipelineInfo {
        ms: Some(&ms),
        fs: Some(&fs),
        topology: gfx::PrimitiveTopology::TriangleList,
        // an empty blend_info leaves the render target write mask at 0 (nothing is written),
        // so provide a default blend target the same way the triangle example does
        blend_info: gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        },
        pass: Some(swap_chain.get_backbuffer_pass()),
        ..Default::default()
    })?;

    while app.run() {
        // update window and swap chain
        window.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // update viewport from window size
        let window_rect = window.get_viewport_rect();
        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        // build command buffer and make draw calls
        cmd.reset(&swap_chain);

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmd.set_viewport(&viewport);
        cmd.set_scissor_rect(&scissor);

        cmd.set_mesh_pipeline(&mesh_pipeline);
        cmd.dispatch_mesh(gfx::Size3 {
            x: 3,
            y: 1,
            z: 1
        }, gfx::Size3 {
            x: 1,
            y: 1,
            z: 1
        });

        cmd.end_render_pass();

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });

        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&mut device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    device.cleanup_dropped_resources(&swap_chain);

    Ok(())
}
