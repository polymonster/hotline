// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;


fn main() -> Result<(), hotline_rs::Error> {    
    
    // create client
    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        ..Default::default()
    })?;

    // add plugins
    let plugins = get_data_path("../plugins");
    ctx.add_plugin_lib("ecs", &plugins);
    
    // run
    ctx.run();

    // exited with code 0
    Ok(())
}
