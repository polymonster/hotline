// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Draw Indexed
///

use crate::prelude::*; 

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw_indexed(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed"
        ],
        render_graph: "mesh_draw_indexed_identity",
        ..Default::default()
    }
}

/// Set's up a single cube mesh. The draw all is made with `draw_meshes` in `draw.rs` 
#[no_mangle]
#[export_update_fn]
pub fn setup_draw_indexed(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let pos = Mat34f::from_translation(Vec3f::unit_y() * 10.0);
    let scale = Mat34f::from_scale(splat3f(10.0));

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh),
        WorldMatrix(pos * scale)
    ));

    Ok(())
}

/// Renders meshes with an indexed draw call
#[no_mangle]
#[export_render_fn]
pub fn draw_meshes_indexed(
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
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}