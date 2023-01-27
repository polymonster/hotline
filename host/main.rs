// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;

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
use std::thread;
use std::sync::Arc;
use std::sync::Mutex;

use libloading::Symbol;

#[derive(PartialEq)]
enum ReloadState {
    None,
    Requested,
    Confirmed,
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

    fn file_watcher_thread(&self) {
        let lib_path = hotline_rs::get_data_path("../lib/src/lib.rs");
        let mut lib_modified_time = std::fs::metadata(&lib_path).unwrap().modified().unwrap();
    
        let pmfx_path = hotline_rs::get_data_path("../src/shaders/imdraw.pmfx");
        let pmfx_modified_tome = std::fs::metadata(&pmfx_path).unwrap().modified().unwrap();
    
        let lock = self.lock.clone();

        thread::spawn(move || {
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
                        .arg("--release")
                        .output()
                        .expect("hotline::hot_lib:: hot lib failed to build!");
        
                    if output.stdout.len() > 0 {
                        println!("{}", String::from_utf8(output.stdout).unwrap());
                    }
        
                    if output.stderr.len() > 0 {
                        println!("{}", String::from_utf8(output.stderr).unwrap());
                    }

                    let mut a = lock.lock().unwrap();
                    println!("hotline_rs::reload:: requested");
                    *a = ReloadState::Requested;
                    drop(a);
        
                    lib_modified_time = cur_lib_modified_time;
                }
        
                // check shader changes
                let cur_pmfx_modified_time = std::fs::metadata(&pmfx_path).unwrap().modified().unwrap();
                if cur_pmfx_modified_time > pmfx_modified_tome {
                    println!("hotline_rs::hot_lib:: pmfx changes detected");
                    // kick off a build
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
        });
    }

    /// Start watching for and invoking reload changes, this will spawn threads to watch files
    pub fn start(&self) {
        self.file_watcher_thread();
    }

    pub fn check_for_reload(&self) -> ReloadResult {
        let lock = self.lock.lock().unwrap();
        if *lock == ReloadState::Requested {
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
    }
}

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

    fn add_lib(&mut self, path: &str, name: &str) {
        self.libs.push(hot_lib_reloader::LibReloader::new(path, name, None).unwrap());

        self.demo_list = Vec::new();
        for lib in &self.libs {
            unsafe {
                let get_demo_names : Symbol<unsafe extern fn() -> Vec<String>> 
                    = lib.get_symbol("get_demo_names".as_bytes()).unwrap();
                self.demo_list.append(&mut get_demo_names());
            }
        }
    }
}

impl Runner<gfx_platform::Device, os_platform::App> for BevyRunner {
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

    fn setup(
        &mut self, 
        setup_systems: Vec<String>, 
        update_systems: Vec<String>, 
        render_systems: Vec<String>) {
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

    fn reload(&mut self) {
        // drop everything while its safe
        self.setup_schedule = Schedule::default();
        self.schedule = Schedule::default();
        self.world = World::new();
    }
}

impl<D, A> UserInterface<D, A> for BevyRunner where D: gfx::Device, A: os::App {
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

    //
    // create context
    //
    
    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        ..Default::default()
    })?;

    ctx.pmfx.create_render_graph(& mut ctx.device, "forward")?;
    let render_funcs = ctx.pmfx.get_render_function_names("forward");
    
    let mut runner = BevyRunner::create();
    runner.add_lib("target/release/", "lib");
    
    let mut imgui_open = true;

    let reloader = Reloader::create(ReloaderInfo{
        files: vec![]
    });

    // TODO: functions from config
    runner.setup(
        vec![
            "setup_single".to_string(),
        ], 
        vec![
            "mat_movement".to_string(),
            "update_cameras".to_string()
        ], 
        render_funcs.to_vec()
    );

    reloader.start();

    while ctx.app.run() {
    
        // sync
        if reloader.check_for_reload() == ReloadResult::Reload {
            ctx.swap_chain.wait_for_last_frame();

            // allow the runner to drop
            runner.reload();

            // safe to continue
            reloader.complete_reload();

            loop {
                if runner.libs[0].update().unwrap() {
                    println!("reload success");
                    break;
                }
                std::thread::sleep(Duration::from_millis(16));
            }

            // run startup again
            runner.setup(
                vec![
                    "setup_single".to_string(),
                ], 
                vec![
                    "mat_movement".to_string(),
                    "update_cameras".to_string()
                ], 
                render_funcs.to_vec()
            );
        }

        ctx.new_frame();
        ctx = runner.update(ctx);

        imgui_open = ctx.pmfx.show_ui(&ctx.imgui, imgui_open);
        imgui_open = runner.show_ui(&ctx.imgui, imgui_open);

        // present to back buffer
        ctx.present("main_colour");
    }

    ctx.wait_for_last_frame();

    // exited with code 0
    Ok(())
}