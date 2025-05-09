// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Raytraced Shadows
/// 

use crate::prelude::*;

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn raytraced_shadows(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_raytraced_shadows_scene"
        ],
        update: systems![
            "animate_meshes",
            "animate_lights",
            "batch_lights",
            "setup_tlas"
        ],
        render_graph: "mesh_lit_rt_shadow2",
        ..Default::default()
    }
}

#[export_update_fn]
pub fn setup_raytraced_shadows_scene(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 32);
    let dodeca_mesh = hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0);
    let teapot_mesh = hotline_rs::primitives::create_teapot_mesh(&mut device.0, 32);
    let tube_mesh = hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 5, 0, 4, false, true, 0.33, 0.33, 1.0);
    let triangle_mesh = hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true);
    
    let bounds = 100.0;

    // point light
    let light_bounds = bounds * 0.75;
    let light_pos = vec3f(100.0, 0.0, 100.0);
    let light_radius = 256.0;
    commands.spawn((
        Position(light_pos),
        Velocity(Vec3f::unit_z()),
        Colour(vec4f(0.5, 0.125, 0.25, 1.0)),
        LightComponent {
            light_type: LightType::Point,
            radius: light_radius,
            shadow_map_info: pmfx::ShadowMapInfo {
                srv_index: 0,
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

    let shape_bounds = bounds * 0.6;
    let shape_size = bounds * 0.1;

    // dodeca
    let dodeca_blas = commands.spawn((
        Position(vec3f(shape_bounds * -0.75, shape_bounds * 0.7, -shape_bounds * 0.1)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(dodeca_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &dodeca_mesh)?
    )).id();

    // tepot
    commands.spawn((
        Position(vec3f(shape_bounds * -0.3, shape_bounds * -0.6, shape_bounds * 0.8)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(teapot_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &teapot_mesh)?
    ));

    // tube
    commands.spawn((
        Position(vec3f(shape_bounds * 1.0, shape_bounds * 0.1, shape_bounds * -1.0)),
        Scale(splat3f(shape_size)),
        Rotation(Quatf::identity()),
        MeshComponent(tube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &tube_mesh)?
    ));

    // tri prsim
    commands.spawn((
        Position(vec3f(shape_bounds * 0.123, shape_bounds * -0.6, shape_bounds * -0.8)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(triangle_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &triangle_mesh)?
    ));

    // walls
    let thickness = bounds * 0.1;
    let face_size = bounds * 2.0;

    let wall_offset = (bounds * 2.0) - thickness;

    // -y
    commands.spawn((
        Position(vec3f(0.0, -wall_offset, 0.0)),
        Scale(vec3f(face_size, thickness, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // + y
    commands.spawn((
        Position(vec3f(0.0, wall_offset, 0.0)),
        Scale(vec3f(face_size, thickness, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // -z
    commands.spawn((
        Position(vec3f(0.0, 0.0, -wall_offset)),
        Scale(vec3f(face_size, face_size, thickness)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // -x
    commands.spawn((
        Position(vec3f(-wall_offset, 0.0, 0.0)),
        Scale(vec3f(thickness, face_size, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    commands.spawn(
        TLASComponent {
            tlas: None,
            instance_buffer: None,
            instance_buffer_len: 0,
            instance_geometry_buffer: None
        }
    );
    
    Ok(())
}

#[export_update_fn]
pub fn animate_lights (
    time: Res<TimeRes>,
    mut light_query: Query<(&mut Position, &mut LightComponent)>) -> Result<(), hotline_rs::Error> {

    let extent = 60.0;
    for (mut position, _) in &mut light_query {
        position.0 = vec3f(sin(time.accumulated), cos(time.accumulated), cos(time.accumulated)) * extent;
    }

    Ok(())
}