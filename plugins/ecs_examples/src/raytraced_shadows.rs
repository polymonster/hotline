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
            "setup_raytraced_shadows"
        ],
        update: systems![
            "animate_lights",
            "animate_meshes",
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

#[export_update_fn]
pub fn setup_raytraced_shadows(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let tourus_mesh = hotline_rs::primitives::create_tourus_mesh(&mut device.0, 32);
    let helix_mesh = hotline_rs::primitives::create_helix_mesh(&mut device.0, 32, 4);
    let tube_mesh = hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 32, 0, 32, true, true, 1.0, 0.66, 1.0);
    let triangle_mesh = hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 3, 0, 3, false, true, 0.33, 0.66, 1.0);
    
    let bounds = 100.0;

    // point light
    let light_bounds = bounds * 0.75;
    let light_pos = vec3f(light_bounds, light_bounds, 0.0);
    let light_radius = 256.0;
    commands.spawn((
        Position(light_pos),
        Velocity(Vec3f::unit_z()),
        Colour(vec4f(0.125, 0.5, 0.25, 1.0)),
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

    // tourus
    let tourus_size = bounds * 0.1;
    commands.spawn((
        Position(vec3f(shape_bounds * -0.75, shape_bounds * 0.7, -shape_bounds * 0.1)),
        Scale(splat3f(tourus_size)),
        Rotation(Quatf::identity()),
        MeshComponent(tourus_mesh.clone()),
        WorldMatrix(Mat34f::identity())
    ));

        // create the BLAS itself
        /*
        let blas = device.create_raytracing_blas(&RaytracingBLASInfo {
            geometry: gfx::RaytracingGeometryInfo::Triangles(
                gfx::RaytracingTrianglesInfo {
                    index_buffer: &index_buffer,
                    vertex_buffer: &vertex_buffer,
                    transform3x4: None,
                    index_count: 3,
                    index_format: gfx::Format::R16u,
                    vertex_count: 3,
                    vertex_format: gfx::Format::RGB32f,
                    vertex_stride: 12
                }),
            geometry_flags: gfx::RaytracingGeometryFlags::OPAQUE,
            build_flags: AccelerationStructureBuildFlags::PREFER_FAST_TRACE
        })?;
        */

    // helix
    commands.spawn((
        Position(vec3f(shape_bounds * -0.3, shape_bounds * -0.6, shape_bounds * 0.8)),
        Scale(splat3f(tourus_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(helix_mesh.clone()),
        WorldMatrix(Mat34f::identity())
    ));

    // tube
    commands.spawn((
        Position(vec3f(shape_bounds * 1.0, shape_bounds * 0.1, shape_bounds * -1.0)),
        Scale(splat3f(tourus_size)),
        Rotation(Quatf::identity()),
        MeshComponent(tube_mesh.clone()),
        WorldMatrix(Mat34f::identity())
    ));

    // tri prsim
    commands.spawn((
        Position(vec3f(shape_bounds * 0.123, shape_bounds * -0.6, shape_bounds * -0.8)),
        Scale(splat3f(tourus_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(triangle_mesh.clone()),
        WorldMatrix(Mat34f::identity())
    ));

    // walls
    let thickness = bounds * 0.1;
    let face_size = bounds * 2.0;

    // -y
    commands.spawn((
        Position(vec3f(0.0, -bounds, 0.0)),
        Scale(vec3f(face_size, thickness, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard
    ));

    // + y
    commands.spawn((
        Position(vec3f(0.0, bounds, 0.0)),
        Scale(vec3f(face_size, thickness, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard
    ));

    // -z
    commands.spawn((
        Position(vec3f(0.0, 0.0, -bounds)),
        Scale(vec3f(face_size, face_size, thickness)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard
    ));

    // -x
    commands.spawn((
        Position(vec3f(-bounds, 0.0, 0.0)),
        Scale(vec3f(thickness, face_size, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard
    ));

    /*
    // create a TLAS with a single instance of BLAS
    let tlas = device.create_raytracing_tlas(&RaytracingTLASInfo {
        instances: &vec![RaytracingInstanceInfo {
            transform: [
                1.0, 0.0, 0.0, 0.0, 
                0.0, 1.0, 0.0, 0.0, 
                0.0, 0.0, 1.0, 0.0
            ],
            instance_id: 0,
            instance_mask: 0xff,
            hit_group_index: 0,
            instance_flags: 0,
            blas: &blas
        }],
        build_flags: AccelerationStructureBuildFlags::PREFER_FAST_TRACE
    })?;
    */

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn animate_lights (
    time: Res<TimeRes>,
    mut light_query: Query<(&mut Position, &mut Velocity, &LightComponent)>) -> Result<(), hotline_rs::Error> {

    let extent = 60.0;

    for (mut position, _, _) in &mut light_query {
        
        position.0 = vec3f(sin(time.accumulated), cos(time.accumulated), cos(time.accumulated)) * extent;
    }

    Ok(())
}