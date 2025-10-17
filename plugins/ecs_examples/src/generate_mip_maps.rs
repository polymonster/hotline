// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Generate Mip Maps
/// 

use crate::prelude::*;

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn generate_mip_maps(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_generate_mip_maps"
        ],
        render_graph: "generate_mip_maps",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_generate_mip_maps(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 32);
    let dim = 64;
    let sphere_size = 5.0;

    let mut angle = 0.0;
    let mut offset = 0.0;

    let step = f32::tau() / dim as f32;

    for _ in 0..dim {    
        for _ in 0..dim {    

            let wave_x = f32::sin(angle) * (sphere_size + offset);
            let wave_y = f32::cos(angle) * (sphere_size + offset);

            let pos = Mat34f::from_translation(
                vec3f(
                    wave_x, 
                    50.0, 
                    wave_y
                )
            );

            let scale = Mat34::from_scale(splat3f(sphere_size));

            commands.spawn((
                Position(Vec3f::zero()),
                Velocity(Vec3f::one()),
                MeshComponent(sphere_mesh.clone()),
                WorldMatrix(pos * scale)
            ));

            angle += step;
            offset += step * 2.0;
        }
    }

    Ok(())
}