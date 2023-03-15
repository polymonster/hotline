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
    _device: &Res<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let mesh_debug = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

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

    Ok(())
}

/// Renders all scene meshes with a pipeline component, binding a new pipeline each draw
#[no_mangle]
pub fn render_meshes_pipeline(
    _device: &Res<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &Pipeline)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

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

    Ok(())
}

/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_vertex_buffer_instanced(
    _device: &Res<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&draw::InstanceBuffer, &MeshComponent, &Pipeline)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16 * 3, 0, gfx::as_u8_slice(camera));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.set_vertex_buffer(&instance_batch.buffer, 1);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene instance batches with cbuffer instance buffer
#[no_mangle]
pub fn render_meshes_cbuffer_instanced(
    _device: &Res<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&draw::InstanceBuffer, &MeshComponent, &Pipeline)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16 * 3, 0, gfx::as_u8_slice(camera));

        view.cmd_buf.set_render_heap(1, instance_batch.heap.as_ref().unwrap(), 0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);

        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// Register demo names
#[no_mangle]
pub fn get_demos_ecs_demos() -> Vec<String> {
    demos![
        "primitives",

        // draw tests
        "draw_indexed",
        "draw_indexed_push_constants",
        "draw_indexed_vertex_buffer_instanced",
        "draw_indexed_cbuffer_instanced",

        // render state tests
        "test_raster_states",
        "test_blend_states",

        // basic tests
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

        // draw tests
        "setup_draw_indexed" => system_func![setup_draw_indexed],
        "setup_draw_indexed_push_constants" => system_func![setup_draw_indexed_push_constants],
        "setup_draw_indexed_vertex_buffer_instanced" => system_func![setup_draw_indexed_vertex_buffer_instanced],
        "setup_draw_indexed_cbuffer_instanced" => system_func![setup_draw_indexed_cbuffer_instanced],

        // render state tests
        "setup_raster_test" => system_func![setup_raster_test],
        "setup_blend_test" => system_func![setup_blend_test],
        
        // updates
        "rotate_meshes" => system_func![rotate_meshes],
        "batch_world_matrix_instances" => system_func![draw::batch_world_matrix_instances],

        // render functions
        "render_meshes" => render_func![render_meshes, view_name],
        "render_meshes_pipeline" => render_func_query![
            render_meshes_pipeline, view_name, 
            Query<(&WorldMatrix, &MeshComponent, &Pipeline)>
        ],
        "render_meshes_vertex_buffer_instanced" => render_func_query![
            render_meshes_vertex_buffer_instanced, view_name, 
            Query<(&draw::InstanceBuffer, &MeshComponent, &Pipeline)>
        ],
        "render_meshes_cbuffer_instanced" => render_func_query![
            render_meshes_cbuffer_instanced, view_name, 
            Query<(&draw::InstanceBuffer, &MeshComponent, &Pipeline)>
        ],
        
        // basic tests
        "render_missing_camera" => render_func![render_missing_camera, view_name],
        "render_missing_pipeline" => render_func![render_missing_pipeline, view_name],
        _ => std::hint::black_box(None)
    }
}