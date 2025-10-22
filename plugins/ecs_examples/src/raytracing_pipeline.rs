// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::gfx::{CpuAccessFlags, RaytracingTLAS};

///
/// Raytracing Pipeline
///

#[repr(C)]
pub struct GeometryLookup {
    pub ib_srv: u32,
    pub vb_srv: u32,
    pub ib_stride: u32,
    pub material_type: u32
}

use crate::prelude::*;

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn raytracing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_raytracing_pipeline_scene"
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

pub fn geometry_lookup_from_mesh(device: &mut gfx_platform::Device, heap: &mut gfx_platform::Heap, mesh: &pmfx::Mesh<gfx_platform::Device>, material_type: u32) -> Result<GeometryLookup, hotline_rs::Error> {
    Ok(GeometryLookup {
        ib_srv: device.create_resource_view(&gfx::ResourceViewInfo {
            view_type: gfx::ResourceView::ShaderResource,
            format: gfx::Format::Unknown,
            first_element: 0,
            structure_byte_size: mesh.index_size_bytes as usize,
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
        heap)? as u32,
        ib_stride: mesh.index_size_bytes,
        material_type
    })
}

#[export_update_fn]
pub fn setup_raytracing_pipeline_scene(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 32);
    let teapot_mesh = hotline_rs::primitives::create_teapot_mesh(&mut device.0, 32);
    
    let bounds = 100.0;
    let shape_bounds = bounds * 0.6;
    let shape_size = bounds * 0.1;

    let mut instance_geometry_lookup = Vec::new();

    let dodeca_blas = commands.spawn((
        Position(vec3f(shape_bounds * -0.75, shape_bounds * 0.7, -shape_bounds * 0.1)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(sphere_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &sphere_mesh)?
    )).id();
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &sphere_mesh, 1)?);

    commands.spawn((
        Position(vec3f(shape_bounds * -0.3, shape_bounds * -0.6, shape_bounds * 0.8)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(teapot_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &teapot_mesh)?
    ));
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &teapot_mesh, 1)?);    

    commands.spawn((
        Position(vec3f(shape_bounds * 1.0, shape_bounds * 0.1, shape_bounds * -1.0)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(teapot_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &teapot_mesh)?
    ));
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &teapot_mesh, 2)?);

    commands.spawn((
        Position(vec3f(shape_bounds * 0.123, shape_bounds * -0.6, shape_bounds * -0.8)),
        Scale(splat3f(shape_size * 2.0)),
        Rotation(Quatf::identity()),
        MeshComponent(sphere_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        blas_from_mesh(&mut device, &sphere_mesh)?
    ));
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &sphere_mesh, 2)?);

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
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &cube_mesh, 0)?);

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
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &cube_mesh, 0)?);

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
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &cube_mesh, 0)?);

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
    instance_geometry_lookup.push(geometry_lookup_from_mesh(&mut device, &mut pmfx.shader_heap, &cube_mesh, 0)?);

    // create a buffer for srv indices
    let instance_geometry_buffer = device.create_buffer_with_heap::<u8>(&gfx::BufferInfo{
            usage: gfx::BufferUsage::SHADER_RESOURCE,
            cpu_access: CpuAccessFlags::NONE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<GeometryLookup>(),
            num_elements: instance_geometry_lookup.len(),
            initial_state: gfx::ResourceState::ShaderResource
        }, 
        Some(gfx::slice_as_u8_slice(instance_geometry_lookup.as_slice())),
        &mut pmfx.shader_heap
    )?;

    commands.spawn(
        TLASComponent {
            tlas: None,
            instance_buffer: None,
            instance_buffer_len: 0,
            instance_geometry_buffer: Some(instance_geometry_buffer)
        }
    );
    
    Ok(())
}

#[export_compute_fn]
pub fn render_meshes_raytraced(
    device: ResMut<DeviceRes>,
    pmfx: &Res<PmfxRes>,
    pass: &pmfx::ComputePass<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    tlas_query: Query<&mut TLASComponent>,
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
                cmd_buf.set_raytracing_pipeline(&raytracing_pipeline.pipeline);

                let slot = raytracing_pipeline.pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
                if let Some(slot) = slot {
                    // camera constants
                    let inv = camera.view_projection_matrix.inverse();
                    cmd_buf.push_compute_constants(slot.index, 16, 0, &inv);

                    // output uav
                    cmd_buf.push_compute_constants(slot.index, 1, 16, gfx::as_u8_slice(&pass.use_indices[0].index));

                    // scene tlas
                    let srv0 =  tlas.get_srv_index().expect("expect tlas to have an srv");
                    cmd_buf.push_compute_constants(slot.index, 1, 17, gfx::as_u8_slice(&srv0));

                    // point light info
                    let world_buffer_info = pmfx.get_world_buffer_info();
                    cmd_buf.push_compute_constants(slot.index, 2, 18, gfx::as_u8_slice(&world_buffer_info.point_light));
                }

                cmd_buf.set_heap(&raytracing_pipeline.pipeline, &pmfx.shader_heap);
                
                let second_slot = raytracing_pipeline.pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::ShaderResource);
                if let Some(second_slot) = second_slot {
                    let srv_buffer = t.instance_geometry_buffer.as_ref().unwrap().get_srv_index().unwrap();
                    cmd_buf.set_binding(&raytracing_pipeline.pipeline, &pmfx.shader_heap, second_slot.index, srv_buffer);
                }

                // dispatch
                cmd_buf.dispatch_rays(&raytracing_pipeline.sbt, gfx::Size3 {
                    x: output_size.0 as u32,
                    y: output_size.1 as u32,
                    z: 1
                });
            }
        }
    }

    Ok(())
}