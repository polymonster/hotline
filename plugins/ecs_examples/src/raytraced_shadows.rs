// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::gfx::{CpuAccessFlags, RaytracingTLAS, ResourceView};

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
    pub tlas: Option<gfx_platform::RaytracingTLAS>,
    pub instance_buffer: Option<gfx_platform::Buffer>,
    pub instance_buffer_len: usize,
    pub instance_srv_buffer: gfx_platform::Buffer
}

pub struct GeometryLookup {
    pub ib_srv: u32,
    pub vb_srv: u32,
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
            "animate_meshes",
            "animate_lights",
            "batch_lights",
            "setup_tlas"
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
                    index_count: mesh.num_indices as usize,
                    index_format: gfx::Format::R16u,
                    vertex_count: mesh.num_vertices as usize,
                    vertex_format: gfx::Format::RGB32f,
                    vertex_stride: std::mem::size_of::<hotline_rs::primitives::Vertex3D>()
                }),
            geometry_flags: gfx::RaytracingGeometryFlags::OPAQUE,
            build_flags: gfx::AccelerationStructureBuildFlags::PREFER_FAST_TRACE
        })?
    })
}

pub fn geometry_lookup_from_mesh(device: &mut ResMut<DeviceRes>, heap: &mut gfx_platform::Heap, mesh: &pmfx::Mesh<gfx_platform::Device>) -> Result<GeometryLookup, hotline_rs::Error> {
    Ok(GeometryLookup {
        ib_srv: device.create_resource_view(&gfx::ResourceViewInfo {
            view_type: gfx::ResourceView::ShaderResource,
            format: gfx::Format::Unknown,
            first_element: 0,
            structure_byte_size: std::mem::size_of::<u16>(),
            num_elements: mesh.num_indices as usize
        },
        gfx::Resource::Buffer(&mesh.ib),
        heap)? as u32,
        vb_srv: device.create_resource_view(&gfx::ResourceViewInfo {
            view_type: gfx::ResourceView::ShaderResource,
            format: gfx::Format::Unknown,
            first_element: 0,
            structure_byte_size: std::mem::size_of::<hotline_rs::primitives::Vertex3D>(),
            num_elements: mesh.num_vertices as usize
        },
        gfx::Resource::Buffer(&mesh.vb),
        heap)? as u32
    })
}

#[export_update_fn]
pub fn setup_raytraced_shadows_scene(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
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

    // TODO: create srv indices
    let mut instance_srv_indices : Vec<u32> = vec![
        8, 9, 10, 11, 12, 13, 14, 15, 16
    ];

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

    let mut instance_geometry_lookup = Vec::new();
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &dodeca_mesh)?);
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &teapot_mesh)?);
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &tube_mesh)?);
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &triangle_mesh)?);

    // create a buffer for srv indices
    let instance_srv_buffer = device.create_buffer_with_heap::<u8>(&gfx::BufferInfo{
            usage: gfx::BufferUsage::SHADER_RESOURCE,
            cpu_access: CpuAccessFlags::NONE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of_val(&instance_srv_indices[0]),
            num_elements: instance_srv_indices.len(),
            initial_state: gfx::ResourceState::ShaderResource
        }, 
        Some(gfx::slice_as_u8_slice(instance_srv_indices.as_slice())),
        &mut pmfx.shader_heap
    )?;

    commands.spawn(
        TLAS {
            tlas: None,
            instance_buffer: None,
            instance_buffer_len: 0,
            instance_srv_buffer
        }
    );
    
    Ok(())
}

