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

fn new_plugin() -> *mut EmptyPlugin {
    unsafe {
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<EmptyPlugin>(),
            8,
        )
        .unwrap();
        std::alloc::alloc_zeroed(layout) as *mut EmptyPlugin
    }
}


#[no_mangle]
pub fn update(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
    println!("update plugin!");
    unsafe { 
        let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut EmptyPlugin>(ptr);
        let plugin = plugin.as_mut().unwrap();
        plugin.update(client)
    }
}

#[no_mangle]
pub fn setup(client: &mut client::Client<gfx_platform::Device, os_platform::App>) -> *mut core::ffi::c_void {
    println!("hello plugin!");
    new_plugin() as *mut core::ffi::c_void
}