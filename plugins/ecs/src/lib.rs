// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::pmfx::CameraConstants;
use hotline_rs::prelude::*;
use maths_rs::prelude::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use std::collections::HashMap;

macro_rules! log_error {
    ($map:expr, $name:expr) => {
        if !$map.contains_key(&$name) {
            $map.insert($name, Vec::new());
        }
    }
}

struct BevyPlugin {
    world: World,
    setup_schedule: Schedule,
    schedule: Schedule,
    schedule_info: ScheduleInfo,
    run_setup: bool,
    session_info: SessionInfo,
    errors: HashMap<String, Vec<String>>,
    render_graph_hash: pmfx::PmfxHash
}

type PlatformClient = Client<gfx_platform::Device, os_platform::App>;
type PlatformImgui = imgui::ImGui<gfx_platform::Device, os_platform::App>;

fn update_world_matrices(
    mut query: Query<(&Position, &Rotation, &Scale, &mut WorldMatrix)>) {
    // bake a local matrix from position, rotation and scale
    for (position, rotation, scale, mut world_matrix) in &mut query {
        let translate = Mat4f::from_translation(position.0);
        let scale = Mat4f::from_scale(scale.0);
        world_matrix.0 = translate * scale;
    }
}

fn update_billboard_matrices(
    mut query: Query<(&Position, &Rotation, &mut WorldMatrix), With<Billboard>>) {
    for (position, rotation, world_matrix) in &mut query {

    }
}