#[export_update_fn]
pub fn setup_tlas(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut entities_query: Query<(&mut Position, &mut Scale, &mut Rotation, &BLAS)>,
    mut tlas_query: Query<&mut TLAS>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // ..
    for mut t in &mut tlas_query {
        let mut instances = Vec::new();
        for (index, (position, scale, rotation, blas)) in &mut entities_query.iter().enumerate() {
            let translate = Mat34f::from_translation(position.0);
            let rotate = Mat34f::from(rotation.0);
            let scale = Mat34f::from_scale(scale.0);
            let flip = Mat34f::from_scale(vec3f(1.0, 1.0, 1.0));
            instances.push(
                gfx::RaytracingInstanceInfo::<gfx_platform::Device> {
                    transform: (flip * translate * rotate * scale).m,
                    instance_id: index as u32,
                    instance_mask: 0xff,
                    hit_group_index: 0,
                    instance_flags: 0,
                    blas: &blas.blas
                }
            );
        }
        if t.tlas.is_none() {
            let tlas = device.create_raytracing_tlas_with_heap(&gfx::RaytracingTLASInfo {
                instances: &instances,
                build_flags: gfx::AccelerationStructureBuildFlags::PREFER_FAST_TRACE |
                    gfx::AccelerationStructureBuildFlags::ALLOW_UPDATE
                },
                &mut pmfx.shader_heap
            )?;
            let tlas_srv =  tlas.get_srv_index().expect("expect tlas to have an srv");
            pmfx.push_constant_user_data[0] = tlas_srv as u32;
            t.tlas = Some(tlas);
        }
    }

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

#[export_compute_fn]
pub fn update_tlas(
    mut device: ResMut<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    mut pass: &mut pmfx::ComputePass<gfx_platform::Device>,
    mut entities_query: Query<(&mut Position, &mut Scale, &mut Rotation, &BLAS)>,
    mut tlas_query: Query<&mut TLAS>,
) -> Result<(), hotline_rs::Error> {
    let pmfx = &pmfx.0;

    // update tlas
    let mut first = true;
    for mut t in &mut tlas_query {
        let mut instances = Vec::new();
        for (index, (position, scale, rotation, blas)) in &mut entities_query.iter().enumerate() {
            let translate = Mat34f::from_translation(position.0);
            let rotate = Mat34f::from(rotation.0);
            let scale = Mat34f::from_scale(scale.0);
            let flip = Mat34f::from_scale(vec3f(1.0, 1.0, 1.0));
            instances.push(
                gfx::RaytracingInstanceInfo::<gfx_platform::Device> {
                    transform: (flip * translate * rotate * scale).m,
                    instance_id: index as u32,
                    instance_mask: 0xff,
                    hit_group_index: 0,
                    instance_flags: 0,
                    blas: &blas.blas
                }
            );
        }
        
        if let Some(tlas) = t.tlas.as_ref() {
            let instance_buffer = device.create_raytracing_instance_buffer(&instances)?;
            pass.cmd_buf.update_raytracing_tlas(tlas, &instance_buffer, instances.len(), gfx::AccelerationStructureRebuildMode::Refit);
            t.instance_buffer = Some(instance_buffer);
        }
    }

    Ok(())
}

#[export_compute_fn]
pub fn render_meshes_raytraced(
    mut device: ResMut<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    pass: &pmfx::ComputePass<gfx_platform::Device>,
    mut entities_query: Query<(&mut Position, &mut Scale, &mut Rotation, &BLAS)>,
    mut tlas_query: Query<&mut TLAS>,
) -> Result<(), hotline_rs::Error> {
    let pmfx = &pmfx.0;

    let mut heap = pmfx.shader_heap.clone();

    let output_size = pmfx.get_texture_2d_size("staging_output").expect("expected staging_output");
    let output_tex = pmfx.get_texture("staging_output").expect("expected staging_output");

    let camera = pmfx.get_camera_constants("main_camera");
    if let Ok(camera) = camera {
        for t in &tlas_query {            
            if let Some(tlas) = &t.tlas {
                // set pipeline
                let raytracing_pipeline = pmfx.get_raytracing_pipeline(&pass.pass_pipline)?;
                pass.cmd_buf.set_raytracing_pipeline(&raytracing_pipeline.pipeline);

                let slot = raytracing_pipeline.pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
                if let Some(slot) = slot {
                    // camera constants
                    let inv = camera.view_projection_matrix.inverse();
                    pass.cmd_buf.push_compute_constants(slot.index, 16, 0, &inv);

                    // output uav
                    pass.cmd_buf.push_compute_constants(slot.index, 1, 16, gfx::as_u8_slice(&pass.use_indices[0].index));

                    // scene tlas
                    let srv0 =  tlas.get_srv_index().expect("expect tlas to have an srv");
                    pass.cmd_buf.push_compute_constants(slot.index, 1, 17, gfx::as_u8_slice(&srv0));

                    // point light info
                    let world_buffer_info = pmfx.get_world_buffer_info();
                    pass.cmd_buf.push_compute_constants(slot.index, 2, 18, gfx::as_u8_slice(&world_buffer_info.point_light));
                }

                pass.cmd_buf.set_heap(&raytracing_pipeline.pipeline, &pmfx.shader_heap);
                
                let second_slot = raytracing_pipeline.pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::ShaderResource);
                if let Some(second_slot) = second_slot {
                    let srv_buffer = t.instance_srv_buffer.get_srv_index().unwrap();
                    println!("bind {} on {}", srv_buffer, second_slot.index);
                    pass.cmd_buf.set_binding(&raytracing_pipeline.pipeline, &pmfx.shader_heap, second_slot.index, srv_buffer);
                }

                // dispatch
                pass.cmd_buf.dispatch_rays(&raytracing_pipeline.sbt, gfx::Size3 {
                    x: output_size.0 as u32,
                    y: output_size.1 as u32,
                    z: 1
                });
            }
        }
    }

    Ok(())
}