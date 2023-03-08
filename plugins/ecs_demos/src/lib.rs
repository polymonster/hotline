// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

mod primitives;
mod test;
mod dev;
mod draw;

use crate::draw::*;
use crate::primitives::*;
use crate::test::*;

#[no_mangle]
fn rotate_meshes(
    time: Res<TimeRes>,
    mut query: Query<&mut Rotation>) {
    for mut rotation in &mut query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
    }
}

#[no_mangle]
pub fn render_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let mesh_debug = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&mesh_debug);
    view.cmd_buf.push_constants(0, 16 * 3, 0, gfx::as_u8_slice(camera));

    // billboard
    // let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    // let bbmat = world_matrix.0 * Mat4f::from(inv_rot);

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();

    Ok(())
}

/// Renders all scene meshes with a pipeline component, binding a new pipeline each draw
#[no_mangle]
pub fn render_meshes_pipeline(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &Pipeline)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    for (world_matrix, mesh, pipeline) in &mesh_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16 * 3, 0, gfx::as_u8_slice(camera));

        view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();

    Ok(())
}

/// Register demo names
#[no_mangle]
pub fn get_demos_ecs_demos() -> Vec<String> {
    demos![
        "primitives",
        "draw_indexed",
        "draw_indexed_push_constants",

        // tests
        "test_raster_states",
        "test_missing_demo",
        "test_missing_systems",
        "test_missing_render_graph",
        "test_missing_view",
        "test_missing_pipeline",
        "test_failing_pipeline",
        "test_missing_camera"
    ]
}

/// Register plugin system functions
#[no_mangle]
pub fn get_system_ecs_demos(name: String, view_name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        // setup functions
        "setup_primitives" => system_func![setup_primitives],
        "setup_draw_indexed" => system_func![setup_draw_indexed],
        "setup_draw_indexed_push_constants" => system_func![setup_draw_indexed_push_constants],
        "setup_raster_test" => system_func![setup_raster_test],

        // updates
        "rotate_meshes" => system_func![rotate_meshes],

        // render functions
        "render_meshes" => render_func![render_meshes, view_name],
        "render_meshes_pipeline" => render_func_query![render_meshes_pipeline, view_name, Query<(&WorldMatrix, &MeshComponent, &Pipeline)>],

        // test functions
        "render_missing_camera" => render_func![render_missing_camera, view_name],
        "render_missing_pipeline" => render_func![render_missing_pipeline, view_name],
        _ => std::hint::black_box(None)
    }
}