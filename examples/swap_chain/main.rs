use hotline_rs::gfx::CmdBuf;
use hotline_rs::gfx::RenderPassInfo;
use hotline_rs::gfx::SwapChain;
use hotline_rs::*;

use os::App;
use os::Window;

use maths_rs::Vec3f;
use maths_rs::vec::vec3f;
use maths_rs::vec::splat3f;
use maths_rs::dot;
use maths_rs::cos;

use gfx::Device;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;

#[cfg(target_os = "windows")]
use gfx::d3d12 as gfx_platform;

#[cfg(target_os = "macos")]
use os::macos as os_platform;

#[cfg(target_os = "macos")]
use gfx::mtl as gfx_platform;

fn main() -> Result<(), hotline_rs::Error> {
    // create an app
    println!("create app!");
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("swap_chain"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // create a window
    println!("create window!");
    let mut window = app.create_window(os::WindowInfo {
        title: String::from("swap_chain!"),
        ..Default::default()
    });

    // create a device
    println!("create device!");
    let num_buffers = 2;
    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        ..Default::default()
    });

    // create a swap chain
    println!("create swap chain!");
    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers,
        format: gfx::Format::RGBA8n,
        clear_colour: None
    };

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(num_buffers);

    let mut t = 0.0;
    while app.run() {
        // update the swap chain
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // update window and swap chain
        window.update(&mut app);

        // gen col... thx to IQ!! https://iquilezles.org/articles/palettes/
        let a = vec3f(0.5, 0.5, 0.5);
        let b = vec3f(0.5, 0.5, 0.5);
        let c = vec3f(1.0, 1.0, 0.5);
        let d = vec3f(0.80, 0.90, 0.30);
        let col = a + b * cos(6.283185 * (c * t + d));

        let render_pass = device.create_render_pass(&RenderPassInfo{
            rt_clear: Some(gfx::ClearColour {
                r: col.x,
                g: col.y,
                b: col.z,
                a: 1.0
            }),
            render_targets: vec![swap_chain.get_backbuffer_texture()],
            depth_stencil: None,
            ds_clear: None,
            discard: false,
            resolve: false,
            array_slice: 0
        })?;

        // render pass to clear
        cmd.reset(&swap_chain);
        cmd.begin_render_pass(&render_pass);
        cmd.end_render_pass();
        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        println!("swap {}", counter);
        swap_chain.swap(&device);

        // swap colours and inc counter
        counter += 1;
        if counter % 30 == 0 {
            col_index = (col_index + 1) % colours.len();
        }

        t += 1.0 / 600.0;
    }

    Ok(())
}