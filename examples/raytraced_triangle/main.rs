use gfx::RaytracingPipelineInfo;
use hotline_rs::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

use std::fs;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn main() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("raytraced_triangle"),
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
    println!("features: {:?}", device.get_feature_flags());

    let mut window = app.create_window(os::WindowInfo {
        title: String::from("raytraced_triangle!"),
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

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut device, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/raytracing_example"))?;
    pmfx.create_raytracing_pipeline(&device, "raytracing");

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
        swap_chain.swap(&device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    device.cleanup_dropped_resources(&swap_chain);

    Ok(())
}