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
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(num_buffers);

    let mut counter = 0;
    while app.run() {
        // update the swap chain
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // update window and swap chain
        window.update(&mut app);

        println!("swap {}", counter);
        swap_chain.swap(&device);

        // sleep?
        println!("sleep");
        std::thread::sleep(std::time::Duration::from_millis(16));
        counter += 1;
    }

    Ok(())
}