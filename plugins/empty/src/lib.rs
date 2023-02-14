use hotline_rs::*;
use hotline_rs::client::*;
use hotline_rs::plugin::*;

pub struct EmptyPlugin;

impl Plugin<gfx_platform::Device, os_platform::App> for EmptyPlugin {
    fn create() -> Self {
        EmptyPlugin {
        }
    }

    fn setup(&mut self, client: Client<gfx_platform::Device, os_platform::App>) 
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin setup");
        client
    }

    fn update(&mut self, client: client::Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin update");
        client
    }

    fn unload(&mut self, client: Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin unload");
        client
    }

    fn ui(&mut self, client: Client<gfx_platform::Device, os_platform::App>)
    -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin ui");
        client
    }
}

hotline_plugin![EmptyPlugin];