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
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_omni_shadow_map(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let pyramid_mesh = hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true);
    
    let dim = 32;
    let dim2 = dim / 2;
    let tile_size = 5.0;
    let extent = dim as f32 * tile_size * 4.0;
    let half_extent = extent / 2.0;

    // spot light
    let light_pos = normalize(vec3f(0.0, 32.0, 0.0));
    commands.spawn((
        Position(Vec3f::zero()),
        Colour(vec4f(0.5, 0.25, 0.125, 1.0)),
        LightComponent {
            light_type: LightType::Point,
            radius: 64.0,
            ..Default::default()
        }
    ));

    // shadow map camera
    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        point_light_capacity: 1,
        shadow_matrix_capacity: 1,
        ..Default::default()
    });

    let start = vec3f(-half_extent, tile_size, -half_extent);
    let mut pos = start;

    for y in 0..dim {    
        pos.x = start.x;
        for x in 0..dim {
            commands.spawn((
                Position(pos),
                Scale(vec3f(tile_size, tile_size, tile_size)),
                Rotation(Quatf::identity()),
                MeshComponent(pyramid_mesh.clone()),
                WorldMatrix(Mat34f::identity())
            ));

            pos.x += tile_size * 4.0;
        }

        pos.z += tile_size * 4.0
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

    Ok(())
}