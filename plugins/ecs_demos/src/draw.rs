// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn draw_indexed_push_constants(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_push_constants"
        ],
        update: systems![
            "update_cameras",
            "update_main_camera_config"
        ],
        render_graph: "mesh_debug".to_string()
    }
}

#[no_mangle]
pub fn setup_draw_indexed_push_constants(
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

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw_indexed(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/debug").as_str()).unwrap();

    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed"
        ],
        update: systems![
            "update_cameras",
            "update_main_camera_config"
        ],
        render_graph: "mesh_debug".to_string(),
    }
}

#[no_mangle]
pub fn setup_draw_indexed(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let pos = Mat4f::from_translation(Vec3f::unit_y() * 10.0);
    let scale = Mat4f::from_scale(splat3f(10.0));

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}