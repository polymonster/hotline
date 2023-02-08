use hotline_rs::*;
use hotline_rs::client::*;

pub struct EmptyPlugin {
}

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

    fn setup(&mut self, mut client: Client<gfx_platform::Device, os_platform::App>) 
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin setup");
        client
    }

    fn update(&mut self, mut client: client::Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin update");
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

#[no_mangle]
pub fn update(client: &mut client::Client<gfx_platform::Device, os_platform::App>) {
    println!("update plugin!");
}

#[no_mangle]
pub fn setup(client: &mut client::Client<gfx_platform::Device, os_platform::App>) {
    println!("{}", client.pmfx.active_update_graph);
    println!("hello plugin!");
}