// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::ecs::*;
use hotline_rs::primitives;

use maths_rs::Vec3f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

use hotline_rs::system_func;

#[no_mangle]
pub fn setup_single(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let cube_mesh = primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: cube_mesh.clone()},
        WorldMatrix { 0: Mat4f::identity()}
    ));

    commands.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: cube_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_translation(vec3f(0.0, 0.0, 0.0))}
    ));
}


#[no_mangle]
pub fn setup_multiple(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    commands.spawn((
        Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
        Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ViewProjectionMatrix { 0: Mat4f::identity()},
        Camera,
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
pub fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

#[no_mangle]
pub fn mat_movement(mut query: Query<&mut WorldMatrix>) {
    for mut mat in &mut query {
        mat.0 = mat.0 * Mat4f::from_translation(vec3f(0.0, 0.1, 0.0));
    }
}

#[no_mangle]
pub fn get_demo_names() -> Vec<String> {
    vec![
        "single".to_string(),
        "multiple".to_string()
    ]
}

#[no_mangle]
pub fn get_demo_info(name: &str) -> Option<client::DemoInfo> {   
    match name {
        "single" => Some(client::DemoInfo {
            setup_systems: vec!["setup_single".to_string()],
            update_systems: vec!["mat_movement".to_string()],
            render_graph: "forward".to_string()
        }),
        "multiple" => Some(client::DemoInfo {
            setup_systems: vec!["setup_single".to_string()],
            update_systems: vec!["mat_movement".to_string()],
            render_graph: "forward".to_string()
        }),
        _ => None
    }
}

#[no_mangle]
pub fn get_system_function_lib(name: String) -> Option<SystemDescriptor> {    
    match name.as_str() {
        "mat_movement" => system_func![mat_movement],
        "setup_single" => system_func![setup_multiple],
        "setup_multiple" => system_func![setup_multiple],
        _ => None
    }
}