// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;
use hotline_rs::client::*;
use hotline_rs::plugin::*;

use ecs_base::*;

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

use hotline_rs::gfx_platform;
use hotline_rs::os_platform;

use hotline_rs::gfx;
use hotline_rs::os;

use gfx::RenderPass;
use gfx::CmdBuf;
use gfx::SwapChain;

struct BevyPlugin {
    world: World,
    setup_schedule: Schedule,
    schedule: Schedule,
    run_setup: bool,
    session_info: SessionInfo
}

use ecs_base::SheduleInfo;

type PlatformClient = Client<gfx_platform::Device, os_platform::App>;

fn update_main_camera_config(
    mut info: ResMut<SessionInfo>, 
    mut query: Query<(&Position, &Rotation), With<MainCamera>>) {
    for (position, rotation) in &mut query {
        info.main_camera = Some(CameraInfo{
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

        let (enable_keyboard, enable_mouse) = app.get_input_enabled();
        if main_window.0.is_focused() {

            let mut cam_move_delta = Vec3f::zero();

            if enable_keyboard {
                // get keyboard position movement
                let keys = app.get_keys_down();
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
            }

            // get mouse rotation
            if enable_mouse {
                if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
                    let mouse_delta = app.get_mouse_pos_delta();
                    rotation.0.x -= mouse_delta.y as f32;
                    rotation.0.y -= mouse_delta.x as f32;
                }
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

    let constant_colour_mesh = pmfx.get_render_pipeline_for_format("constant_colour_mesh", fmt);
    if constant_colour_mesh.is_none() {
        return;
    }

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&constant_colour_mesh.unwrap());

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

impl BevyPlugin {
    /// Finds get_system calls inside ecs compatible plugins, call the function `get_system_<lib_name>` to disambiguate
    fn get_system_function(&self, name: &str, client: &PlatformClient) -> Option<SystemDescriptor> {
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
    fn get_demo_list(&self, client: &PlatformClient) -> Vec<String> {
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

    // Default_setup, creates a render graph and update functions which are hooked into the scheduler
    fn default_demo_shedule(&self, client: &mut PlatformClient) -> SheduleInfo {
        client.pmfx.create_render_graph(&mut client.device, "forward").unwrap();
        SheduleInfo {
            update: vec![
                "mat_movement".to_string(),
                "update_cameras".to_string(),
                "update_main_camera_config".to_string()
            ],
            render: client.pmfx.get_render_function_names("forward"),
            setup: Vec::new()
        }
    }

    /// Find the `SheduleInfo` within loaded plugins for the chosen `demo` or return the default otherwise
    fn get_demo_schedule_info(&self, client: &mut PlatformClient) -> SheduleInfo {
        // Get schedule info from the chosen demo
        if !self.session_info.active_demo.is_empty() {
            for (_, lib) in &client.libs {
                unsafe {
                    let function_name = format!("{}", self.session_info.active_demo).to_string();
                    let demo = lib.get_symbol::<unsafe extern fn(&mut PlatformClient) -> SheduleInfo>(function_name.as_bytes());
                    if demo.is_ok() {
                        let demo_fn = demo.unwrap();
                        return demo_fn(client);
                    }
                }
            }
        }
        self.default_demo_shedule(client)
    }
}

impl Plugin<gfx_platform::Device, os_platform::App> for BevyPlugin {
    fn create() -> Self {
        BevyPlugin {
            world: World::new(),
            setup_schedule: Schedule::default(),
            schedule: Schedule::default(),
            run_setup: false,
            session_info: SessionInfo::default()
        }
    }

    fn setup(&mut self, mut client: PlatformClient) -> PlatformClient {

        // deserialise user data saved from a previous session
        self.session_info = if client.user_config.plugin_data.contains_key("ecs") {
            serde_json::from_slice(&client.user_config.plugin_data["ecs"].as_bytes()).unwrap()
        }
        else {
            SessionInfo::default()
        };

        // dynamically change demos and lookup infos in other libs
        let info = self.get_demo_schedule_info(&mut client);

        // hook in render functions
        let mut render_stage = SystemStage::parallel();
        for func_name in &info.render {
            if let Some(func) = self.get_system_function(func_name, &client) {
                render_stage = render_stage.with_system(func);
            }
        }
        self.schedule.add_stage(StageRender, render_stage);

        // hook in startup funcs
        let mut setup_stage = SystemStage::parallel();
        for func_name in &info.setup {
            if let Some(func) = self.get_system_function(func_name, &client) {
                setup_stage = setup_stage.with_system(func);
            }
        }
        self.setup_schedule.add_stage(StageStartup, setup_stage);

        // hook in updates funcs
        let mut update_stage = SystemStage::parallel();
        for func_name in &info.update {
            if let Some(func) = self.get_system_function(func_name, &client) {
                update_stage = update_stage.with_system(func);
            }
        }
        self.schedule.add_stage(StageUpdate, update_stage);

        // we defer the actual setup system calls until the update where resources will be inserted into the world
        self.run_setup = true;
        client
    }

    fn update(&mut self, mut client: PlatformClient) -> PlatformClient {

        let session_info = self.session_info.clone();

        // move hotline resource into world
        self.world.insert_resource(DeviceRes {0: client.device});
        self.world.insert_resource(AppRes {0: client.app});
        self.world.insert_resource(MainWindowRes {0: client.main_window});
        self.world.insert_resource(PmfxRes {0: client.pmfx});
        self.world.insert_resource(ImDrawRes {0: client.imdraw});
        self.world.insert_resource(UserConfigRes {0: client.user_config});
        self.world.insert_resource(session_info);

        // run setup if requested, we did it here so hotline resources are inserted into World
        if self.run_setup {
            let main_camera = self.session_info.main_camera.unwrap_or_default();
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
        client.user_config = self.world.remove_resource::<UserConfigRes>().unwrap().0;
        self.session_info = self.world.remove_resource::<SessionInfo>().unwrap();

        // write back session info which will be serialised to disk and reloaded between sessions
        if let Some(config_info) = client.user_config.plugin_data.get_mut("ecs") {
            *config_info = serde_json::to_string(&self.session_info).unwrap();
        }

        client
    }

    fn unload(&mut self, client: PlatformClient) -> PlatformClient {
        // drop everything while its safe
        self.setup_schedule = Schedule::default();
        self.schedule = Schedule::default();
        self.world = World::new();
        client
    }

    fn ui(&mut self, mut client: PlatformClient) -> PlatformClient {
        // Demo list / demo select
        let demo_list = self.get_demo_list(&client);
        if client.imgui.begin_main_menu_bar() {
            let (open, selected) = client.imgui.combo_list("", &demo_list, &self.session_info.active_demo);
            if open {
                if selected != self.session_info.active_demo {

                    // write back session info
                    self.session_info.active_demo = selected;

                    let serialised = serde_json::to_string(&self.session_info).unwrap();
                    let config = client.user_config.plugin_data.entry("ecs".to_string()).or_insert(String::default());
                    *config = serialised;

                    if let Some(config_info) = client.user_config.plugin_data.get_mut("ecs") {
                        *config_info = serde_json::to_string(&self.session_info).unwrap();
                    }

                    client.swap_chain.wait_for_last_frame();
                    client = self.unload(client);
                    client = self.setup(client);
                    
                }
            }
            client.imgui.end_main_menu_bar();
        }
        client
    }
}

//
// Plugin
//

hotline_plugin![BevyPlugin];

#[no_mangle]
pub fn get_system_ecs(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "update_cameras" => ecs_base::system_func![update_cameras],
        "update_main_camera_config" => ecs_base::system_func![update_main_camera_config],
        "render_grid" => ecs_base::system_func![render_grid],
        "render_world_view" => ecs_base::view_func![render_world_view, "render_world_view"],
        _ => None
    }
}