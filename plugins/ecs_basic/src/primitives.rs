// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::client::Client;
use hotline_rs::gfx_platform;
use hotline_rs::os_platform;
use hotline_rs::gfx;

use ecs_base::*;
use ecs_base::SheduleInfo;

use maths_rs::Mat4f;
use maths_rs::vec::*;
use maths_rs::mat::*;

use gfx::CmdBuf;
use gfx::RenderPass;

#[derive(bevy_ecs::prelude::Component)]
struct Billboard;

#[no_mangle]
pub fn primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    // pmfx
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str()).unwrap();
    client.pmfx.create_render_graph(&mut client.device, "checkerboard").unwrap();

    SheduleInfo {
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("checkerboard"),
        setup: vec!["setup_primitives".to_string()]
    }
}

#[no_mangle]
pub fn setup_primitives(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_plane_mesh(&mut device.0, 64),
        hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_cube_mesh(&mut device.0),
    ];

    // square number of rows and columns
    let rc = f32::round(f32::sqrt(meshes.len() as f32));
    let irc = rc as i32; 

    let size = 10.0;
    let half_size = size * 0.5;
    let step = size * 2.5;
    let half_extent = rc * half_size;
    let start_pos = vec3f(half_extent, size, half_extent);

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < meshes.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                commands.spawn((
                    MeshComponent {0: meshes[i].clone()},
                    WorldMatrix { 0: Mat4f::from_translation(iter_pos) * Mat4f::from_scale(splat3f(10.0))},
                ));
            }
            i = i + 1;
        }
    }
}

#[no_mangle]
pub fn render_checkerboard_basic(
    pmfx: bevy_ecs::prelude::Res<PmfxRes>,
    view_name: String,
    view_proj_query: bevy_ecs::prelude::Query<&ViewProjectionMatrix>,
    mesh_draw_query: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let checkerboard = pmfx.get_render_pipeline_for_format("checkerboard_mesh", fmt);
    if checkerboard.is_none() {
        return;
    }

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&checkerboard.unwrap());

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