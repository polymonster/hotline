use hotline_rs::*;
use hotline_rs::prelude::*;

#[cfg(target_os = "macos")]
fn platform_dpi_aware() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn platform_dpi_aware() -> bool {
    true
}

fn main() -> Result<(), hotline_rs::Error> {

    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        eprintln!("PANIC: {info}\n{bt}");
        std::process::abort();
    }));

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        dpi_aware: platform_dpi_aware(),
        ..Default::default()
    })?;

    // run
    if let Err(e) = ctx.run() {
        println!("error: {}", e.msg);
    };

    Ok(())
}
