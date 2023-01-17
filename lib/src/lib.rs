use hotline_rs::*;
use ecs::*;
use primitives;

use gfx::CmdBuf;

use os::App;
use os::Window;

use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;

use std::collections::HashMap;

#[no_mangle]
pub fn setup_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    commands.spawn((
        Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
        Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ViewProjectionMatrix { 0: Mat4f::identity()},
        Camera,
    ));

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    let dim = 1;
    let dim2 = dim / 2;

    for y in 0..dim {    
        for x in 0..dim {    
            commands.spawn((
                Position { 0: Vec3f::zero() },
                Velocity { 0: Vec3f::one() },
                MeshComponent {0: cube_mesh.clone()},
                WorldMatrix { 0: Mat4f::from_translation(
                    vec3f(
                        x as f32 * 2.5 - dim2 as f32 * 2.5, 
                        0.0, 
                        y as f32 * 2.5 - 2.5 * dim as f32)) * 
                        Mat4::from_scale(vec3f(1.0, f32::abs(f32::sin((x + y) as f32 / 20.0 as f32)) * 20.0, 1.0)) }
            ));
        }
    }
}

#[no_mangle]
pub fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

#[no_mangle]
pub fn mat_movement(mut query: Query<&mut WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.1, 0.0));
    }
}

#[no_mangle]
pub fn mat_movement2(mut query: Query<&mut WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.001, 0.001));
    }
}

#[no_mangle]
pub fn mat_movement3(mut query: Query<&mut WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(1.0, 0.01, 0.01));
    }
}

#[no_mangle]
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
        let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 0.1, 10000.0);

        // construct rotation matrix
        let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rotation.0.x));
        let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rotation.0.y));
        let mat_rot = mat_rot_y * mat_rot_x;

        // build view / proj matrix
        let translate = Mat4f::from_translation(position.0);
        let view = translate * mat_rot;
        let view = view.inverse();
       
        // assign view proj
        view_proj.0 = proj * view;
    }
}

#[no_mangle]
pub fn render_grid(
    mut device: ResMut<DeviceRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: Res<PmfxRes>,
    mut query: Query<&ViewProjectionMatrix> ) {

    let arc_view = pmfx.0.get_view("render_grid").unwrap();
    let mut view = arc_view.lock().unwrap();
    let bb = view.cmd_buf.get_backbuffer_index();

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

        view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_3d").unwrap());
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);

        imdraw.draw_3d(&mut view.cmd_buf, bb as usize);

        view.cmd_buf.end_render_pass();
    }
}

#[no_mangle]
pub fn render_world_view(
    pmfx: Res<PmfxRes>,
    view_name: String,
    view_proj_query: Query<&ViewProjectionMatrix>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) {
    
    // unpack
    let pmfx = &pmfx.0;

    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);
    view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_mesh").unwrap());

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
pub fn build_schedule(startup_schedule: &mut Schedule, schedule: &mut Schedule) {
    let mut fn_map = HashMap::new();
    fn_map.insert("update_cameras", update_cameras.into_descriptor());
    fn_map.insert("mat_movement", mat_movement2.into_descriptor());
    fn_map.insert("setup_test", setup_test.into_descriptor());

    let starup_funcs = vec!["setup_test"];
    let update_funcs = vec!["update_cameras", "mat_movement"];
    let render_funcs = vec!["render_grid", "render_world_view"];

    // add startup funcs by name
    let mut startup_stage = SystemStage::parallel();
    for func_name in starup_funcs {
        startup_stage = startup_stage.with_system(fn_map.remove(func_name).unwrap());
    }
    
    startup_schedule.add_stage(StageStartup, startup_stage);

    // add update funcs by name
    let mut update_stage = SystemStage::parallel();
    for func_name in update_funcs {
        update_stage = update_stage.with_system(fn_map.remove(func_name).unwrap());
    }

    schedule.add_stage(StageUpdate, update_stage);

    // create a "constructor" closure, which can initialize
    // our data and move it into a closure that bevy can run as a system
    let view_constructor = |view: String| {
        move |
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>,
            qmesh: Query::<(&WorldMatrix, &MeshComponent)>| {
                render_world_view(
                    pmfx,
                    view.to_string(),
                    qvp,
                    qmesh
                );
        }
    };

    fn_map.insert("render_world_view", view_constructor("render_world_view".to_string()).into_descriptor());
    fn_map.insert("render_grid", render_grid.into_descriptor());

    let mut render_stage = SystemStage::parallel();
    for func_name in render_funcs {
        render_stage = render_stage.with_system(fn_map.remove(func_name).unwrap());
    }

    schedule.add_stage(StageRender, render_stage);
}