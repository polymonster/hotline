// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Cubemap
///

use crate::prelude::*;

/// Test cubemap loading (including mip-maps) and rendering
#[no_mangle]
pub fn cubemap(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_cubemap"
        ],
        render_graph: "cubemap_test",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_cubemap(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    // square number of rows and columns
    let rc = 3.0;
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    let cubemap_filepath = hotline_rs::get_data_path("textures/cubemap.dds");
    let cubemap = image::load_texture_from_file(&mut device.0, &cubemap_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere_mesh.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(cubemap.get_srv_index().unwrap() as u32)
            ));
        }
    }

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(cubemap)
    );

    Ok(())
}

/// Renders all scene meshes with a cubemap applied and samples the separate mip levels in the shader per entity
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_cubemap(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    cmd_buf.set_render_pipeline(pipeline);
    cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    let mut mip = 0;
    for (world_matrix, mesh, cubemap) in &mesh_draw_query {
        cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[cubemap.0, mip, 0, 0]));

        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);

        mip += 1;
    }

    Ok(())
}