// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;
use hotline_rs::plugin::*;
use ecs::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::Mat4f;

use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;

use hotline_rs::os::App;
use hotline_rs::os::Window;

//use hotline_rs::pmfx;
//use hotline_rs::imdraw;

use hotline_rs::imgui;

use hotline_rs::gfx_platform;
use hotline_rs::os_platform;

use hotline_rs::gfx;
use hotline_rs::os;

use gfx::RenderPass;
use gfx::CmdBuf;

struct BevyPlugin {
    world: World,
    setup_schedule: Schedule,
    schedule: Schedule,
    run_setup: bool,
    demo_list: Vec<String>,
    demo: String
}

use hotline_rs::system_func;

fn update_main_camera_config(
    mut config: ResMut<UserConfigRes>, 
    mut query: Query<(&Position, &Rotation), With<MainCamera>>) {
    for (position, rotation) in &mut query {
        config.0.main_camera = Some(CameraInfo{
            pos: (position.0.x, position.0.y, position.0.z),
            rot: (rotation.0.x, rotation.0.y, rotation.0.z),
            fov: 60.0,
            aspect: 16.0/9.0
        });
    }
}

pub fn camera_view_proj_from(pos: &Position, rot: &Rotation, aspect: f32, fov_degrees: f32) -> Mat4f {
    // rotational matrix
    let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rot.0.x));
    let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rot.0.y));
    let mat_rot = mat_rot_y * mat_rot_x;
    // generate proj matrix
    let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(fov_degrees), aspect, 0.1, 10000.0);
    // translation matrix
    let translate = Mat4f::from_translation(pos.0);
    // build view / proj matrix
    let view = translate * mat_rot;
    let view = view.inverse();
    proj * view
}

fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>, 
    mut query: Query<(&mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.0;
    for (mut position, mut rotation, mut view_proj) in &mut query {

        if main_window.0.is_focused() {
            // get keyboard position movement
            let keys = app.get_keys_down();
            let mut cam_move_delta = Vec3f::zero();
            if keys['A' as usize] {
                cam_move_delta.x -= 1.0;
            }
            if keys['D' as usize] {
                cam_move_delta.x += 1.0;
            }
            if keys['Q' as usize] {
                cam_move_delta.y -= 1.0;
            }
            if keys['E' as usize] {
                cam_move_delta.y += 1.0;
            }
            if keys['W' as usize] {
                cam_move_delta.z -= 1.0;
            }
            if keys['S' as usize] {
                cam_move_delta.z += 1.0;
            }

            // get mouse rotation
            if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
                let mouse_delta = app.get_mouse_pos_delta();
                rotation.0.x -= mouse_delta.y as f32;
                rotation.0.y -= mouse_delta.x as f32;
            }

            // construct rotation matrix
            let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rotation.0.x));
            let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rotation.0.y));
            let mat_rot = mat_rot_y * mat_rot_x;

            // move relative to facing directions
            position.0 += mat_rot * cam_move_delta;
        }

        // generate proj matrix
        let window_rect = main_window.0.get_viewport_rect();
        let aspect = window_rect.width as f32 / window_rect.height as f32;
       
        // assign view proj
        view_proj.0 = camera_view_proj_from(&position, &rotation, aspect, 60.0);
    }
}

fn render_grid(
    mut device: ResMut<DeviceRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: Res<PmfxRes>,
    mut query: Query<&ViewProjectionMatrix> ) {

    let arc_view = pmfx.0.get_view("render_grid").unwrap();
    let mut view = arc_view.lock().unwrap();
    let bb = view.cmd_buf.get_backbuffer_index();
    let fmt = view.pass.get_format_hash();

    for view_proj in &mut query {
        // render grid
        let imdraw = &mut imdraw.0;
        let pmfx = &pmfx.0;

        let scale = 1000.0;
        let divisions = 10.0;
        for i in 0..((scale * 2.0) /divisions) as usize {
            let offset = -scale + i as f32 * divisions;
            imdraw.add_line_3d(Vec3f::new(offset, 0.0, -scale), Vec3f::new(offset, 0.0, scale), Vec4f::from(0.3));
            imdraw.add_line_3d(Vec3f::new(-scale, 0.0, offset), Vec3f::new(scale, 0.0, offset), Vec4f::from(0.3));
        }

        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 0.0, 1000.0), Vec4f::blue());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(1000.0, 0.0, 0.0), Vec4f::red());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 1000.0, 0.0), Vec4f::green());

        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 0.0, -1000.0), Vec4f::yellow());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(-1000.0, 0.0, 0.0), Vec4f::cyan());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, -1000.0, 0.0), Vec4f::magenta());

        imdraw.submit(&mut device.0, bb as usize).unwrap();

        view.cmd_buf.begin_render_pass(&view.pass);
        view.cmd_buf.set_viewport(&view.viewport);
        view.cmd_buf.set_scissor_rect(&view.scissor_rect);

        view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline_for_format("imdraw_3d", fmt).unwrap());
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);

        imdraw.draw_3d(&mut view.cmd_buf, bb as usize);

        view.cmd_buf.end_render_pass();
    }
}

