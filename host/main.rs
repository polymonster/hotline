use hot_lib_reloader::LibReloadObserver;
use hotline_rs::*;
use ecs::*;
use gfx::SwapChain;
use os::App;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use core::time::Duration;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use std::process::Command;
use std::collections::HashMap;

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
    fn show_ui(&mut self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool;
}

impl<D, A> UserInterface<D, A> for pmfx::Pmfx<D> where D: gfx::Device, A: os::App {
    fn show_ui(&mut self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;


            if imgui.begin("pmfx", &mut imgui_open, imgui::WindowFlags::NONE) {
                //imgui.image(self.get_texture("main_colour").unwrap(), 640.0, 360.0);
                //imgui.image(self.get_texture("main_depth").unwrap(), 640.0, 360.0);

                let options = self.get_update_graph_names();
                for option in options {
                    if imgui.button(&option) {
                        self.active_update_graph = option;
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

#[derive(PartialEq, Eq, Hash)]
enum ReloadCategory {
    Code,
    Pmfx
}

#[derive(PartialEq)]
enum ReloadResult {
    Continue,
    Reload
}

struct ReloaderInfo {
    files: Vec<(ReloadCategory, Duration, String)>,
}

struct Reloader {
    /// Hash map storing files grouped by type (pmfx, code) and then keep a vector of files
    /// and timestamps for quick checking at run time.
    files: HashMap<ReloadCategory, Vec<(Duration, String)>>,
    lock: Arc<Mutex<ReloadState>>
}

impl Reloader {
    fn create(info: ReloaderInfo) -> Reloader {
        let mut files = HashMap::new();
        files.insert(ReloadCategory::Code, Vec::new());
        files.insert(ReloadCategory::Pmfx, Vec::new());
        for file in info.files {
            //files[file.0].push((file.1, file.2.to_string()));
        }
        Reloader {
            files,
            lock: Arc::new(Mutex::new(ReloadState::None))
        }
    }

    fn lib_reload_sync_thread(&self) {
        let lock = self.lock.clone();
        let observer = hot_lib::subscribe();
        thread::spawn(move || {
            loop {
                // wait for a reload
                let tok = observer.wait_for_about_to_reload();
                
                // request enter reload state
                let mut a = lock.lock().unwrap();
                println!("hotline_rs::reload:: requested");
                *a = ReloadState::Requested;
                drop(a);
                
                // wait till main thread signals we are safe
                loop {
                    let mut a = lock.lock().unwrap();
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
    }

    fn file_watcher_thread() {
        let lib_path = hotline_rs::get_data_path("../lib/src/lib.rs");
        let mut lib_modified_time = std::fs::metadata(&lib_path).unwrap().modified().unwrap();
    
        let pmfx_path = hotline_rs::get_data_path("../src/shaders/imdraw.pmfx");
        let pmfx_modified_tome = std::fs::metadata(&pmfx_path).unwrap().modified().unwrap();
    
        loop {
            // check code changes
            let cur_lib_modified_time = std::fs::metadata(&lib_path).unwrap().modified().unwrap();
            if cur_lib_modified_time > lib_modified_time {
                println!("hotline_rs::hot_lib:: code changes detected");
                // kick off a build
                let output = Command::new("cargo")
                    .arg("build")
                    .arg("-p")
                    .arg("lib")
                    .output()
                    .expect("hotline::hot_lib:: hot lib failed to build!");
    
                if output.stdout.len() > 0 {
                    println!("{}", String::from_utf8(output.stdout).unwrap());
                }
    
                if output.stderr.len() > 0 {
                    println!("{}", String::from_utf8(output.stderr).unwrap());
                }
    
                lib_modified_time = cur_lib_modified_time;
            }
    
            // check shader changes
            let cur_pmfx_modified_time = std::fs::metadata(&pmfx_path).unwrap().modified().unwrap();
            if cur_pmfx_modified_time > pmfx_modified_tome {
                println!("hotline_rs::hot_lib:: pmfx changes detected");
                // kick off a buils
                let output = Command::new("cargo")
                    .arg("build")
                    .output()
                    .expect("hotline::hot_lib:: pmfx failed to build!");
    
                if output.stdout.len() > 0 {
                    println!("{}", String::from_utf8(output.stdout).unwrap());
                }
    
                if output.stderr.len() > 0 {
                    println!("{}", String::from_utf8(output.stderr).unwrap());
                }
    
                lib_modified_time = cur_lib_modified_time;
            }
    
            // yield
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    /// Start watching for and invoking reload changes, this will spawn threads to watch files
    pub fn start(&self) {
        thread::spawn(Self::file_watcher_thread);
        self.lib_reload_sync_thread();
    }

    pub fn check_for_reload(&self) -> ReloadResult {
        let lock = self.lock.lock().unwrap();
        if *lock != ReloadState::None {
            ReloadResult::Reload
        }
        else {
            ReloadResult::Continue
        }
    }

    pub fn complete_reload(&self) {
        let mut lock = self.lock.lock().unwrap();
        // signal it is safe to proceed and reload the new code
        *lock = ReloadState::Confirmed;
        drop(lock);
        println!("hotline_rs::reload:: confirmed");
        
        // wait for reload to complete
        hot_lib::subscribe().wait_for_reload();
    }

    pub fn lock(&self) -> Arc<Mutex<ReloadState>> {
        self.lock.clone()
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

    ctx.pmfx.load(&hotline_rs::get_data_path("data/shaders/imdraw").as_str())?;
    
    ctx.pmfx.create_render_graph(& mut ctx.device, "forward")?;
    ctx.pmfx.create_pipeline(&mut ctx.device, "imdraw_blit", &ctx.swap_chain.get_backbuffer_pass())?;

    //
    // build schedules
    //

    let mut run_startup = true;
    let mut imgui_open = true;
    
    let mut world = World::new();
    let mut schedule = Schedule::default();
    let mut startup_schedule = Schedule::default();

    let mut current_update_graph = ctx.pmfx.active_update_graph.to_string();

    let reloader = Reloader::create(ReloaderInfo{
        files: vec![]
    });

    reloader.start();

    while ctx.app.run() {

        // sync
        if reloader.check_for_reload() == ReloadResult::Reload {
            // drop everything while its safe.. might want to wait for the GPU
            startup_schedule = Schedule::default();
            schedule = Schedule::default();
            world = World::new();

            // safe to continue
            reloader.complete_reload();

            // run startup again
            run_startup = true;
        }
        let reload_lock = reloader.lock();
        let reload_lock = reload_lock.lock().unwrap();

        // check for changes
        let graph = ctx.pmfx.active_update_graph.to_string();
        if current_update_graph != graph {
            startup_schedule = Schedule::default();
            schedule = Schedule::default();
            world = World::new();
            current_update_graph = graph;
            run_startup = true;
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
            let graph = &ctx.pmfx.active_update_graph;
            let setup_funcs = ctx.pmfx.get_setup_function_names(graph);
            let mut startup_stage = SystemStage::parallel();
            for func_name in &setup_funcs {
                if let Some(func) = get_system_function(func_name) {
                    startup_stage = startup_stage.with_system(func);
                }
            }
            startup_schedule.add_stage(StageStartup, startup_stage);

            // add update funcs by name
            let update_funcs = ctx.pmfx.get_update_function_names(graph);
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