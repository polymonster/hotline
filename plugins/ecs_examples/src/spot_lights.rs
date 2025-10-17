// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

//
// Spot Lights
//

use crate::prelude::*;

/// Init function for primitives demo
#[no_mangle]
pub fn spot_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_spot_lights"
        ],
        update: systems![
            "animate_spot_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit"
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
#[export_update_fn]
pub fn setup_spot_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    // animating lights
    let num_lights = 64;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 128.0, 0.0);
        let col = match i {
            i if i < 16 => rgba8_to_vec4(0xA3C9A8FF),
            i if i < 32 => rgba8_to_vec4(0xA3C9A8FF),
            i if i < 48 => rgba8_to_vec4(0xA3C9A8FF),
            _ => rgba8_to_vec4(0xA3C9A8FF),
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Spot,
                ..Default::default()
            }
        ));
    }

    // fixed spots
    commands.spawn((
        Position(vec3f(0.0, 2000.0, 0.0)),
        Colour(Vec4f::cyan() * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    let height = 1000.0;
    let edge = 1500.0;
    let col = vec4f(1.0, 0.5, 0.1, 1.0);

    commands.spawn((
        Position(vec3f(-edge, height, -edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(edge, height, -edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(edge, height, edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(-edge, height, edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        spot_light_capacity: num_lights + 5,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 32),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let height = 50.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, height, y as f32 * step);
            commands.spawn((
                MeshComponent(meshes[0].clone()),
                Position(iter_pos),
                Rotation(Quatf::from_euler_angles(0.0, 0.0, 0.0)),
                Scale(vec3f(size, height, size)),
                WorldMatrix(Mat34f::identity())
            ));
        }
    }

    // ground plane
    commands.spawn((
        MeshComponent(plane.clone()),
        Position(Vec3f::zero()),
        Rotation(Quatf::identity()),
        Scale(splat3f(half_extent * 2.0)),
        WorldMatrix(Mat34f::identity())
    ));

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn animate_spot_lights(
    time: Res<TimeRes>, 
    mut light_query: Query<&mut Position, With<LightComponent>>) -> Result<(), hotline_rs::Error> {
    
    let t = time.accumulated;
    let rot0 = t;
    
    let mut i = 0;
    for mut position in &mut light_query {
        if i < 16 {
            let fi = i as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
        }
        else if i < 32 {
            let fi = (i-16) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = -sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), -rot0);
            position.set_xz(pr);
        }
        else if i < 48 {
            let fi = (i-32) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = -cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), -rot0);
            position.set_xz(pr);
        }
        else if i < 64 {
            let fi = (i-48) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = -sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = -cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
        }
        else {
            let pr = rotate_2d(position.xz(), sin(rot0));
            position.set_xz(pr);
        }
        i += 1;
    }

    Ok(())
}