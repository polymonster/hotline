// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::pmfx::CameraConstants;
use hotline_rs::prelude::*;
use maths_rs::prelude::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemConfig;

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

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
#[system_set(base)]
pub enum CoreSystemSets {
    Update,
    Batch,
    Render,
}

type PlatformClient = Client<gfx_platform::Device, os_platform::App>;
type PlatformImgui = imgui::ImGui<gfx_platform::Device, os_platform::App>;

fn update_world_matrices(
    mut query: Query<(&Position, &mut Rotation, &Scale, &mut WorldMatrix)>) {
    // bake a local matrix from position, rotation and scale
    for (position, rotation, scale, mut world_matrix) in &mut query {
        let translate = Mat34f::from_translation(position.0);
        let rotate = Mat34f::from(rotation.0);
        let scale = Mat34f::from_scale(scale.0);
        world_matrix.0 = translate * rotate * scale;
    }
}

fn update_main_camera_config(
    main_window: Res<MainWindowRes>,
    mut info: ResMut<SessionInfo>,
    mut query: Query<(&Position, &Camera), With<MainCamera>>) {
    let window_rect = main_window.0.get_viewport_rect();
    let aspect = window_rect.width as f32 / window_rect.height as f32;
    for (position, camera) in &mut query {
        info.main_camera = Some(CameraInfo{
            camera_type: camera.camera_type,
            zoom: camera.zoom,
            focus: (camera.focus.x, camera.focus.y, camera.focus.z),
            pos: (position.0.x, position.0.y, position.0.z),
            rot: (camera.rot.x, camera.rot.y, camera.rot.z),
            fov: 60.0,
            aspect: aspect
        });
    }
}

/*
fn create_perspective_projection_lh_yup_reverse_z_internal<T: Float + FloatOps<T>>(left: T, right: T, bottom: T, top: T, near: T, far: T) -> Mat4<T> {
    Mat4::new(
        (T::two() * near) / (right - left), 
        T::zero(), 
        (right + left) / (right - left), 
        T::zero(),

        T::zero(), 
        (T::two() * near) / (top - bottom), 
        (top + bottom) / (top - bottom), 
        T::zero(),

        T::zero(), 
        T::zero(),

        //(-far - near) / (far - near),
        (-near) / (near - far),

        //(-(T::two() * near) * far) / (far - near),
        (-near * far) / (near - far),

        T::zero(), 
        T::zero(), 
        T::minus_one(), 
        T::zero()
    )
}

fn create_perspective_projection_lh_yup_reverse_z<T: Float + FloatOps<T>>(fov: T, aspect: T, near: T, far: T) -> Mat4<T> {
    let tfov = T::tan(fov * T::point_five());
    let right = tfov * aspect * near;
    let left = -right;
    let top = tfov * near;
    let bottom = -top;
    create_perspective_projection_lh_yup_reverse_z_internal(left, right, bottom, top, near, far)
}
*/

pub fn camera_constants_from_fly(pos: &Position, rot: &Vec3f, aspect: f32, fov_degrees: f32) -> CameraConstants {
    // rotational matrix
    let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rot.x));
    let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rot.y));
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
        view_projection_matrix: proj * view,
        view_position: Vec4f::from((pos.0, 0.0))
    }
}

pub fn camera_constants_from_orbit(rot: &Vec3f, focus: &Vec3f, zoom: f32, aspect: f32, fov_degrees: f32) -> CameraConstants {
    // rotational matrix
    let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rot.x));
    let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rot.y));
    let mat_rot = mat_rot_y * mat_rot_x;
    // generate proj matrix
    let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(fov_degrees), aspect, 0.1, 10000.0);
    // translation matrix
    let translate_zoom = Mat4f::from_translation(vec3f(0.0, 0.0, zoom));
    let translate_focus = Mat4f::from_translation(*focus);        
    // build view / proj matrix
    let view = translate_focus * mat_rot * translate_zoom;
    let pos = view.get_column(3);
    let view = view.inverse();
    CameraConstants {
        view_matrix: view,
        view_projection_matrix: proj * view,
        view_position: pos
    }
}

