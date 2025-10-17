// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

/// 
/// Bindless Texture
/// 

use crate::prelude::*; 

#[no_mangle]
pub fn bindless_texture(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_bindless_texture"
        ],
        render_graph: "mesh_bindless_texture",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_bindless_texture(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let sphere = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_albedo.dds"), 
            Some(&mut pmfx.shader_heap)
        )?),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_normal.dds"), 
            Some(&mut pmfx.shader_heap)
        )?),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/bluechecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        )?),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/redchecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        )?)
    ];

    // square number of rows and columns
    let rc = sqrt(textures.len() as f32);
    let irc = (rc + 0.5) as usize; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            let ti = textures[y * irc + x].get_srv_index().unwrap();
            commands.spawn((
                MeshComponent(sphere.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(ti as u32),
            ));
        }
    }
    
    // spawn entities to keep hold of textures
    for tex in textures {
        commands.spawn(
            tex
        );
    }

    // dbeug prims uvs
    let debug_uvs = false;
    if debug_uvs {
        let meshes = vec![
            hotline_rs::primitives::create_plane_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_mesh(&mut device.0),
            hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
            hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_billboard_mesh(&mut device.0),
            hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
            hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
            hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
            hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5),
            hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        ];
    
        let uv_debug_tex = TextureComponent(image::load_texture_from_file(
            &mut device, 
            &hotline_rs::get_src_data_path("textures/blend_test_fg.png"),
            Some(&mut pmfx.shader_heap)
        )?);
    
        let rc = ceil(sqrt(meshes.len() as f32));
        let irc = (rc + 0.5) as usize; 
    
        let size = 10.0;
        let half_size = size * 0.5;    
        let step = size * half_size;
        let half_extent = (rc-1.0) * step * 0.5;
        let start_pos = vec3f(-half_extent, size, -half_extent);
    
        let mut i = 0;
        for y in 0..irc {
            for x in 0..irc {
                if i < meshes.len() {
                    let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                    commands.spawn((
                        MeshComponent(meshes[i].clone()),
                        Position(iter_pos),
                        Rotation(Quatf::identity()),
                        Scale(splat3f(size)),
                        WorldMatrix(Mat34f::identity()),
                        TextureInstance(uv_debug_tex.get_srv_index().unwrap() as u32),
                    ));
                    i += 1;
                }
            }
        }

        commands.spawn(
            uv_debug_tex
        );
    }

    Ok(())
}

/// Renders all scene meshes with a material instance component, using push constants to push texture ids
#[no_mangle]
#[export_render_fn]
pub fn draw_meshes_bindless_texture(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    cmd_buf.set_render_pipeline(pipeline);
    cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    for (world_matrix, mesh, texture) in &mesh_draw_query {
        cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        cmd_buf.push_render_constants(1, 1, 16, gfx::as_u8_slice(&texture.0));

        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}