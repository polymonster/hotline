// currently windows only because here we need a concrete gfx and os implementation
// #![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;

fn main() -> Result<(), hotline_rs::Error> {
    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        ..Default::default()
    })?;

    // run
    if let Err(e) = ctx.run() {
        println!("error: {}", e.msg);
    };

    Ok(())
}
