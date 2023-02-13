// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;

#[no_mangle]
pub fn setup_empty(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::identity()}
    ));
}

#[no_mangle]
pub fn setup_cube(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::identity()}
    ));
}

#[no_mangle]
pub fn setup_multiple(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands:  bevy_ecs::system::Commands) {

    commands.spawn((
        ecs::Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
        ecs::Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ecs::ViewProjectionMatrix { 0: Mat4f::identity()},
        ecs::Camera,
    ));

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    let dim = 500;
    let dim2 = dim / 2;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;

            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            commands.spawn((
                ecs::Position { 0: Vec3f::zero() },
                ecs::Velocity { 0: Vec3f::one() },
                ecs::MeshComponent {0: cube_mesh.clone()},
                ecs::WorldMatrix { 0: Mat4f::from_translation(
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
pub fn movement(mut query:  bevy_ecs::system::Query<(&mut ecs::Position, &ecs::Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

#[no_mangle]
pub fn mat_movement(mut query:  bevy_ecs::system::Query<&mut ecs::WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.0, 0.0));
    }
}

#[no_mangle]
pub fn billboard() {

}

#[no_mangle]
pub fn cube() {
    
}

#[no_mangle]
pub fn get_demos_ecs_basic() -> Vec<String> {
    vec![
        "billboard".to_string(),
        "cube".to_string(),
        "multiple_cubes".to_string()
    ]
}

#[no_mangle]
pub fn get_system_ecs_basic(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "cube" => system_func![setup_cube],
        _ => {
            // weird! we need to print here otherwise it can cause access violation, I wonder of the return value is being
            // optimised away?
            print!("");
            None
        }
    }
}