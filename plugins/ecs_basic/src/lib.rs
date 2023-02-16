// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]
use hotline_rs::client::Client;

use hotline_rs::gfx_platform;
use hotline_rs::os_platform;

use ecs_base::*;
use ecs_base::SheduleInfo;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;

#[no_mangle]
pub fn setup_billboard(
    _device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    _commands: bevy_ecs::system::Commands) {
}

#[no_mangle]
pub fn setup_cube(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: cube_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_translation(Vec3f::unit_x() * 0.0)}
    ));
}

#[no_mangle]
pub fn setup_multiple(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands:  bevy_ecs::system::Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let dim = 50;
    let dim2 = dim / 2;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;

            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            commands.spawn((
                Position { 0: Vec3f::zero() },
                Velocity { 0: Vec3f::one() },
                MeshComponent {0: cube_mesh.clone()},
                WorldMatrix { 0: Mat4f::from_translation(
                    vec3f(
                        x as f32 * 2.5 - dim2 as f32 * 2.5, 
                        0.0, 
                        y as f32 * 2.5 - 2.5 * dim as f32)) * 
                        Mat4::from_scale(vec3f(1.0, wave_x + wave_y + wave_h, 1.0)) }
            ));
        }
    }
}

#[no_mangle]
pub fn movement(mut query:  bevy_ecs::system::Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

#[no_mangle]
pub fn mat_movement(mut query:  bevy_ecs::system::Query<&mut WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.0, 0.0));
    }
}

#[no_mangle]
pub fn billboard(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    client.pmfx.create_render_graph(&mut client.device, "forward").unwrap();
    SheduleInfo {
        update: vec![
            "mat_movement".to_string(),
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("forward"),
        setup: vec!["setup_billboard".to_string()]
    }
}

#[no_mangle]
pub fn cube(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str())
        .expect("expected to have pmfx: data/shaders/basic");
    client.pmfx.create_render_graph(&mut client.device, "basic")
        .expect("expected to have render pipeline: basic");
    SheduleInfo {
        update: vec![
            "mat_movement".to_string(),
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("basic"),
        setup: vec!["setup_cube".to_string()]
    }
}

#[no_mangle]
pub fn multiple(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    client.pmfx.create_render_graph(&mut client.device, "forward").unwrap();
    SheduleInfo {
        update: vec![
            "mat_movement".to_string(),
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("forward"),
        setup: vec!["setup_multiple".to_string()]
    }
}

#[no_mangle]
pub fn get_demos_ecs_basic() -> Vec<String> {
    vec![
        "billboard".to_string(),
        "cube".to_string(),
        "multiple".to_string()
    ]
}

#[no_mangle]
pub fn get_system_ecs_basic(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "setup_billboard" => ecs_base::system_func![setup_billboard],
        "setup_cube" => ecs_base::system_func![setup_cube],
        "setup_multiple" => ecs_base::system_func![setup_multiple],
        _ => {
            // TODO:
            // weird! we need to print here otherwise it can cause access violation, I wonder of the return value is being
            // optimised away?
            print!("");
            None
        }
    }
}