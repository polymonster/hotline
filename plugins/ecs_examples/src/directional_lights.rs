// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::prelude::*;

///
/// Directional Lights
/// 

/// Init function for primitives demo
#[no_mangle]
pub fn directional_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_directional_lights"
        ],
        update: systems![
            "animate_directional_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit"
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
#[export_update_fn]
pub fn setup_directional_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    // animating lights
    let num_lights = 4;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 128.0, 0.0);

        let col = match i {
            0 => vec4f(0.25, 0.0, 0.25, 0.5),
            1 => vec4f(0.25, 0.25, 0.0, 0.5),
            2 => vec4f(0.0, 0.25, 0.25, 0.5),
            _ => vec4f(0.25, 0.0, 0.5, 0.5)
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Directional,
                direction: normalize(vec3f(0.5, -0.5, 0.5)),
                ..Default::default()
            }
        ));
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        directional_light_capacity: num_lights,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let height = 10.0;
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
                Rotation(Quatf::identity()),
                Scale(vec3f(size, height, size)),
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
pub fn animate_directional_lights(
    time: Res<TimeRes>, 
    mut light_query: Query<(&mut Position, &mut LightComponent)>) -> Result<(), hotline_rs::Error> {
    
    let t = time.accumulated;
    let r = sin(t);
    let rot0 = sin(t);
    
    let step = 1.0 / 4.0;
    let mut f = 0.0;
    for (mut position, mut light) in &mut light_query {
        position.x = r * (cos(f32::tau() * f) * 2.0 - 1.0) * 500.0;
        position.z = r * (sin(f32::tau() * f) * 2.0 - 1.0) * 500.0;
        
        let pr = rotate_2d(position.xz(), rot0);
        position.set_xz(pr);

        // derive direction from position, always look at the origin
        light.direction = normalize(-position.0);

        f += step;
    }

    Ok(())
}