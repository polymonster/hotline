use hotline_rs::*;
use ecs::*;
use gfx::SwapChain;
use os::App;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use core::time::Duration;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

// The value of `dylib = "..."` should be the library containing the hot-reloadable functions
// It should normally be the crate name of your sub-crate.
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    use bevy_ecs::schedule::SystemDescriptor;
    use bevy_ecs::prelude::*;
    use super::ecs::*;

    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

fn get_system_function(name: &str) -> Option<SystemDescriptor> {
    let func = ecs::get_system_function(name);
    if func.is_some() {
        func
    }
    else {
        hot_lib::get_system_function_lib(name)
    }
}

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(PartialEq)]
enum ReloadState {
    None,
    Requested,
    Confirmed,
}

pub trait UserInterface<D: gfx::Device, A: os::App> {
    fn show_ui(&self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool;
}

impl<D, A> UserInterface<D, A> for pmfx::Pmfx<D> where D: gfx::Device, A: os::App {
    fn show_ui(&self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("pmfx", &mut imgui_open, imgui::WindowFlags::NONE) {
                imgui.image(self.get_texture("main_colour").unwrap(), 640.0, 360.0);
                imgui.image(self.get_texture("main_depth").unwrap(), 640.0, 360.0);
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

    //
    // create context
    //

    let mut ctx : Context<gfx_platform::Device, os_platform::App> = Context::create(HotlineInfo {
        ..Default::default()
    })?;

    //
    // create pmfx
    //

    ctx.pmfx.load(&hotline_rs::get_asset_path("data/shaders/imdraw").as_str())?;
    ctx.pmfx.create_render_graph(& mut ctx.device, "forward")?;
    ctx.pmfx.create_pipeline(&mut ctx.device, "imdraw_blit", &ctx.swap_chain.get_backbuffer_pass())?;

    //
    // build schedules
    //

    let mut run_startup = true;
    let mut imgui_open = false;
    
    let mut world = World::new();
    let mut schedule = Schedule::default();
    let mut startup_schedule = Schedule::default();

    let reloader = hot_lib::subscribe();
    let a_lock = Arc::new(Mutex::new(ReloadState::None));

    let a_lock_thread = a_lock.clone();
    thread::spawn(move || {
        loop {
            // wait for a reload
            let tok = reloader.wait_for_about_to_reload();
            
            // request enter reload state
            let mut a = a_lock_thread.lock().unwrap();
            println!("hotline_rs::reload:: requested");
            *a = ReloadState::Requested;
            drop(a);
            
            // wait till main thread signals we are safe
            loop {
                let mut a = a_lock_thread.lock().unwrap();
                if *a == ReloadState::Confirmed {
                    *a = ReloadState::None;
                    drop(a);
                    break;
                }
                std::thread::sleep(Duration::from_millis(16));
            }

            // unloack
            drop(tok);
        }
    });

    while ctx.app.run() {
        // sync
        let mut reload_lock = a_lock.lock().unwrap();
        if *reload_lock != ReloadState::None {
            // drop everything while its safe.. might want to wait for the GPU
            startup_schedule = Schedule::default();
            schedule = Schedule::default();
            world = World::new();

            // signal it is safe to proceed and reload the new code
            *reload_lock = ReloadState::Confirmed;
            drop(reload_lock);
            println!("hotline_rs::reload:: confirmed");
            
            // wait for reload to complete
            hot_lib::subscribe().wait_for_reload();
            run_startup = true;
            reload_lock = a_lock.lock().unwrap();
        }

        ctx.new_frame();

        imgui_open = ctx.pmfx.show_ui(&ctx.imgui, imgui_open);

        // schedule builder
        if run_startup {

            // render functions
            let render_funcs = ctx.pmfx.get_render_function_names("forward");
            let mut render_stage = SystemStage::parallel();
            for func_name in &render_funcs {
                if let Some(func) = get_system_function(func_name) {
                    render_stage = render_stage.with_system(func);
                }
            }
            schedule.add_stage(StageRender, render_stage);

            // add startup funcs by name
            let setup_funcs = ctx.pmfx.get_setup_function_names("core");
            let mut startup_stage = SystemStage::parallel();
            for func_name in &setup_funcs {
                if let Some(func) = get_system_function(func_name) {
                    startup_stage = startup_stage.with_system(func);
                }
            }
            startup_schedule.add_stage(StageStartup, startup_stage);

            // add update funcs by name
            let update_funcs = ctx.pmfx.get_update_function_names("core");
            let mut update_stage = SystemStage::parallel();
            for func_name in &update_funcs {
                if let Some(func) = get_system_function(func_name) {
                    update_stage = update_stage.with_system(func);
                }
            }
            schedule.add_stage(StageUpdate, update_stage);
        }

        // move hotline resource into world
        world.insert_resource(DeviceRes {0: ctx.device});
        world.insert_resource(AppRes {0: ctx.app});
        world.insert_resource(MainWindowRes {0: ctx.main_window});
        world.insert_resource(PmfxRes {0: ctx.pmfx});
        world.insert_resource(ImDrawRes {0: ctx.imdraw});
        world.insert_resource(ImGuiRes {0: ctx.imgui});

        // run startup
        if run_startup {
            println!("hotline_rs::reload:: startup");

            world.spawn((
                Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
                Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
                ViewProjectionMatrix { 0: Mat4f::identity()},
                Camera,
            ));

            startup_schedule.run(&mut world);
            run_startup = false;
        }

        // run systems
        schedule.run(&mut world);

        // move resources back out
        ctx.device = world.remove_resource::<DeviceRes>().unwrap().0;
        ctx.app = world.remove_resource::<AppRes>().unwrap().0;
        ctx.main_window = world.remove_resource::<MainWindowRes>().unwrap().0;
        ctx.pmfx = world.remove_resource::<PmfxRes>().unwrap().0;
        ctx.imdraw = world.remove_resource::<ImDrawRes>().unwrap().0;
        ctx.imgui = world.remove_resource::<ImGuiRes>().unwrap().0;

        // present to back buffer
        ctx.present("main_colour");
        drop(reload_lock);
    }

    ctx.wait_for_last_frame();

    // exited with code 0
    Ok(())
}