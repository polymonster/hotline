// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Texture2D Array
///

use crate::prelude::*;

/// Test texture2d_array loading, loads a dds texture2d_array generated from an image sequence
#[no_mangle]
pub fn texture2d_array(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_texture2d_array"
        ],
        update: systems![
            "animate_textures"
        ],
        render_graph: "texture2d_array_test"
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_texture2d_array(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let billboard_mesh = hotline_rs::primitives::create_billboard_mesh(&mut device.0);

    let texture_array_filepath = hotline_rs::get_data_path("textures/bear.dds");

    let texture_array_info = image::load_from_file(&texture_array_filepath)?;
    let texture_array = device.0.create_texture_with_heaps(
        &texture_array_info.info,
        gfx::TextureHeapInfo {
            shader: Some(&mut pmfx.shader_heap),
            ..Default::default()
        },
        Some(texture_array_info.data.as_slice())
    )?;
    let aspect = (texture_array_info.info.width / texture_array_info.info.height) as f32;
    let size = vec2f(20.0 * aspect, 20.0);

    let num_instances = 64;

    let mut rng = rand::thread_rng();
    let dist = rand::distributions::Uniform::from(-200..200);

    // randomly spawn some cylindrical billboards
    for _ in 0..num_instances {
        let mut pos = vec3f(
            dist.sample(&mut rng) as f32, 
            dist.sample(&mut rng) as f32, 
            dist.sample(&mut rng) as f32
        ) * vec3f(1.0, 0.0, 1.0);
        pos.y = size.y * 0.7;
        commands.spawn((
            MeshComponent(billboard_mesh.clone()),
            Position(pos),
            Rotation(Quatf::identity()),
            Scale(vec3f(size.x, size.y, size.x)),
            WorldMatrix(Mat34f::identity()),
            Billboard,
            CylindricalBillboard,
            TextureInstance(texture_array.get_srv_index().unwrap() as u32),
            TimeComponent(0.0),
            AnimatedTexture {
                frame: floor(rng.gen::<f32>() * texture_array_info.info.array_layers as f32) as u32,
                frame_count: texture_array_info.info.array_layers
            }
        ));
    }

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(texture_array)
    );

    Ok(())
}

/// Renders a texture2d test passing the texture index and frame index to the shader for sampling along with a world matrix.
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_texture2d_array(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance, &AnimatedTexture), With<CylindricalBillboard>>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    // spherical billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    let cyl_rot = Mat3f::new(
        inv_rot[0], 0.0, inv_rot[2],
        0.0, 1.0, 0.0,
        inv_rot[6], 0.0, inv_rot[8],
    );

    for (world_matrix, mesh, texture, animated_texture) in &mesh_query {
        let bbmat = world_matrix.0 * Mat4f::from(cyl_rot);
        view.cmd_buf.push_render_constants(1, 12, 0, &bbmat);
        view.cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[texture.0, animated_texture.frame, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn animate_textures(
    time: Res<TimeRes>,
    mut animated_texture_query: Query<(&mut AnimatedTexture, &mut TimeComponent)>) -> Result<(), hotline_rs::Error>{
    let frame_length = 1.0 / 24.0;
    for (mut animated_texture, mut timer) in &mut animated_texture_query {
        timer.0 += time.0.delta;
        if timer.0 > frame_length {
            timer.0 = 0.0;
            animated_texture.frame = (animated_texture.frame + 1) % animated_texture.frame_count;
        }
    }

    Ok(())
}