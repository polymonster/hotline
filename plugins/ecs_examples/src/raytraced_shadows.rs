// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Raytraced Shadows
/// 

use crate::prelude::*;

#[derive(Component)]
pub struct BLAS {
    pub blas: gfx_platform::RaytracingBLAS
}

#[derive(Component)]
pub struct TLAS {
    pub tlas: Option<gfx_platform::RaytracingTLAS>
}

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn raytraced_shadows(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_raytraced_shadows_scene"
        ],
        update: systems![
            "setup_raytraced_shadows_tlas",
            "animate_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit_rt_shadow",
        ..Default::default()
    }
}

pub fn blas_from_mesh(device: &mut ResMut<DeviceRes>, mesh: &pmfx::Mesh<gfx_platform::Device>) -> Result<BLAS, hotline_rs::Error> {
    Ok(BLAS {
        blas: device.create_raytracing_blas(&gfx::RaytracingBLASInfo {
            geometry: gfx::RaytracingGeometryInfo::Triangles(
                gfx::RaytracingTrianglesInfo {
                    index_buffer: &mesh.ib,
                    vertex_buffer: &mesh.vb,
                    transform3x4: None,
                    index_count: 3,
                    index_format: gfx::Format::R16u,
                    vertex_count: 3,
                    vertex_format: gfx::Format::RGB32f,
                    vertex_stride: 56
                }),
            geometry_flags: gfx::RaytracingGeometryFlags::OPAQUE,
            build_flags: gfx::AccelerationStructureBuildFlags::PREFER_FAST_TRACE
        })?
    })
}

#[export_update_fn]
pub fn setup_raytraced_shadows_scene(
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
    let tourus_blas = commands.spawn((
        Position(vec3f(shape_bounds * -0.75, shape_bounds * 0.7, -shape_bounds * 0.1)),
        Scale(splat3f(tourus_size)),
        Rotation(Quatf::identity()),
        MeshComponent(tourus_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &tourus_mesh)?
    )).id();

    // helix
    commands.spawn((
        Position(vec3f(shape_bounds * -0.3, shape_bounds * -0.6, shape_bounds * 0.8)),
        Scale(splat3f(tourus_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(helix_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &helix_mesh)?
    ));

    // tube
    commands.spawn((
        Position(vec3f(shape_bounds * 1.0, shape_bounds * 0.1, shape_bounds * -1.0)),
        Scale(splat3f(tourus_size)),
        Rotation(Quatf::identity()),
        MeshComponent(tube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &tube_mesh)?
    ));

    // tri prsim
    commands.spawn((
        Position(vec3f(shape_bounds * 0.123, shape_bounds * -0.6, shape_bounds * -0.8)),
        Scale(splat3f(tourus_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(triangle_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &triangle_mesh)?
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
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // + y
    commands.spawn((
        Position(vec3f(0.0, bounds, 0.0)),
        Scale(vec3f(face_size, thickness, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // -z
    commands.spawn((
        Position(vec3f(0.0, 0.0, -bounds)),
        Scale(vec3f(face_size, face_size, thickness)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    // -x
    commands.spawn((
        Position(vec3f(-bounds, 0.0, 0.0)),
        Scale(vec3f(thickness, face_size, face_size)),
        Rotation(Quatf::identity()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        Billboard,
        blas_from_mesh(&mut device, &cube_mesh)?
    ));

    commands.spawn((
        TLAS {
            tlas: None
        }
    ));

    // 
    
    Ok(())
}

#[export_update_fn]
pub fn setup_raytraced_shadows_tlas(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut entities_query: Query<(&mut Position, &mut Scale, &mut Rotation, &BLAS)>,
    mut tlas_query: Query<&mut TLAS>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // ..
    for mut t in &mut tlas_query {
        if t.tlas.is_none() {
            let mut instances = Vec::new();
            for (index, (position, scale, rotation, blas)) in &mut entities_query.iter().enumerate() {
                let translate = Mat34f::from_translation(position.0);
                let rotate = Mat34f::from(rotation.0);
                let scale = Mat34f::from_scale(scale.0);
                instances.push(
                    gfx::RaytracingInstanceInfo::<gfx_platform::Device> {
                        transform: (translate * rotate * scale).m,
                        instance_id: index as u32,
                        instance_mask: 0xff,
                        hit_group_index: 0,
                        instance_flags: 0,
                        blas: &blas.blas
                    }
                );
                let tlas = device.create_raytracing_tlas(&gfx::RaytracingTLASInfo {
                    instances: &instances,
                    build_flags: gfx::AccelerationStructureBuildFlags::PREFER_FAST_TRACE
                })?;
                
                t.tlas = Some(tlas);
            }
        }
    }

    Ok(())
}

#[export_update_fn]
pub fn animate_lights (
    time: Res<TimeRes>,
    mut light_query: Query<(&mut Position, &mut Scale, &mut LightComponent)>) -> Result<(), hotline_rs::Error> {

    let extent = 60.0;

    for (mut position, _, _) in &mut light_query {
        position.0 = vec3f(sin(time.accumulated), cos(time.accumulated), cos(time.accumulated)) * extent;
    }

    Ok(())
}

#[export_compute_fn]
pub fn render_meshes_raytraced(    
    pmfx: &Res<PmfxRes>,
    pass: &pmfx::ComputePass<gfx_platform::Device>,
    tlas_query: Query<&TLAS>
) -> Result<(), hotline_rs::Error> {

    let pmfx = &pmfx.0;

    let output_size = pmfx.get_texture_2d_size("staging_output").expect("expected staging_output");
    let output_tex = pmfx.get_texture("staging_output").expect("expected staging_output");

    let cam = pmfx.get_camera_constants("main_camera");
    if let Ok(cam) = cam {
        for t in &tlas_query {            
            if let Some(tlas) = &t.tlas {

                // set pipeline
                let rt_pipeline = pmfx.get_raytracing_pipeline(&pass.pass_pipline)?;
                pass.cmd_buf.set_raytracing_pipeline(&rt_pipeline.pipeline);

                // camera constants TODO:

                // resource use constants
                let using_slot = rt_pipeline.pipeline.get_pipeline_slot(0, 1, gfx::DescriptorType::PushConstants);
                if let Some(slot) = using_slot {
                    for i in 0..pass.use_indices.len() {
                        let num_constants = gfx::num_32bit_constants(&pass.use_indices[i]);
                        pass.cmd_buf.push_compute_constants(
                            slot.index, 
                            num_constants, 
                            i as u32 * num_constants, 
                            gfx::as_u8_slice(&pass.use_indices[i])
                        );
                    }
                }

                pass.cmd_buf.set_heap(&rt_pipeline.pipeline, &pmfx.shader_heap);

                // dispatch
                pass.cmd_buf.dispatch_rays(&rt_pipeline.sbt, gfx::Size3 {
                    x: output_size.0 as u32,
                    y: output_size.1 as u32,
                    z: 1
                });
            }
        }
    }

    Ok(())
}