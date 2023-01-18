use hotline_rs::*;
use ecs::*;
use gfx::SwapChain;
use os::App;

use bevy_ecs::prelude::*;

use core::time::Duration;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

// The value of `dylib = "..."` should be the library containing the hot-reloadable functions
// It should normally be the crate name of your sub-crate.
#[hot_lib_reloader::hot_module(dylib = "lib")]
mod hot_lib {
    use bevy_ecs::prelude::*;
    use super::ecs::*;

    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
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

    let mut run_startup = true;
    let mut imgui_open = true;
    
    let mut world = World::new();
    let mut schedule = Schedule::default();

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
        let mut lock = a_lock.lock().unwrap();
        if *lock != ReloadState::None {
            // drop everything while its safe.. might want to wait for the GPU
            schedule = Schedule::default();
            world = World::new();

            // signal it is safe to proceed and reload the new code
            *lock = ReloadState::Confirmed;
            drop(lock);
            println!("hotline_rs::reload:: confirmed");
            
            // wait for reload to complete
            hot_lib::subscribe().wait_for_reload();
            run_startup = true;
            lock = a_lock.lock().unwrap();
        }

        ctx.new_frame();

        // imgui
        if imgui_open {
            if ctx.imgui.begin("hello world", &mut imgui_open, imgui::WindowFlags::NONE) {
                if ctx.imgui.button("run startup") {
                    run_startup = true;
                }
                ctx.imgui.image(ctx.pmfx.get_texture("main_colour").unwrap(), 640.0, 360.0);
                ctx.imgui.image(ctx.pmfx.get_texture("main_depth").unwrap(), 640.0, 360.0);
            }
            ctx.imgui.end();
        }

        // move hotline resource into 
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

            // build schedules
            let mut startup_schedule = Schedule::default();
            hot_lib::build_schedule(&mut startup_schedule, &mut schedule);

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
        drop(lock);
    }

    ctx.wait_for_last_frame();

    // exited with code 0
    Ok(())
}