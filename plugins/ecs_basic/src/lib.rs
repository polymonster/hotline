// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::client::Client;
use hotline_rs::gfx_platform;
use hotline_rs::os_platform;
use hotline_rs::gfx;

use ecs_base::*;
use ecs_base::SheduleInfo;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;

use gfx::CmdBuf;
use gfx::RenderPass;

#[derive(Component)]
struct Billboard;

mod primitives;

#[no_mangle]
pub fn billboard(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    // pmfx
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str())
        .expect("expected to have pmfx: data/shaders/basic");
    client.pmfx.create_render_graph(&mut client.device, "billboard")
        .expect("expected to have render graph: basic");

    SheduleInfo {
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("billboard"),
        setup: vec!["setup_billboard".to_string()]
    }
}

#[no_mangle]
pub fn setup_billboard(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let billboard_mesh = hotline_rs::primitives::create_billboard_mesh(&mut device.0);
    commands.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: billboard_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_scale(splat3f(10.0))},
        Billboard
    ));
}

#[no_mangle]
fn render_billboards_basic(
    pmfx: Res<PmfxRes>,
    view_name: String,
    view_proj_query: Query<&ViewProjectionMatrix>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let billboard_mesh = pmfx.get_render_pipeline_for_format("billboard_mesh", fmt);
    if billboard_mesh.is_none() {
        return;
    }

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&billboard_mesh.unwrap());

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
pub fn multiple(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    client.pmfx.create_render_graph(&mut client.device, "forward").unwrap();
    SheduleInfo {
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("forward"),
        setup: vec!["setup_multiple".to_string()]
    }
}

#[no_mangle]
pub fn setup_multiple(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands:  bevy_ecs::system::Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let dim = 64;
    let dim2 = dim / 2;
    let cube_size = 2.5;

    let half_extent = dim2 as f32 * cube_size;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;
            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            let pos = Mat4f::from_translation(
                vec3f(
                    x as f32 * cube_size - half_extent, 
                    50.0, 
                    y as f32 * cube_size - cube_size * dim as f32 + half_extent
                )
            );

            let scale = Mat4::from_scale(vec3f(1.0, wave_x + wave_y + wave_h, 1.0));

            commands.spawn((
                Position(Vec3f::zero()),
                Velocity(Vec3f::one()),
                MeshComponent(cube_mesh.clone()),
                WorldMatrix(pos * scale)
            ));
        }
    }
}

#[derive(Component)]
struct Heightmap;

#[no_mangle]
pub fn heightmap(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    // pmfx
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str())
        .expect("expected to have pmfx: data/shaders/basic");
    client.pmfx.create_render_graph(&mut client.device, "heightmap")
        .expect("expected to have render graph: heightmap");

    SheduleInfo {
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("heightmap"),
        setup: vec!["setup_heightmap".to_string()]
    }
}

#[no_mangle]
pub fn setup_heightmap(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let plane_mesh = hotline_rs::primitives::create_plane_mesh(&mut device.0, 64);
    commands.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: plane_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_scale(splat3f(500.0))},
        Heightmap
    ));
}

#[no_mangle]
fn render_heightmap_basic(
    pmfx: Res<PmfxRes>,
    view_name: String,
    view_proj_query: Query<&ViewProjectionMatrix>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let heightmap_mesh = pmfx.get_render_pipeline_for_format("heightmap_mesh", fmt);
    if heightmap_mesh.is_none() {
        return;
    }

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&heightmap_mesh.unwrap());

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

//
// Plugin
//

#[no_mangle]
pub fn get_demos_ecs_basic() -> Vec<String> {
    vec![
        "primitives".to_string(),
        "billboard".to_string(),
        "cube".to_string(),
        "multiple".to_string(),
        "heightmap".to_string(),
    ]
}

#[no_mangle]
pub fn get_system_ecs_basic(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        // setup functions
        "setup_cube" => ecs_base::system_func![crate::primitives::setup_cube],
        "setup_primitives" => ecs_base::system_func![crate::primitives::setup_primitives],
        "setup_billboard" => ecs_base::system_func![setup_billboard],
        "setup_multiple" => ecs_base::system_func![setup_multiple],
        "setup_heightmap" => ecs_base::system_func![setup_heightmap],
        // render functions
        "render_billboards_basic" => ecs_base::view_func![render_billboards_basic, "render_billboards_basic"],
        "render_heightmap_basic" => ecs_base::view_func![render_heightmap_basic, "render_heightmap_basic"],
        "render_checkerboard_basic" => ecs_base::view_func![crate::primitives::render_checkerboard_basic, "render_checkerboard_basic"],
        _ => std::hint::black_box(None)
    }
}