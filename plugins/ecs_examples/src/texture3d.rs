// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Texture3D
///

use crate::prelude::*;

/// Test 3d texture loading and rendering using a pre-built sdf texture
#[no_mangle]
pub fn texture3d(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_texture3d"
        ],
        render_graph: "texture3d_test",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_texture3d(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);

    let volume_info = image::load_from_file(&hotline_rs::get_data_path("textures/sdf_shadow.dds")).unwrap();
    let volume = device.0.create_texture_with_heaps(
        &volume_info.info,
        gfx::TextureHeapInfo {
            shader: Some(&mut pmfx.shader_heap),
            ..Default::default()
        },
        Some(volume_info.data.as_slice())
    ).unwrap();

    let dim = 50.0;

    commands.spawn((
        MeshComponent(cube_mesh),
        Position(vec3f(0.0, dim * 0.5, 0.0)),
        Rotation(Quatf::identity()),
        Scale(splat3f(dim)),
        WorldMatrix(Mat34f::identity()),
        TextureInstance(volume.get_srv_index().unwrap() as u32)
    ));

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(volume)
    );

    Ok(())
}

/// Renders a texture3d test from a loaded (pre-generated signed distance field), the shader ray marches the volume
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_texture3d(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    cmd_buf.set_render_pipeline(pipeline);
    cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    cmd_buf.push_render_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    for (world_matrix, mesh, tex) in &mesh_draw_query {
        cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[tex.0, 0, 0, 0]));
        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}