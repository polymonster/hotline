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
        name: String::from("gfx_device"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // create a window
    println!("create window!");
    let mut window = app.create_window(os::WindowInfo {
        title: String::from("gfx_device!"),
        ..Default::default()
    });

    // create a device
    println!("create device!");
    let num_buffers = 2;
    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        ..Default::default()
    });

    while app.run() {
        println!("main loop!");
        // update window and swap chain
        window.update(&mut app);
    }

    Ok(())
}