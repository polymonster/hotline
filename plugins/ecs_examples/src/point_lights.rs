// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

use crate::prelude::*;

///
/// Point Lights
/// 

/// Init function for primitives demo
#[no_mangle]
pub fn point_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_point_lights"
        ],
        update: systems![
            "animate_point_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit"
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
#[export_update_fn]
pub fn setup_point_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    let num_lights = 64;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 32.0, 0.0);
        let col = match i {
            i if i < 16 => rgba8_to_vec4(0xf89f5bff),
            i if i < 32 => rgba8_to_vec4(0xe53f71ff),
            i if i < 48 => rgba8_to_vec4(0x9c3587ff),
            _ => rgba8_to_vec4(0x66023cff),
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Point,
                ..Default::default()
            }
        ));
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        point_light_capacity: num_lights,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 32),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(meshes[0].clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(10.0)),
                WorldMatrix(Mat34f::identity())
            ));
        }
    }

    // ground plane
    commands.spawn((
        MeshComponent(plane),
        Position(Vec3f::zero()),
        Rotation(Quatf::identity()),
        Scale(splat3f(half_extent * 2.0)),
        WorldMatrix(Mat34f::identity())
    ));

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn animate_point_lights(
    time: Res<TimeRes>, 
    mut light_query: Query<&mut Position, With<LightComponent>>) -> Result<(), hotline_rs::Error> {
    
    let t = time.accumulated;
    let r = sin(t);

    let rot0 = sin(t);
    let rot1 = sin(-t);
    let rot2 = sin(t * 0.5);
    let rot3 = sin(-t * 0.5);
    
    let step = 1.0 / 16.0;
    let mut f = 0.0;
    let mut i = 0;
    for mut position in &mut light_query {
        if i < 16 {
            position.x = r * cos(f32::tau() * f) * 1000.0;
            position.z = r * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
            f += step;
        }
        else if i < 32 {
            position.x = (r + 1.0) * cos(f32::tau() * f) * 1000.0;
            position.z = (r + 1.0) * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot2);
            position.set_xz(pr);
            f += step;
        }
        else if i < 48 {
            position.x = (r - 1.0) * cos(f32::tau() * f) * 1000.0;
            position.z = (r - 1.0) * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot3);
            position.set_xz(pr);
            f += step;
        }
        else if i < 64 {
            position.x = r * 2.0 * cos(f32::tau() * f) * 1000.0;
            position.z = r * 2.0 * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot1);
            position.set_xz(pr);
            f += step;
        }
        i += 1;
    }

    Ok(())
}