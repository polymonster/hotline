// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;
use ecs::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;

struct BevyPlugin {
    world: World,
    setup_schedule: Schedule,
    schedule: Schedule,
    run_setup: bool,
    demo_list: Vec<String>,
    demo: String
}

use hotline_rs::system_func;

#[no_mangle]
pub fn setup_single(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::identity()}
    ));

    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::from_translation(vec3f(0.0, 0.0, 0.0))}
    ));

    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::from_translation(vec3f(0.0, 0.0, 0.0))}
    ));
}


#[no_mangle]
pub fn setup_multiple(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands:  bevy_ecs::system::Commands) {

    commands.spawn((
        ecs::Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
        ecs::Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ecs::ViewProjectionMatrix { 0: Mat4f::identity()},
        ecs::Camera,
    ));

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    let dim = 500;
    let dim2 = dim / 2;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;

            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            commands.spawn((
                ecs::Position { 0: Vec3f::zero() },
                ecs::Velocity { 0: Vec3f::one() },
                ecs::MeshComponent {0: cube_mesh.clone()},
                ecs::WorldMatrix { 0: Mat4f::from_translation(
                    vec3f(
                        x as f32 * 2.5 - dim2 as f32 * 2.5, 
                        0.0, 
                        y as f32 * 2.5 - 2.5 * dim as f32)) * 
                        Mat4::from_scale(vec3f(1.0, wave_x + wave_y + wave_h, 1.0)) }
            ));
        }
    }
}

#[no_mangle]
pub fn movement(mut query:  bevy_ecs::system::Query<(&mut ecs::Position, &ecs::Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

#[no_mangle]
pub fn mat_movement(mut query:  bevy_ecs::system::Query<&mut ecs::WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.0, 0.0));
    }
}

#[no_mangle]
pub fn get_demo_names() -> Vec<String> {
    vec![
        "single".to_string(),
        "multiple".to_string()
    ]
}

#[no_mangle]
pub fn get_system_function_lib(name: &str) -> Option<bevy_ecs::schedule::SystemDescriptor> {    
    match name {
        "mat_movement" => system_func![mat_movement],
        "setup_single" => system_func![setup_single],
        "setup_multiple" => system_func![setup_multiple],
        _ => None
    }
}

impl BevyPlugin {
    fn get_system_function(&self, name: &str, _client: &Client<gfx_platform::Device, os_platform::App>) -> Option<SystemDescriptor> {
        let func = ecs::get_system_function(name);
        if func.is_some() {
            func
        }
        else {
            get_system_function_lib(name)

            // TODO: downcast
            /*
            let responder = client.get_responder().unwrap();
            let responder = responder.lock().unwrap();
            let lib = responder.as_any().downcast_ref::<client::LibReloadResponder>().unwrap();

            let sym = lib.get_symbol::<unsafe extern fn(String) -> Option<SystemDescriptor>>("get_system_function_lib");
            unsafe {
                let f = sym.unwrap()(name.to_string());
                if f.is_some() {
                    f
                }
                else {
                    None
                }
            }
            */
        }
    }
}

impl Plugin<gfx_platform::Device, os_platform::App> for BevyPlugin {
    fn create() -> Self {
        BevyPlugin {
            world: World::new(),
            setup_schedule: Schedule::default(),
            schedule: Schedule::default(),
            run_setup: false,
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
            if let Some(func) = self.get_system_function(func_name, &client) {
                render_stage = render_stage.with_system(func);
            }
        }
        self.schedule.add_stage(StageRender, render_stage);

        // add startup funcs by name
        let mut setup_stage = SystemStage::parallel();
        for func_name in &setup_systems {
            if let Some(func) = self.get_system_function(func_name, &client) {
                setup_stage = setup_stage.with_system(func);
            }
        }
        self.setup_schedule.add_stage(StageStartup, setup_stage);

        // add update funcs by name
        let mut update_stage = SystemStage::parallel();
        for func_name in &update_systems {
            if let Some(func) = self.get_system_function(func_name, &client) {
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

impl<D, A> imgui::UserInterface<D, A> for BevyPlugin where D: gfx::Device, A: os::App {
    fn show_ui(&mut self, imgui: &imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("bevy", &mut imgui_open, imgui::WindowFlags::NONE) {
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

hotline_plugin![BevyPlugin];