fn render_world_view(
    pmfx: Res<PmfxRes>,
    view_name: String,
    view_proj_query: Query<&ViewProjectionMatrix>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;

    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);
    view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline_for_format("imdraw_mesh", fmt).unwrap());

    for view_proj in &view_proj_query {
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);
        for (world_matrix, mesh) in &mesh_draw_query {
            // draw
            view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
            view.cmd_buf.set_index_buffer(&mesh.0.ib);
            view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
            view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        }
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();
}

#[no_mangle]
pub fn get_system_ecs(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "update_cameras" => system_func![update_cameras],
        "update_main_camera_config" => system_func![update_main_camera_config],
        "render_grid" => system_func![render_grid],
        "render_world_view" => view_func![render_world_view, "render_world_view"],
        _ => None
    }
}

impl BevyPlugin {
    /// Finds get_system calls inside ecs compatible plugins, call the function `get_system_<lib_name>` to disambiguate
    fn get_system_function(&self, name: &str, client: &Client<gfx_platform::Device, os_platform::App>) -> Option<SystemDescriptor> {
        for (lib_name, lib) in &client.libs {
            unsafe {
                let function_name = format!("get_system_{}", lib_name).to_string();
                let hook = lib.get_symbol::<unsafe extern fn(String) -> Option<SystemDescriptor>>(function_name.as_bytes());
                if hook.is_ok() {
                    let hook_fn = hook.unwrap();
                    let desc = hook_fn(name.to_string());
                    if desc.is_some() {
                        return desc;
                    }
                }
            }
        }
        None
    }

    /// Finds available demo names from inside ecs compatible plugins, call the function `get_system_<lib_name>` to disambiguate
    fn get_demo_list(&self, client: &Client<gfx_platform::Device, os_platform::App>) -> Vec<String> {
        let mut demos = Vec::new();
        for (lib_name, lib) in &client.libs {
            unsafe {
                let function_name = format!("get_demos_{}", lib_name).to_string();
                let demo = lib.get_symbol::<unsafe extern fn() ->  Vec<String>>(function_name.as_bytes());
                if demo.is_ok() {
                    let demo_fn = demo.unwrap();
                    let mut lib_demos = demo_fn();
                    demos.append(&mut lib_demos);
                }
            }
        }
        demos
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
            "cube".to_string(),
        ];
        let update_systems = vec![
            "mat_movement".to_string(),
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ];
        let render_systems = client.pmfx.get_render_function_names("forward");

        let list = self.get_demo_list(&client);
        for item in list {
            println!("{}", item);
        }
        
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

        let main_camera = if let Some(main_camera) = &client.user_config.main_camera {
            main_camera.clone()
        }
        else {
            CameraInfo {
                pos: (0.0, 100.0, 0.0),
                rot: (-45.0, 0.0, 0.0),
                aspect: 16.0/9.0,
                fov: 60.0
            }
        };

        // move hotline resource into world
        self.world.insert_resource(DeviceRes {0: client.device});
        self.world.insert_resource(AppRes {0: client.app});
        self.world.insert_resource(MainWindowRes {0: client.main_window});
        self.world.insert_resource(PmfxRes {0: client.pmfx});
        self.world.insert_resource(ImDrawRes {0: client.imdraw});
        self.world.insert_resource(ImGuiRes {0: client.imgui});
        self.world.insert_resource(UserConfigRes {0: client.user_config});

        // run setup if requested, we did it here so hotline resources are inserted into World
        if self.run_setup {

            let pos = Position { 0: Vec3f::new(main_camera.pos.0, main_camera.pos.1, main_camera.pos.2) };
            let rot = Rotation { 0: Vec3f::new(main_camera.rot.0, main_camera.rot.1, main_camera.rot.2) };

            self.world.spawn((
                ViewProjectionMatrix(camera_view_proj_from(&pos, &rot, 16.0/9.0, 60.0)),
                pos,
                rot,
                Camera,
                MainCamera
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
        client.user_config = self.world.remove_resource::<UserConfigRes>().unwrap().0;
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