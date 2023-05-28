// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Omni Shadow Map
/// 

use crate::prelude::*;

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn omni_shadow_map(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_omni_shadow_map"
        ],
        update: systems![
            "animate_omni_shadow",
            "batch_lights"
        ],
        render_graph: "mesh_lit_omni_shadow_map",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_omni_shadow_map(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    
    let dim = 32;
    let dim2 = dim / 2;
    let tile_size = 10.0;
    let height = 100.0;
    let spacing = 16.0;
    let extent = dim as f32 * tile_size * spacing;
    let half_extent = extent / 2.0;

    let sm = pmfx.get_texture("single_omni_shadow_map").unwrap();

    // spot light
    let light_pos = vec3f(0.0, height * 0.5, 0.0);
    let light_radius = 256.0;
    commands.spawn((
        Position(light_pos),
        Velocity(Vec3f::unit_z()),
        Colour(vec4f(0.5, 0.25, 0.125, 1.0)),
        LightComponent {
            light_type: LightType::Point,
            radius: light_radius,
            shadow_map_info: pmfx::ShadowMapInfo {
                srv_index: sm.get_srv_index().unwrap() as u32,
                matrix_index: 0
            },
            ..Default::default()
        }
    ));

    // shadow map camera
    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        point_light_capacity: 1,
        shadow_matrix_capacity: 1,
        ..Default::default()
    });

    let offset = (tile_size + spacing) * 3.0;
    let start = vec3f(-half_extent, height, -half_extent) + vec3f(offset, 0.0, offset);
    let mut pos = start;

    for y in 0..dim {    
        pos.x = start.x;
        for x in 0..dim {
            commands.spawn((
                Position(pos),
                Scale(vec3f(tile_size, height, tile_size)),
                Rotation(Quatf::identity()),
                MeshComponent(cube_mesh.clone()),
                WorldMatrix(Mat34f::identity())
            ));

            pos.x += tile_size * spacing;
        }

        pos.z += tile_size * spacing
    }

    // ground plane
    let plane_mesh = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    commands.spawn((
        Position(Vec3f::zero()),
        Scale(vec3f(half_extent, 1.0, half_extent)),
        Rotation(Quatf::identity()),
        MeshComponent(plane_mesh.clone()),
        WorldMatrix(Mat34f::identity())
    ));
    
    pmfx.update_cubemap_camera_constants("omni_shadow_camera", light_pos, 0.1, light_radius * 2.0);

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn animate_omni_shadow (
    time: Res<TimeRes>, 
    mut pmfx: ResMut<PmfxRes>,
    mut light_query: Query<(&mut Position, &mut Velocity, &LightComponent)>) -> Result<(), hotline_rs::Error> {

    let dim = 32;
    let dim2 = dim / 2;
    let tile_size = 10.0;
    let spacing = 16.0;

    let extent = (tile_size + spacing) * 3.0 * 6.0;

    for (mut position, mut velocity, component) in &mut light_query {
        
        position.0 += velocity.0 * time.delta * 400.0;

        if position.z > extent {
            velocity.0 = Vec3f::unit_x();
        }
        
        if position.x > extent {
            velocity.0 = -Vec3f::unit_z();
        }
        
        if position.z < -extent {
            velocity.0 = -Vec3f::unit_x();
        }
        
        if position.x < -extent {
            velocity.0 = Vec3f::unit_z();
        }

        pmfx.update_cubemap_camera_constants("omni_shadow_camera", position.0, 0.1, component.radius * 2.0);
    }

    Ok(())
}