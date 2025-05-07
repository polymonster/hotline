use hotline_rs::*;

use os::App;
use os::Window;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;

#[cfg(target_os = "macos")]
use os::macos as os_platform;

fn main() -> Result<(), hotline_rs::Error> {
    println!("create app!");
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("window"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    println!("create window!");
    let mut window = app.create_window(os::WindowInfo {
        title: String::from("window!"),
        ..Default::default()
    });

    while app.run() {
        println!("main loop!");
        // update window and swap chain
        window.update(&mut app);
    }

    Ok(())
}