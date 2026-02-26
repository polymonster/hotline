// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::prelude::*;

///
/// Claude - Spiral Galaxy
///

/// galaxy disk normal — tilted diagonal
fn galaxy_axis() -> Vec3f {
    normalize(vec3f(1.0, 2.0, 1.0))
}

/// rotate point p around an arbitrary axis by angle (Rodrigues' formula)
fn rotate_around_axis(p: Vec3f, axis: Vec3f, angle: f32) -> Vec3f {
    let c = cos(angle);
    let s = sin(angle);
    p * c + cross(axis, p) * s + axis * (p.x * axis.x + p.y * axis.y + p.z * axis.z) * (1.0 - c)
}

#[no_mangle]
pub fn claude(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_claude"
        ],
        update: systems![
            "update_claude",
            "batch_lights"
        ],
        render_graph: "mesh_lit_dark"
    }
}

#[export_update_fn]
pub fn setup_claude(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let sphere = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16);
    let torus = hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16);

    let axis = galaxy_axis();
    let num_arms = 4;
    let particles_per_arm = 150;
    let mut rng = rand::thread_rng();

    for arm in 0..num_arms {
        let arm_offset = f32::pi() * 2.0 * arm as f32 / num_arms as f32;

        for i in 0..particles_per_arm {
            let t = i as f32 / particles_per_arm as f32;

            // logarithmic spiral in flat XZ
            let theta = arm_offset + t * f32::pi() * 4.0;
            let r = 20.0 + t * 400.0;

            let scatter_r = rng.gen::<f32>() * 20.0 * (1.0 + t);
            let scatter_angle = rng.gen::<f32>() * f32::pi() * 2.0;
            let scatter_y = (rng.gen::<f32>() - 0.5) * 10.0 * (1.0 - t * 0.5);

            let flat_x = r * cos(theta) + scatter_r * cos(scatter_angle);
            let flat_z = r * sin(theta) + scatter_r * sin(scatter_angle);
            let flat_y = scatter_y;

            // tilt from Y-up to galaxy axis
            let flat_pos = vec3f(flat_x, flat_y, flat_z);
            let pos = rotate_around_axis(flat_pos, normalize(cross(Vec3f::unit_y(), axis)), acos(axis.y));

            let scale = 1.0 + rng.gen::<f32>() * 3.0;

            let mesh = if rng.gen::<f32>() > 0.3 {
                sphere.clone()
            } else {
                torus.clone()
            };

            commands.spawn((
                MeshComponent(mesh),
                Position(pos),
                Rotation(Quatf::from_euler_angles(
                    rng.gen::<f32>() * f32::pi() * 2.0,
                    rng.gen::<f32>() * f32::pi() * 2.0,
                    rng.gen::<f32>() * f32::pi() * 2.0
                )),
                Scale(splat3f(scale)),
                WorldMatrix(Mat34f::identity())
            ));
        }
    }

    // dense central cluster
    for _ in 0..50 {
        let r = rng.gen::<f32>() * 30.0;
        let angle = rng.gen::<f32>() * f32::pi() * 2.0;
        let y = (rng.gen::<f32>() - 0.5) * 15.0;
        let flat_pos = vec3f(r * cos(angle), y, r * sin(angle));
        let pos = rotate_around_axis(flat_pos, normalize(cross(Vec3f::unit_y(), axis)), acos(axis.y));
        let scale = 2.0 + rng.gen::<f32>() * 5.0;

        commands.spawn((
            MeshComponent(sphere.clone()),
            Position(pos),
            Rotation(Quatf::identity()),
            Scale(splat3f(scale)),
            WorldMatrix(Mat34f::identity())
        ));
    }

    // --- Lighting ---

    // large warm point light at center to illuminate everything
    commands.spawn((
        Position(vec3f(0.0, 50.0, 0.0)),
        Colour(vec4f(1.0, 0.85, 0.6, 1.0) * 0.8),
        LightComponent {
            light_type: LightType::Point,
            radius: 800.0,
            ..Default::default()
        }
    ));

    // 8 spot lights that will orbit through the galaxy
    let num_spots = 8;
    for i in 0..num_spots {
        let phase = f32::pi() * 2.0 * i as f32 / num_spots as f32;
        let orbit_r = 200.0;
        let flat_pos = vec3f(orbit_r * cos(phase), 30.0, orbit_r * sin(phase));
        let pos = rotate_around_axis(flat_pos, normalize(cross(Vec3f::unit_y(), axis)), acos(axis.y));

        // cycle through warm/cool spot colors
        let col = match i % 4 {
            0 => vec4f(1.0, 0.6, 0.3, 1.0),
            1 => vec4f(0.5, 0.7, 1.0, 1.0),
            2 => vec4f(1.0, 0.4, 0.6, 1.0),
            _ => vec4f(0.6, 1.0, 0.8, 1.0),
        };

        commands.spawn((
            Position(pos),
            Colour(col * 0.5),
            LightComponent {
                light_type: LightType::Spot,
                radius: 400.0,
                ..Default::default()
            }
        ));
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        point_light_capacity: 1,
        spot_light_capacity: num_spots,
        ..Default::default()
    });

    Ok(())
}

#[export_update_fn]
pub fn update_claude(
    time: Res<TimeRes>,
    mut mesh_query: Query<(&mut Position, &mut Rotation), Without<LightComponent>>,
    mut light_query: Query<&mut Position, With<LightComponent>>) -> Result<(), hotline_rs::Error> {

    let dt = time.0.delta;
    let t = time.accumulated;
    let axis = galaxy_axis();

    // orbit galaxy meshes
    for (mut position, mut rotation) in &mut mesh_query {
        let p = position.0;

        // distance from the galaxy axis (perpendicular distance)
        let along = p.x * axis.x + p.y * axis.y + p.z * axis.z;
        let perp = p - axis * along;
        let dist = mag(perp).max(1.0);

        // preserve radius to prevent numerical drift
        let radius = dist;

        // gentle Keplerian orbit around the tilted axis
        let angular_vel = 0.5 / sqrt(dist);
        let angle = angular_vel * dt;

        // rotate around galaxy axis
        let rotated = rotate_around_axis(p, axis, angle);

        // restore perpendicular distance to prevent drift
        let new_along = rotated.x * axis.x + rotated.y * axis.y + rotated.z * axis.z;
        let new_perp = rotated - axis * new_along;
        let new_dist = mag(new_perp).max(0.001);
        let corrected = axis * new_along + new_perp * (radius / new_dist);
        position.0 = corrected;

        // gentle tumble
        rotation.0 *= Quat::from_euler_angles(dt * 0.05, dt * 0.08, dt * 0.03);
        rotation.0 = Quatf::normalize(rotation.0);
    }

    // animate spot lights orbiting through the galaxy
    let tilt_axis = normalize(cross(Vec3f::unit_y(), axis));
    let tilt_angle = acos(axis.y);
    let mut i = 0;
    for mut position in &mut light_query {
        // skip the center point light (first one spawned)
        if i == 0 {
            i += 1;
            continue;
        }

        let si = (i - 1) as f32;
        let phase = f32::pi() * 2.0 * si / 8.0;
        let orbit_r = 150.0 + 100.0 * sin(t * 0.3 + phase);
        let orbit_angle = t * 0.4 + phase;
        let bob_y = 30.0 + 20.0 * sin(t * 0.5 + si);

        let flat_pos = vec3f(
            orbit_r * cos(orbit_angle),
            bob_y,
            orbit_r * sin(orbit_angle)
        );
        position.0 = rotate_around_axis(flat_pos, tilt_axis, tilt_angle);

        i += 1;
    }

    Ok(())
}
