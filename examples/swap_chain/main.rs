use hotline_rs::gfx::CmdBuf;
use hotline_rs::gfx::RenderPassInfo;
use hotline_rs::gfx::SwapChain;
use hotline_rs::*;

use os::App;
use os::Window;

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

    let colours = [
        gfx::ClearColour {
            r: 1.00,
            g: 0.00,
            b: 0.00,
            a: 1.00
        },
        gfx::ClearColour {
            r: 0.00,
            g: 1.00,
            b: 0.00,
            a: 1.00
        },
        gfx::ClearColour {
            r: 0.00,
            g: 0.00,
            b: 1.00,
            a: 1.00
        },
        gfx::ClearColour {
            r: 0.00,
            g: 1.00,
            b: 1.00,
            a: 1.00
        },
        gfx::ClearColour {
            r: 1.00,
            g: 0.00,
            b: 1.00,
            a: 1.00
        },
        gfx::ClearColour {
            r: 1.00,
            g: 1.00,
            b: 0.00,
            a: 1.00
        }
    ];
    let mut col_index = 0;
    let mut counter = 0;

    while app.run() {
        // update the swap chain
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // update window and swap chain
        window.update(&mut app);

        let render_pass = device.create_render_pass(&RenderPassInfo{
            rt_clear: Some(colours[col_index]),
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

        println!("swap {}", counter);
        swap_chain.swap(&device);

        // swap colours and inc counter
        counter += 1;
        if counter % 30 == 0 {
            col_index = (col_index + 1) % colours.len();
        }
    }

    Ok(())
}