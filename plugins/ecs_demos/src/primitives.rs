// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;

#[derive(bevy_ecs::prelude::Component)]
struct Billboard;

/// Init function for primitives demo
#[no_mangle]
pub fn primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str()).unwrap();
    
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        update: systems![
            "update_cameras",
            "update_main_camera_config"
        ],
        render_graph: "mesh_debug".to_string()
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_primitives(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_plane_mesh(&mut device.0, 1),
        
        hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_cube_mesh(&mut device.0),
        hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1),

        crate::dev::create_sphere_mesh(&mut device.0, 16),
        crate::dev::create_sphere_mesh_ex(&mut device.0, 16, 8, true),

        crate::dev::create_prism_mesh(&mut device.0, 3, false, true),
        crate::dev::create_prism_mesh(&mut device.0, 4, false, true),
        crate::dev::create_prism_mesh(&mut device.0, 5, false, true),
        crate::dev::create_cylinder_mesh(&mut device.0, 16),
        
        crate::dev::create_pyramid_mesh(&mut device.0, 4, false, true),
        crate::dev::create_pyramid_mesh(&mut device.0, 5, false, true),
        crate::dev::create_cone_mesh(&mut device.0, 16),

        crate::dev::create_cube_subdivision_mesh(&mut device.0, 1),
        crate::dev::create_capsule_mesh(&mut device.0, 16),

        crate::dev::create_tourus_mesh(&mut device.0, 16),
    ];

    // square number of rows and columns
    let rc = f32::ceil(f32::sqrt(meshes.len() as f32));
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size, -half_extent * 4.0);

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < meshes.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                commands.spawn((
                    MeshComponent(meshes[i].clone()),
                    WorldMatrix(Mat4f::from_translation(iter_pos) * Mat4f::from_scale(splat3f(10.0))),
                ));
            }
            i = i + 1;
        }
    }
}

#[no_mangle]
pub fn render_meshes(
    pmfx: bevy_ecs::prelude::Res<PmfxRes>,
    view_name: String,
    view_proj_query: bevy_ecs::prelude::Query<&ViewProjectionMatrix>,
    mesh_draw_query: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name);
    if arc_view.is_none() {
        return;
    }
    let arc_view = arc_view.unwrap();
    
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let mesh_debug = pmfx.get_render_pipeline_for_format("mesh_debug", fmt);
    if mesh_debug.is_none() {
        return;
    }
    let mesh_debug = mesh_debug.unwrap();

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&mesh_debug);

    for view_proj in &view_proj_query {
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);
        for (world_matrix, mesh) in &mesh_draw_query {
            // draw
            view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
            view.cmd_buf.set_index_buffer(&mesh.0.ib);
            view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
            view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        }
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();
}

#[no_mangle]
pub fn render_wireframe(
    pmfx: bevy_ecs::prelude::Res<PmfxRes>,
    view_name: String,
    view_proj_query: bevy_ecs::prelude::Query<&ViewProjectionMatrix>,
    mesh_draw_query: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name);
    if arc_view.is_none() {
        //println!("missing view: {}", view_name);
        return;
    }
    let arc_view = arc_view.unwrap();

    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let wireframe = pmfx.get_render_pipeline_for_format("wireframe_overlay", fmt);
    if wireframe.is_none() {
        return;
    }
    let wireframe = wireframe.unwrap();

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&wireframe);

    for view_proj in &view_proj_query {
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);
        for (world_matrix, mesh) in &mesh_draw_query {
            // draw
            view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
            view.cmd_buf.set_index_buffer(&mesh.0.ib);
            view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
            view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        }
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();
}