fn update_camera_orbit(
    app: &AppRes,
    pmfx: &mut PmfxRes,
    camera: &mut Camera,
    position: &mut Position,
    view_proj: &mut ViewProjectionMatrix,
    name: &String
) {
    let drag = app.get_mouse_pos_delta();
    let wheel = app.get_mouse_wheel();
    let buttons = app.get_mouse_buttons();
    let drag = vec2f(drag.x as f32, drag.y as f32);

    let (enable_keyboard, enable_mouse) = app.get_input_enabled();

    // speed modifier
    let boost_speed = 2.0;
    let mut scroll_speed = 100.0;
    let control_speed = 0.1;

    if enable_keyboard {
        // modifiers
        if app.is_sys_key_down(os::SysKey::Shift) {
            // speed boost
            scroll_speed *= boost_speed;
        }
        else if app.is_sys_key_down(os::SysKey::Ctrl) {
            // fine control
            scroll_speed *= control_speed;
        }
    }

    if enable_mouse {
        if app.is_sys_key_down(os::SysKey::Shift) && enable_keyboard && buttons[os::MouseButton::Left as usize] {
            let right = view_proj.get_row(0).xyz();
            let up = view_proj.get_row(1).xyz();
            camera.focus += up * -drag.y;
            camera.focus += right * -drag.x;
        }
        else {
            if buttons[os::MouseButton::Left as usize] {
                camera.rot -= Vec3f::from((drag.yx(), 0.0));
            }
            camera.zoom += wheel * scroll_speed;
            camera.zoom = max(camera.zoom, 1.0);
        }
    }

    // generate proj matrix
    let aspect = pmfx.get_window_aspect("main_dock");

    let constants = camera_constants_from_orbit(&camera.rot, &camera.focus, camera.zoom, aspect, 60.0);
    view_proj.0 = constants.view_projection_matrix;
    position.0 = constants.view_position.xyz();

    // update camera in pmfx
    pmfx.update_camera_constants(&name, &constants);
}

fn update_camera_fly(
    app: &AppRes,
    time: &TimeRes,
    pmfx: &mut PmfxRes,
    camera: &mut Camera,
    position: &mut Position,
    view_proj: &mut ViewProjectionMatrix,
    name: &String
) {
    let (enable_keyboard, enable_mouse) = app.get_input_enabled();

    let mut cam_move_delta = Vec3f::zero();

    let speed = 240.0;
    let boost_speed = 2.0;
    let control_speed = 0.25;

    if enable_keyboard {
        // get keyboard position movement
        let keys = app.get_keys_down();
        if keys['A' as usize] {
            cam_move_delta.x -= speed;
        }
        if keys['D' as usize] {
            cam_move_delta.x += speed;
        }
        if keys['Q' as usize] {
            cam_move_delta.y -= speed;
        }
        if keys['E' as usize] {
            cam_move_delta.y += speed;
        }
        if keys['W' as usize] {
            cam_move_delta.z -= speed;
        }
        if keys['S' as usize] {
            cam_move_delta.z += speed;
        }

        // modifiers
        if app.is_sys_key_down(os::SysKey::Shift) {
            // speed boost
            cam_move_delta *= boost_speed;
        }
        else if app.is_sys_key_down(os::SysKey::Ctrl) {
            // fine control
            cam_move_delta *= control_speed;
        }

        // scale by delta time, consistencies, but we ignore time scaling
        cam_move_delta *= time.raw_delta;
    }

    // get mouse rotation
    if enable_mouse {
        if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
            let mouse_delta = app.get_mouse_pos_delta();
            camera.rot.x -= mouse_delta.y as f32;
            camera.rot.y -= mouse_delta.x as f32;
        }
    }

    // construct rotation matrix
    let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(camera.rot.x));
    let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(camera.rot.y));
    let mat_rot = mat_rot_y * mat_rot_x;

    // move relative to facing directions
    position.0 += mat_rot * cam_move_delta;

    // generate proj matrix
    let aspect = pmfx.get_window_aspect("main_dock");
    
    // assign view proj
    let constants = camera_constants_from_fly(&position, &camera.rot, aspect, 60.0);
    view_proj.0 = constants.view_projection_matrix;

    // update camera in pmfx
    pmfx.update_camera_constants(&name, &constants);
}

