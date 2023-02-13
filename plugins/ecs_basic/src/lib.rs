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
pub fn setup_single2(
    mut device: bevy_ecs::change_detection::ResMut<ecs::DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::identity()}
    ));

    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::from_translation(vec3f(0.0, 5.0, 5.0))}
    ));

    commands.spawn((
        ecs::Position { 0: Vec3f::zero() },
        ecs::Velocity { 0: Vec3f::one() },
        ecs::MeshComponent {0: cube_mesh.clone()},
        ecs::WorldMatrix { 0: Mat4f::from_translation(vec3f(0.0, 10.0, 0.0))}
    ));
}

#[no_mangle]
pub fn get_system_function_empty2(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        "setup_single2" => system_func![setup_single2],
        _ => None
    }
}