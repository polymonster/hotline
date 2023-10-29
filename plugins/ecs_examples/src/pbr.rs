// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Cubemap
///

use crate::prelude::*;

#[derive(Resource)]
pub struct State {
    lut_srv : u32
}

/// Basic pbr example
#[no_mangle]
pub fn pbr(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/util").as_str()).unwrap();
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_pbr"
        ],
        render_graph: "mesh_pbr",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_pbr(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    // square number of rows and columns
    let rc = 5.0;
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, -half_extent, 0.0);

    let cubemap_filepath = hotline_rs::get_data_path("textures/cubemap.dds");
    let cubemap = image::load_texture_from_file(&mut device.0, &cubemap_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    let brdf_lut_filepath = hotline_rs::get_data_path("textures/luts/ibl_brdf_lut.dds");
    let lut = image::load_texture_from_file(&mut device.0, &brdf_lut_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    let lut_srv = lut.get_srv_index().unwrap() as u32;
    let cubemap_srv = cubemap.get_srv_index().unwrap() as u32;

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, y as f32 * step, 0.0);
            commands.spawn((
                MeshComponent(sphere_mesh.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(cubemap_srv),
            ));
        }
    }
    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(cubemap)
    );

    commands.spawn(
        TextureComponent(lut)
    );

    // ?
    commands.insert_resource(State{
        lut_srv
    });

    Ok(())
}

/// Renders all scene meshes with a irradiance and specular cubemaps bound to perform image based pbr lighting
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_pbr(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    state: &Res<State>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    view.cmd_buf.push_render_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    let mut i = 0;
    let min_roughness = 1.0 / 5.0;
    for (world_matrix, mesh, cubemap) in &mesh_draw_query {

        let roughness = min_roughness + ((i % 5).as_f32() / 6.0);
        let metalness = floor((i / 5).as_f32()) / 5.0;

        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 4, 12, gfx::as_u8_slice(&[roughness, metalness, 0.0, 0.0]));
        view.cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[cubemap.0, state.lut_srv, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);

        i += 1;
    }

    Ok(())
}