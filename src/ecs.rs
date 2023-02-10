// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::os::App;
use crate::os::Window;

use crate::pmfx;
use crate::imdraw;
use crate::imgui;

use crate::gfx_platform;
use crate::os_platform;

use crate::gfx;
use crate::os;

use gfx::RenderPass;
use gfx::CmdBuf;

use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

//
// Stages
//

#[derive(StageLabel)]
pub struct StageStartup;

#[derive(StageLabel)]
pub struct StageUpdate;

#[derive(StageLabel)]
pub struct StageRender;

//
// Resources
//

#[derive(Resource)]
pub struct DeviceRes(pub gfx_platform::Device);

#[derive(Resource)]
pub struct AppRes(pub os_platform::App);

#[derive(Resource)]
pub struct MainWindowRes(pub os_platform::Window);

#[derive(Resource)]
pub struct PmfxRes(pub pmfx::Pmfx<gfx_platform::Device>);

#[derive(Resource)]
pub struct ImDrawRes(pub imdraw::ImDraw<gfx_platform::Device>);

#[derive(Resource)]
pub struct ImGuiRes(pub imgui::ImGui::<gfx_platform::Device, os_platform::App>);

//
// Components
//

#[derive(Component)]
pub struct Position(pub Vec3f);

#[derive(Component)]
pub struct Velocity(pub Vec3f);

#[derive(Component)]
pub struct WorldMatrix(pub Mat4f);

#[derive(Component)]
pub struct Rotation(pub Vec3f);

#[derive(Component)]
pub struct ViewProjectionMatrix(pub Mat4f);

#[derive(Component)]
pub struct Camera;

#[derive(Component)]
pub struct MeshComponent(pub pmfx::Mesh<gfx_platform::Device>);

//
// Core Systems
//

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

        println!("{}", view_proj.0);

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

#[macro_export]
macro_rules! system_func {
    ($func:expr) => {
        Some($func.into_descriptor())
    }
}

#[macro_export]
macro_rules! view_func {
    ($func:expr, $view:literal) => {
        Some(view_func_closure![$func, $view].into_descriptor())
    }
}

#[macro_export]
macro_rules! view_func_closure {
    ($func:expr, $view:literal) => {
        move |
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>,
            qmesh: Query::<(&WorldMatrix, &MeshComponent)>| {
                $func(
                    pmfx,
                    "render_world_view".to_string(),
                    qvp,
                    qmesh
                );
        }
    }
}

#[no_mangle]
pub fn get_system_function(name: &str) -> Option<SystemDescriptor> {
    match name {
        "update_cameras" => system_func![update_cameras],
        "render_grid" => system_func![render_grid],
        "render_world_view" => view_func![render_world_view, "render_world_view"],
        _ => None
    }
}
