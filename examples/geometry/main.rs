use hotline_rs::*;
use ecs::*;
use gfx::SwapChain;
use os::App;

use bevy_ecs::prelude::*;

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;

use core::time::Duration;

// The value of `dylib = "..."` should be the library containing the hot-reloadable functions
// It should normally be the crate name of your sub-crate.
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    use bevy_ecs::prelude::*;
    use super::ecs::*;

    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_updated]
    pub fn was_updated() -> bool {}

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
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
    ctx.pmfx.create_graph(& mut ctx.device, "forward")?;

    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_2d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_3d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_blit", ctx.swap_chain.get_backbuffer_pass())?;

    // create pass with depth
    {
        let view = ctx.pmfx.get_view("render_world_view").unwrap().clone();
        let view = view.lock().unwrap();
        ctx.pmfx.create_pipeline(&ctx.device, "imdraw_mesh", &view.pass)?;
    }

    //
    // build schedules
    //

    let mut world = World::new();
    let mut schedule = Schedule::default();

    let mut run_startup = true;
    let mut imgui_open = true;

    let mut call_schedule = true;
    let reloader = hot_lib::subscribe();

    let mut rest_timer = 0;

    let mut schedules : Vec<Schedule> = Vec::new();
    let mut startup_schedules : Vec<Schedule> = Vec::new();
    schedules.push(schedule);

    while ctx.app.run() {
        ctx.new_frame();

        // imgui
        if ctx.imgui.begin("hello world", &mut imgui_open, imgui::WindowFlags::NONE) {
            if ctx.imgui.button("run startup") {
                run_startup = true;
            }
            ctx.imgui.image(ctx.pmfx.get_texture("main_colour").unwrap(), 640.0, 360.0);
            ctx.imgui.image(ctx.pmfx.get_texture("main_depth").unwrap(), 640.0, 360.0);
        }

        if call_schedule {
            // move hotline resource into world
            world.insert_resource(DeviceRes {0: ctx.device});
            world.insert_resource(AppRes {0: ctx.app});
            world.insert_resource(MainWindowRes {0: ctx.main_window});
            world.insert_resource(PmfxRes {0: ctx.pmfx});
            world.insert_resource(ImDrawRes {0: ctx.imdraw});
            world.insert_resource(ImGuiRes {0: ctx.imgui});
            
            // run startup
            if run_startup {
                println!("hotline_rs::hot_reload:: startup");
                world.clear_entities();

                // build schedules
                let mut startup_schedule = Schedule::default();
                let mut update_schedule = Schedule::default();
                hot_lib::build_schedule(&mut startup_schedule, &mut update_schedule);
                
                // track schedules
                startup_schedules.push(startup_schedule);
                schedules.push(update_schedule);

                startup_schedules.last_mut().unwrap().run(&mut world);
                run_startup = false;
            }

            // run systems
            schedules.last_mut().unwrap().run(&mut world);
        
            // move resources back out
            ctx.device = world.remove_resource::<DeviceRes>().unwrap().0;
            ctx.app = world.remove_resource::<AppRes>().unwrap().0;
            ctx.main_window = world.remove_resource::<MainWindowRes>().unwrap().0;
            ctx.pmfx = world.remove_resource::<PmfxRes>().unwrap().0;
            ctx.imdraw = world.remove_resource::<ImDrawRes>().unwrap().0;
            ctx.imgui = world.remove_resource::<ImGuiRes>().unwrap().0;
        }

        // present to back buffer
        ctx.present("main_colour");

        if reloader.wait_for_about_to_reload_timeout(Duration::from_micros(100)).is_some() {
            println!("hotline_rs::hot_reload:: reloading");
            reloader.wait_for_reload();
            run_startup = true;
        }
    }

    ctx.wait_for_last_frame();

    // exited with code 0
    Ok(())
}