fn update_cameras(
    app: Res<AppRes>,
    time: Res<TimeRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut query: Query<(&Name, &mut Position, &mut Camera, &mut ViewProjectionMatrix)>) {
    
    pmfx.get_world_buffers_mut().camera.clear();
    
    for (name, mut position, mut camera, mut view_proj) in &mut query {
        match camera.camera_type {
            CameraType::Fly => {
                update_camera_fly(&app, &time, &mut pmfx, &mut camera, &mut position, &mut view_proj, name);
            },
            CameraType::Orbit => {
                update_camera_orbit(&app, &mut pmfx, &mut camera, &mut position, &mut view_proj, name);
            }
            _ => continue
        }

        let constants = pmfx.get_camera_constants(name).unwrap();
        let camera_data = pmfx::CameraData {
            view_projection_matrix: constants.view_projection_matrix,
            view_position: constants.view_position,
            planes: constants.view_projection_matrix.get_frustum_planes()
        };

        if pmfx.get_world_buffers_mut().camera.capacity() > 0 {
            pmfx.get_world_buffers_mut().camera.push(&camera_data);
        }
    }
}

impl BevyPlugin {
    /// Finds get_system calls inside ecs compatible plugins, call the function `get_system_<lib_name>` to disambiguate
    fn get_system_function(&self, name: &str, view_name: &str, client: &PlatformClient) -> Option<SystemConfig> {
        // find by export name
        for (_, lib) in &client.libs {
            unsafe {
                let exported_function_name = format!("export_{}", name);
                if view_name.is_empty() {
                    // function with () no args
                    let hook = lib.get_symbol::<unsafe extern fn() -> SystemConfig>(exported_function_name.as_bytes());
                    if let Ok(hook_fn) = hook {
                        return Some(hook_fn());
                    }
                }
                else {
                    // function with pass name args
                    let hook = lib.get_symbol::<unsafe extern fn(String) -> SystemConfig>(exported_function_name.as_bytes());
                    if let Ok(hook_fn) = hook {
                        return Some(hook_fn(view_name.to_string()));
                    }
                }
            }
        }
        
        // deprecated using get_system function
        for (lib_name, lib) in &client.libs {
            unsafe {
                let function_name = format!("get_system_{}", lib_name).to_string();
                let hook = lib.get_symbol::<unsafe extern fn(String, String) -> Option<SystemConfig>>(function_name.as_bytes());
                if let Ok(hook_fn) = hook {
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
            render_graph: "mesh_debug",
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
        println!("  failed to find demo_schedule_info {}", self.session_info.active_demo);
        None
    }

    /// If we change demo or need to rebuild render graphs we need to invoke this, code changes will already invoke setup
    fn resetup(&mut self, mut client: PlatformClient) -> PlatformClient {
        // serialize
        client.serialise_plugin_data("ecs", &self.session_info);
        // unload / setup
        client.swap_chain.wait_for_last_frame();
        client = self.unload(client);
        client.pmfx.unload_views();
        self.setup(client)
    }

    /// Custom function to handle custome data change events which can trigger resetup
    fn check_for_changes(&mut self, client: PlatformClient) -> PlatformClient {
        // render graph itself has chaned
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

        let graph = self.schedule_info.render_graph;

        if self.errors.contains_key(graph) {
            client.imgui.colour_text(
                &format!("Render Graph: {}: {}.", "error", graph), 
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

    fn setup_camera(&mut self) -> (Camera, Mat4f, Position) {
        // use a default demo camera, if we have no main camera (mainly for test runners)
        if let Some(default_cameras) = &self.session_info.default_cameras {
            if default_cameras.contains_key(&self.session_info.active_demo) {
                if self.session_info.main_camera.is_none() {
                    self.session_info.main_camera = Some(default_cameras[&self.session_info.active_demo]);
                }
            }
        }
        let main_camera = self.session_info.main_camera.unwrap_or_default();
        let pos = Position(Vec3f::from(main_camera.pos));
        let focus = Vec3f::from(main_camera.focus);
        let rot = Vec3f::from(main_camera.rot);
        let zoom = main_camera.zoom;
        let constants = match main_camera.camera_type {
            CameraType::Orbit => camera_constants_from_orbit(&rot, &focus, zoom, main_camera.aspect, main_camera.fov),
            _ => camera_constants_from_fly(&pos, &rot, main_camera.aspect, main_camera.fov)
        };
        (
            Camera {
                rot: rot,
                focus: focus,
                zoom: zoom,
                camera_type: main_camera.camera_type
            },
            constants.view_projection_matrix,
            pos
        )
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
        let graph = self.schedule_info.render_graph;
        let graph_result = client.pmfx.create_render_graph(&mut client.device, &graph);

        let render_functions = if let Err(error) = graph_result {
            // if render graph fails to build, use the default and log errors for the user
            self.schedule_info = ScheduleInfo::default();
            let ext_msg = format!("{} (Check GPU Validation Messages For More Info)", error.msg);
            println!("{}", error.msg);
            self.errors.entry(graph.to_string()).or_insert(Vec::new()).push(ext_msg);
            self.schedule_info.render_graph = graph;
            Vec::new()
        }
        else {
            client.pmfx.get_render_graph_function_info(&graph)
        };
        let info = &self.schedule_info;

        // core update
        self.schedule.add_system(update_cameras.in_base_set(CoreSystemSets::Update));
        self.schedule.add_system(update_main_camera_config.in_base_set(CoreSystemSets::Update));

        // core batch functions do syncronised work to prepare buffers / matrices for drawing
        self.schedule.add_system(update_world_matrices.in_base_set(SystemSets::Batch));

        // hook in setup funcs
        for func_name in &info.setup {
            if let Some(func) = self.get_system_function(func_name, "", &client) {
                self.setup_schedule.add_system(func);
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }

        // hook in updates funcs
        for func_name in &info.update {
            if let Some(func) = self.get_system_function(func_name, "", &client) {
                self.schedule.add_system(func);
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }

        // hook in render functions
        for (func_name, view_name) in &render_functions {
            if let Some(func) = self.get_system_function(func_name, view_name, &client) {
                self.schedule.add_system(func);
            }
            else {
                self.errors.entry(func_name.to_string()).or_insert(Vec::new());
            }
        }
        self.render_graph_hash = client.pmfx.get_render_graph_hash(&info.render_graph);

        // process sets in fixed order
        self.schedule.configure_sets((
            CoreSystemSets::Update,
            SystemSets::Update,
            CoreSystemSets::Batch,
            SystemSets::Batch,
            CoreSystemSets::Render,
            SystemSets::Render
        ).chain());

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
        self.world.insert_resource(TimeRes(client.time));

        // run setup if requested, we did it here so hotline resources are inserted into World
        if self.run_setup {
            let (cam, vp, pos) = self.setup_camera();           
            self.world.spawn((
                ViewProjectionMatrix(vp),
                pos,
                cam,
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
        client.time = self.world.remove_resource::<TimeRes>().unwrap().0;
        self.session_info = self.world.remove_resource::<SessionInfo>().unwrap();

        // write back session info which will be serialised to disk and reloaded between sessions
        client.serialise_plugin_data("ecs", &self.session_info);
        client
    }

    fn unload(&mut self, client: PlatformClient) -> PlatformClient {
        // drop everything while its safe
        self.setup_schedule = Schedule::default();
        self.schedule = Schedule::default();
        self.world = World::default();
        client
    }

    fn ui(&mut self, mut client: PlatformClient) -> PlatformClient {
        let mut open = true;
        let mut resetup = false;
        if client.imgui.begin("ecs", &mut open, imgui::WindowFlags::NONE) {
            // refresh button
            if client.imgui.button_size(font_awesome::strs::SYNC, 32.0, 0.0) {
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

            // camera type select
            if client.imgui.button_size(font_awesome::strs::CAMERA, 32.0, 0.0) {
                // save default
                if self.session_info.default_cameras.is_none() {
                    self.session_info.default_cameras = Some(HashMap::new());
                }
                let default_cam_map = self.session_info.default_cameras.as_mut().unwrap();
                let entry = default_cam_map.entry(self.session_info.active_demo.to_string()).or_default();
                *entry = self.session_info.main_camera.unwrap();
            }
            client.imgui.same_line();

            let mut main_camera_query = self.world.query::<(&mut Camera, &MainCamera)>();
            for (mut camera, _) in &mut main_camera_query.iter_mut(&mut self.world) {
                let camera_types = vec![
                    "Fly".to_string(),
                    "Orbit".to_string()
                ];
                let selected = format!("{:?}", camera.camera_type);
                let (_, selected) = client.imgui.combo_list("Camera", &camera_types, &selected);
                camera.camera_type = match selected.as_str() {
                    "Fly" => CameraType::Fly,
                    "Orbit" => CameraType::Orbit,
                    _ => CameraType::Fly
                };
            }

            // -/+ to toggle through demos, ignore test missing and test failing demos
            let wrap_len = demo_list.len();
            
            let cur_demo_index = demo_list.iter().position(|d| *d == self.session_info.active_demo);
            if let Some(index) = cur_demo_index {
                let keys = client.app.get_keys_pressed();
                let toggle = if keys[189] {
                    index.wrapping_sub(1) % wrap_len
                }
                else if keys[187] {
                     (index + 1) % wrap_len
                }
                else {
                    index
                };
                if toggle != index {
                    self.session_info.active_demo = demo_list[toggle].to_string();
                    resetup = true;
                }
            }

            // debug draw options
            client.imgui.button_size(font_awesome::strs::EYE, 32.0, 0.0);

            // grid
            client.imgui.same_line();
            let mut dd = self.session_info.debug_draw_flags.contains(DebugDrawFlags::GRID);
            if client.imgui.checkbox("Grid", &mut dd) {
                if dd {
                    self.session_info.debug_draw_flags |= DebugDrawFlags::GRID;
                }
                else {
                    self.session_info.debug_draw_flags &= !DebugDrawFlags::GRID;
                }
            }

            // aabb
            client.imgui.same_line();
            let mut dd = self.session_info.debug_draw_flags.contains(DebugDrawFlags::AABB);
            if client.imgui.checkbox("AABB", &mut dd) {
                if dd {
                    self.session_info.debug_draw_flags |= DebugDrawFlags::AABB;
                }
                else {
                    self.session_info.debug_draw_flags &= !DebugDrawFlags::AABB;
                }
            }

            // obb
            client.imgui.same_line();
            let mut dd = self.session_info.debug_draw_flags.contains(DebugDrawFlags::OBB);
            if client.imgui.checkbox("OBB", &mut dd) {
                if dd {
                    self.session_info.debug_draw_flags |= DebugDrawFlags::OBB;
                }
                else {
                    self.session_info.debug_draw_flags &= !DebugDrawFlags::OBB;
                }
            }

            client = self.schedule_ui(client);
        }

        // preform any re-setup actions
        if resetup {
            // set camera to the default position for the selected demo
            if let Some(default_cameras) = &self.session_info.default_cameras {
                if default_cameras.contains_key(&self.session_info.active_demo) {
                    self.session_info.main_camera = Some(default_cameras[&self.session_info.active_demo]);
                }
            }
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