fn update_main_camera_config(
    main_window: Res<MainWindowRes>,
    mut info: ResMut<SessionInfo>,
    mut query: Query<(&Position, &Rotation), With<MainCamera>>) {
    let window_rect = main_window.0.get_viewport_rect();
    let aspect = window_rect.width as f32 / window_rect.height as f32;
    for (position, rotation) in &mut query {
        info.main_camera = Some(CameraInfo{
            pos: (position.0.x, position.0.y, position.0.z),
            rot: (rotation.0.x, rotation.0.y, rotation.0.z),
            fov: 60.0,
            aspect: aspect
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
    let view = view.transpose();
    proj * view
}

pub fn camera_constants_from(pos: &Position, rot: &Rotation, aspect: f32, fov_degrees: f32) -> CameraConstants {
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
    CameraConstants {
        view_matrix: view,
        projection_matrix: proj,
        view_projection_matrix: proj * view
    }
}

fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut query: Query<(&Name, &mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.0;
    for (name, mut position, mut rotation, mut view_proj) in &mut query {

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
        let aspect = pmfx.0.get_window_aspect("main_dock");
       
        // assign view proj
        view_proj.0 = camera_view_proj_from(&position, &rotation, aspect, 60.0);

        // update camera in pmfx
        pmfx.0.update_camera_constants(&name.0, &camera_constants_from(&position, &rotation, aspect, 60.0));
    }
}

fn render_grid(
    mut device: ResMut<DeviceRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: Res<PmfxRes>) {

    let imdraw = &mut imdraw.0;
    let pmfx = &pmfx.0;

    let view = pmfx.get_view("grid");
    if view.is_err() {
        return;
    }
    
    let arc_view = view.unwrap();
    let mut view = arc_view.lock().unwrap();

    let bb = view.cmd_buf.get_backbuffer_index();
    let fmt = view.pass.get_format_hash();

    let pipeline = pmfx.get_render_pipeline_for_format("imdraw_3d", fmt);
    if pipeline.is_err() {
        return;
    }
    let pipeline = pipeline.unwrap();

    let camera = pmfx.get_camera_constants(&view.camera);
    if camera.is_err() {
        return;
    }
    let camera = camera.unwrap();

    // render grid
    let scale = 1000.0;
    let divisions = 10.0;
    for i in 0..((scale * 2.0) /divisions) as usize {
        let offset = -scale + i as f32 * divisions;
        let mut tint = 0.3;
        if i % 5 == 0 {
            tint *= 0.5;
        }
        if i % 10 == 0 {
            tint *= 0.25;
        }
        if i % 20 == 0 {
            tint *= 0.125;
        }

        imdraw.add_line_3d(Vec3f::new(offset, 0.0, -scale), Vec3f::new(offset, 0.0, scale), Vec4f::from(tint));
        imdraw.add_line_3d(Vec3f::new(-scale, 0.0, offset), Vec3f::new(scale, 0.0, offset), Vec4f::from(tint));
    }

    imdraw.submit(&mut device.0, bb as usize).unwrap();

    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, &camera.view_projection_matrix);

    imdraw.draw_3d(&mut view.cmd_buf, bb as usize);

    view.cmd_buf.end_render_pass();
}

impl BevyPlugin {
    /// Finds get_system calls inside ecs compatible plugins, call the function `get_system_<lib_name>` to disambiguate
    fn get_system_function(&self, name: &str, view_name: &str, client: &PlatformClient) -> Option<SystemDescriptor> {
        for (lib_name, lib) in &client.libs {
            unsafe {
                let function_name = format!("get_system_{}", lib_name).to_string();
                let hook = lib.get_symbol::<unsafe extern fn(String, String) -> Option<SystemDescriptor>>(function_name.as_bytes());
                if hook.is_ok() {
                    let hook_fn = hook.unwrap();
                    let desc = hook_fn(name.to_string(), view_name.to_string());
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
                let list = lib.get_symbol::<unsafe extern fn() ->  Vec<String>>(function_name.as_bytes());
                if let Ok(list_fn) = list {
                    let mut lib_demos = list_fn();
                    demos.append(&mut lib_demos);
                }
            }
        }
        demos
    }

    // Default_setup, creates a render graph and update functions which are hooked into the scheduler
    fn default_demo_shedule(&self) -> ScheduleInfo {
        ScheduleInfo {
            setup: Vec::new(),
            update: vec![
                "update_cameras".to_string(),
                "update_main_camera_config".to_string()
            ],
            render_graph: "mesh_debug".to_string(),
            ..Default::default()
        }
    }

    /// Find the `ScheduleInfo` within loaded plugins for the chosen `demo` or return the default otherwise
    fn get_demo_schedule_info(&self, client: &mut PlatformClient) -> Option<ScheduleInfo> {
        // Get schedule info from the chosen demo
        if !self.session_info.active_demo.is_empty() {
            for (_, lib) in &client.libs {
                unsafe {
                    let function_name = format!("{}", self.session_info.active_demo).to_string();
                    let demo = lib.get_symbol::<unsafe extern fn(&mut PlatformClient) -> ScheduleInfo>(function_name.as_bytes());
                    if let Ok(demo_fn) = demo {
                        return Some(demo_fn(client));
                    }
                }
            }
        }
        None
    }

    /// If we change demo or need to rebuild render graphs we need to invoke this, code changes will already invoke setup
    fn resetup(&mut self, mut client: PlatformClient) -> PlatformClient {
        // serialize
        client.serialise_plugin_data("ecs", &self.session_info);
        // unload / setup
        client.swap_chain.wait_for_last_frame();
        client = self.unload(client);
        self.setup(client)
    }

    /// Custom function to handle custome data change events which can trigger resetup
    fn check_for_changes(&mut self, client: PlatformClient) -> PlatformClient {
        // rendere graph itself has chaned
        if self.render_graph_hash != client.pmfx.get_render_graph_hash(&self.schedule_info.render_graph) {
            self.resetup(client)
        }
        else {
            client
        }
    }

    fn status_ui_category(&self, imgui: &mut PlatformImgui, header: &str, function_list: &Vec<String>) {
        let error_col = vec4f(1.0, 0.0, 0.3, 1.0);
        let default_col = vec4f(1.0, 1.0, 1.0, 1.0);
        if function_list.len() > 0 {
            imgui.text(header);
            if function_list.len() > 0 {
                for f in function_list {
                    if self.errors.contains_key(f) {
                        imgui.colour_text(&format!("  missing function: `{}`", f), error_col);
                    }
                    else {
                        imgui.colour_text(&format!("  {}", f), default_col);
                    }
                }
            }
        }
    }

    fn schedule_ui(&mut self, mut client: PlatformClient) -> PlatformClient {
        let error_col = vec4f(1.0, 0.0, 0.3, 1.0);
        let warning_col = vec4f(1.0, 7.0, 0.0, 1.0);
        let default_col = vec4f(1.0, 1.0, 1.0, 1.0);
        
        // schedule
        client.imgui.separator();
        client.imgui.text("Schedule");
        client.imgui.separator();

        // warn of missing demo
        if self.errors.contains_key("active_demo") {
            client.imgui.colour_text(&format!("warning: missing demo function: {}, using default schedule.", self.session_info.active_demo), warning_col);
        }

        self.status_ui_category(&mut client.imgui, "Setup:", &self.schedule_info.setup);
        self.status_ui_category(&mut client.imgui, "Update:", &self.schedule_info.update);

        let graph = &self.schedule_info.render_graph;

        if self.errors.contains_key(graph) {
            client.imgui.colour_text(
                &format!("Render Graph: {}: {}.", "missing", graph), 
                error_col
            );

            for err in &self.errors[graph] {
                client.imgui.colour_text(
                    &format!("  {}", err), 
                    error_col
                );
            }
        }
        else {
            let render_functions = client.pmfx.get_render_graph_function_info(graph);
            let mut render_function_names = Vec::new();
            for v in render_functions {
                render_function_names.push(v.0);
            }
            self.status_ui_category(
                &mut client.imgui, 
                &format!("Render Graph ({}):", graph),
                &render_function_names
            );

            // actual exec order of the GPU command queue
            let queue = client.pmfx.get_render_graph_execute_order();
            let view_errors = client.pmfx.view_errors.lock().unwrap();
            client.imgui.text(&format!("Command Queue ({}):", graph));

            // flag missing views
            for (k, v) in &*view_errors {
                if !queue.contains(k) {
                    client.imgui.colour_text(&format!("  {}: error: `{}`", k, v), error_col);
                }
            }

            // flag errors with present views
            for f in queue {
                if view_errors.contains_key(f) {
                    client.imgui.colour_text(&format!("  {}: error: `{}`", f, view_errors[f]), error_col);
                }
                else {
                    client.imgui.colour_text(&format!("  {}", f), default_col);
                }
            }
        }

        client
    }
}

impl Plugin<gfx_platform::Device, os_platform::App> for BevyPlugin {
    fn create() -> Self {
        BevyPlugin {
            world: World::new(),
            setup_schedule: Schedule::default(),
            schedule: Schedule::default(),
            schedule_info: ScheduleInfo::default(),
            render_graph_hash: 0,
            run_setup: false,
            session_info: SessionInfo::default(),
            errors: HashMap::new()
        }
    }

    fn setup(&mut self, mut client: PlatformClient) -> PlatformClient {
        // clear errors
        self.errors = HashMap::new();

        self.session_info = client.deserialise_plugin_data("ecs");

        // dynamically change demos and lookup infos in other libs
        let schedule_info = self.get_demo_schedule_info(&mut client);
        
        // get schedule or use default and warn the user 
        self.schedule_info = if let Some(info) = schedule_info {
            info
        }
        else {
            log_error!(self.errors, "active_demo".to_string());
            self.default_demo_shedule()
        };

        // build render graph
        let graph = self.schedule_info.render_graph.to_string();
        let graph_result = client.pmfx.create_render_graph(&mut client.device, &graph);

        let render_functions = if let Err(error) = graph_result {
            // if render graph fails to build, use the default and log errors for the user
            self.schedule_info = ScheduleInfo::default();
            let ext_msg = format!("{} (Check GPU Validation Messages For More Info)", error.msg);
            self.errors.entry(graph.to_string()).or_insert(Vec::new()).push(ext_msg);
            self.schedule_info.render_graph = graph.to_string();
            Vec::new()
        }
        else {
            client.pmfx.get_render_graph_function_info(&graph)
        };

        let info = &self.schedule_info;

        // hook in setup funcs
        let mut setup_stage = SystemStage::parallel();
        for func_name in &info.setup {
            if let Some(func) = self.get_system_function(func_name, "", &client) {
                setup_stage = setup_stage.with_system(func);
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }
        self.setup_schedule.add_stage(StageStartup, setup_stage);

        // hook in updates funcs
        let mut update_stage = SystemStage::parallel();
        for func_name in &info.update {
            if let Some(func) = self.get_system_function(func_name, "", &client) {
                update_stage = update_stage.with_system(func);
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }
        self.schedule.add_stage(StageUpdate, update_stage);

        // batch functions do syncronised work to prpare buffers / matrices for drawing
        let batch_stage = SystemStage::parallel()
            .with_system(update_world_matrices)
            .with_system(update_billboard_matrices);
        self.schedule.add_stage(StageBatch, batch_stage);

        // hook in render functions
        let mut render_stage = SystemStage::parallel();
        for (func_name, view_name) in &render_functions {
            if let Some(func) = self.get_system_function(func_name, view_name, &client) {
                render_stage = render_stage.with_system(func.after("temp-debug"));
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }
        self.render_graph_hash = client.pmfx.get_render_graph_hash(&info.render_graph);
        self.schedule.add_stage(StageRender, render_stage);

        // we defer the actual setup system calls until the update where resources will be inserted into the world
        self.run_setup = true;
        client
    }

    fn update(&mut self, mut client: PlatformClient) -> PlatformClient {

        let session_info = self.session_info.clone();

        // check for any changes
        client = self.check_for_changes(client);

        // clear pmfx view errors before we render
        client.pmfx.view_errors.lock().unwrap().clear();

        // move hotline resource into world
        self.world.insert_resource(session_info);
        self.world.insert_resource(DeviceRes(client.device));
        self.world.insert_resource(AppRes(client.app));
        self.world.insert_resource(MainWindowRes(client.main_window));
        self.world.insert_resource(PmfxRes(client.pmfx));
        self.world.insert_resource(ImDrawRes(client.imdraw));
        self.world.insert_resource(UserConfigRes(client.user_config));

        // run setup if requested, we did it here so hotline resources are inserted into World
        if self.run_setup {
            let main_camera = self.session_info.main_camera.unwrap_or_default();
            let pos = Position { 0: Vec3f::new(main_camera.pos.0, main_camera.pos.1, main_camera.pos.2) };
            let rot = Rotation { 0: Vec3f::new(main_camera.rot.0, main_camera.rot.1, main_camera.rot.2) };

            self.world.spawn((
                ViewProjectionMatrix(camera_view_proj_from(&pos, &rot, main_camera.aspect, main_camera.fov)),
                pos,
                rot,
                Camera,
                MainCamera,
                Name(String::from("main_camera"))
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
        client.serialise_plugin_data("ecs", &self.session_info);

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
        let mut open = true;
        let mut resetup = false;
        if client.imgui.begin("ecs", &mut open, imgui::WindowFlags::NONE) {
            // refresh button
            if client.imgui.button("\u{f021}") {
                resetup = true;
            }
            client.imgui.same_line();

            // demo select
            let demo_list = self.get_demo_list(&client);
            let (open, selected) = client.imgui.combo_list("Demo", &demo_list, &self.session_info.active_demo);
            if open {
                if selected != self.session_info.active_demo {
                    // update session info
                    self.session_info.active_demo = selected;
                    resetup = true;
                }
            }

            client = self.schedule_ui(client);
        }

        // preform any re-setup actions
        if resetup {
            client = self.resetup(client);
        }

        client.imgui.end();
        client
    }
}

//
// Plugin
//

hotline_plugin![BevyPlugin];

/// Register plugin systems
#[no_mangle]
pub fn get_system_ecs(name: String, _view_name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "update_cameras" => system_func![update_cameras],
        "update_main_camera_config" => system_func![update_main_camera_config],
        "render_grid" => system_func![render_grid],
        _ => None
    }
}