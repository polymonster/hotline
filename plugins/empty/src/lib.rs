// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;

pub struct EmptyPlugin;

impl Plugin<gfx_platform::Device, os_platform::App> for EmptyPlugin {
    fn create() -> Self {
        EmptyPlugin {
        }
    }

    fn setup(&mut self, _client: &mut Client<gfx_platform::Device, os_platform::App>) {
        println!("plugin setup");
    }

    fn update(&mut self, client: client::Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin update");
        client
    }

    fn unload(&mut self, _client: &mut Client<gfx_platform::Device, os_platform::App>) {
        println!("plugin unload");
    }

    fn ui(&mut self, _client: &mut Client<gfx_platform::Device, os_platform::App>) {
        println!("plugin ui");
    }
}

hotline_plugin![EmptyPlugin];