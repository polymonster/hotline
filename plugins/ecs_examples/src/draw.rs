// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Draw
/// 

use crate::prelude::*;

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_draw_identity"
    }
}

/// Adds a single triangle mesh
#[no_mangle]
#[export_update_fn]
pub fn setup_draw(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) -> Result<(), hotline_rs::Error> {

    let pos = Mat34f::identity();
    let scale = Mat34f::from_scale(splat3f(100.0));

    let tri_mesh = hotline_rs::primitives::create_triangle_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(tri_mesh),
        WorldMatrix(pos * scale)
    ));

    Ok(())
}

/// Renders meshes with a draw call (non-indexed) (single triangle)
#[no_mangle]
#[export_render_fn]
pub fn draw_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    for (_, mesh) in &mesh_draw_query {
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_instanced(3, 1, 0, 0);
    }

    Ok(())
}