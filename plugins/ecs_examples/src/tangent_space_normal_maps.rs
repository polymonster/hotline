// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

/// 
/// Tangent Space Normal Maps
/// 

use crate::prelude::*; 

/// Init function for tangent space normal maps to debug tangents 
#[no_mangle]
pub fn tangent_space_normal_maps(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_geometry_primitives",
            "setup_tangent_space_normal_maps"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_debug_tangent_space"
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_tangent_space_normal_maps(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {
    
    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/pbr/antique-grate1/antique-grate1_normal.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap())
    ];

    for tex in textures {
        commands.spawn(
            tex
        );
    }

    Ok(())
}

/// Renders all scene meshes with a constant normal map texture, used to debug tangent space on meshes
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_debug_tangent_space(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    queries: (
        Query<&TextureComponent>,
        Query<(&WorldMatrix, &MeshComponent)>
    )) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    let (texture_query, mesh_draw_query) = queries;

    // bind first texture
    if let Some(texture) = (&texture_query).into_iter().next() {
        let usrv = texture.get_srv_index().unwrap() as u32;
        view.cmd_buf.push_render_constants(1, 1, 16, gfx::as_u8_slice(&usrv));
    }

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}