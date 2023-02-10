use hotline_rs::*;
use hotline_rs::client::*;
use hotline_rs::plugin::*;

pub struct EmptyPlugin;

impl EmptyPlugin {
    fn test(&self) {
        println!("calling: test");
    }
}

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
        self.test();
        client
    }

    fn reload(&mut self, client: Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin reload");
        client
    }
}

impl<D, A> imgui::UserInterface<D, A> for EmptyPlugin where D: gfx::Device, A: os::App {
    fn show_ui(&mut self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("hello world!", &mut imgui_open, imgui::WindowFlags::NONE) {
            }
            imgui.end();
            imgui_open
        }
        else {
            false
        }
    }
}

hotline_plugin![EmptyPlugin];