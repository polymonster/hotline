// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;
use ecs::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use libloading::Symbol;

struct BevyRunner {
    world: World,
    setup_schedule: Schedule,
    schedule: Schedule,
    run_setup: bool,
    libs: Vec<hot_lib_reloader::LibReloader>,
    demo_list: Vec<String>,
    demo: String
}

impl BevyRunner {
    fn get_system_function(&self, name: &str) -> Option<SystemDescriptor> {
        let func = ecs::get_system_function(name);
        if func.is_some() {
            func
        }
        else {
            for lib in &self.libs {
                unsafe {
                    let get_function : Symbol<unsafe extern fn(String) -> Option<SystemDescriptor>> 
                        = lib.get_symbol("get_system_function_lib".as_bytes()).unwrap();
                    let f = get_function(name.to_string());
                    if f.is_some() {
                        return f;
                    }
                }
            }
            None
        }
    }
}

impl Plugin<gfx_platform::Device, os_platform::App> for BevyRunner {
    fn create() -> Self {
        BevyRunner {
            world: World::new(),
            setup_schedule: Schedule::default(),
            schedule: Schedule::default(),
            run_setup: false,
            libs: Vec::new(),
            demo_list: Vec::new(),
            demo: String::new()
        }
    }

    fn setup(&mut self, mut client: Client<gfx_platform::Device, os_platform::App>) 
        -> Client<gfx_platform::Device, os_platform::App> {

        client.pmfx.create_render_graph(& mut client.device, "forward").unwrap();

        let setup_systems = vec![
            "setup_single".to_string(),
        ];
        let update_systems = vec![
            "mat_movement".to_string(),
            "update_cameras".to_string()
        ];
        let render_systems = client.pmfx.get_render_function_names("forward");

        // render functions
        let mut render_stage = SystemStage::parallel();
        for func_name in &render_systems {
            if let Some(func) = self.get_system_function(func_name) {
                render_stage = render_stage.with_system(func);
            }
        }
        self.schedule.add_stage(StageRender, render_stage);

        // add startup funcs by name
        let mut setup_stage = SystemStage::parallel();
        for func_name in &setup_systems {
            if let Some(func) = self.get_system_function(func_name) {
                setup_stage = setup_stage.with_system(func);
            }
        }
        self.setup_schedule.add_stage(StageStartup, setup_stage);

        // add update funcs by name
        let mut update_stage = SystemStage::parallel();
        for func_name in &update_systems {
            if let Some(func) = self.get_system_function(func_name) {
                update_stage = update_stage.with_system(func);
            }
        }
        self.schedule.add_stage(StageUpdate, update_stage);
        self.run_setup = true;
        client
    }

    fn update(&mut self, mut client: client::Client<gfx_platform::Device, os_platform::App>) ->
        client::Client<gfx_platform::Device, os_platform::App> {
        // move hotline resource into world
        self.world.insert_resource(DeviceRes {0: client.device});
        self.world.insert_resource(AppRes {0: client.app});
        self.world.insert_resource(MainWindowRes {0: client.main_window});
        self.world.insert_resource(PmfxRes {0: client.pmfx});
        self.world.insert_resource(ImDrawRes {0: client.imdraw});
        self.world.insert_resource(ImGuiRes {0: client.imgui});

        // run setup if requested, we dio it here so hotline resources are inserted into World
        if self.run_setup {
            self.world.spawn((
                Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
                Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
                ViewProjectionMatrix { 0: Mat4f::identity()},
                Camera,
            ));
            self.setup_schedule.run(&mut self.world);
            self.run_setup = false;
        }

        // update systems
        self.schedule.run(&mut self.world);

        // move resources back out
        client.device = self.world.remove_resource::<DeviceRes>().unwrap().0;
        client.app = self.world.remove_resource::<AppRes>().unwrap().0;
        client.main_window = self.world.remove_resource::<MainWindowRes>().unwrap().0;
        client.pmfx = self.world.remove_resource::<PmfxRes>().unwrap().0;
        client.imdraw = self.world.remove_resource::<ImDrawRes>().unwrap().0;
        client.imgui = self.world.remove_resource::<ImGuiRes>().unwrap().0;
        client
    }

    fn reload(&mut self, client: Client<gfx_platform::Device, os_platform::App>) 
        -> Client<gfx_platform::Device, os_platform::App> {
        // drop everything while its safe
        self.setup_schedule = Schedule::default();
        self.schedule = Schedule::default();
        self.world = World::new();
        client
    }
}

impl<D, A> imgui::UserInterface<D, A> for BevyRunner where D: gfx::Device, A: os::App {
    fn show_ui(&mut self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("runner", &mut imgui_open, imgui::WindowFlags::NONE) {
                for demo in &self.demo_list {
                    if imgui.button(&demo) {
                        self.demo = demo.to_string();
                    }
                }
            }
            imgui.end();
            imgui_open
        }
        else {
            false
        }
    }
}


fn main() -> Result<(), hotline_rs::Error> {    
    
    // create client
    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        ..Default::default()
    })?;

    // add plugins
    ctx.add_plugin(Box::new(BevyRunner::create()));
    
    // run
    ctx.run2();

    // exited with code 0
    Ok(())
}
