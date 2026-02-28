///
/// Bindless Material IBL
///
/// 10 PBR materials × 4 mesh types arranged in a curated 10×4 grid,
/// lit by image-based lighting (cubemap + BRDF LUT) instead of point lights.
///

use crate::prelude::*;

#[derive(Resource)]
pub struct IblData {
    cubemap_srv: u32,
    lut_srv: u32,
}

#[no_mangle]
pub fn bindless_material_ibl(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/util").as_str()).unwrap();
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_bindless_material_ibl"
        ],
        update: systems![
            "batch_material_instances",
            "batch_bindless_draw_data"
        ],
        render_graph: "mesh_instanced_bindless_material_ibl",
        ..Default::default()
    }
}

#[export_update_fn]
pub fn setup_bindless_material_ibl(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let mesh = hotline_rs::primitives::create_teapot_mesh(&mut device.0, 8);

    let materials = vec![
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/copper-scuffed"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/angled-tiled-floor"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/used-stainless-steel"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/office-carpet-fabric1"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/antique-grate1"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/green-ceramic-tiles"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/dirty-padded-leather"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/rusting-lined-metal2"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/worn-painted-cement"))?,
    ];

    // load IBL textures
    let cubemap_filepath = hotline_rs::get_data_path("textures/cubemap.dds");
    let cubemap = hotline_rs::image::load_texture_from_file(
        &mut device.0, &cubemap_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    let lut_filepath = hotline_rs::get_data_path("textures/luts/ibl_brdf_lut.dds");
    let lut = hotline_rs::image::load_texture_from_file(
        &mut device.0, &lut_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    let cubemap_srv = cubemap.get_srv_index().unwrap() as u32;
    let lut_srv = lut.get_srv_index().unwrap() as u32;

    commands.spawn(TextureComponent(cubemap));
    commands.spawn(TextureComponent(lut));

    // grid layout: 3x3 columns
    let num_materials = materials.len();
    let spacing = 40.0_f32;
    let size = 10.0_f32;

    let mut entity_itr: u32 = 0;

    let instance_count = num_materials as u32;
    let parent = commands.spawn(InstanceBatch {
        mesh: MeshComponent(mesh.clone()),
        pipeline: PipelineComponent(String::new()),
        instance_buffer: InstanceBuffer {
            buffer: device.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::VERTEX,
                cpu_access: gfx::CpuAccessFlags::WRITE,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vec4u>(),
                num_elements: instance_count as usize,
                initial_state: gfx::ResourceState::VertexConstantBuffer,
            }, hotline_rs::data![]).unwrap(),
            instance_count,
            heap: None,
        },
    }).id();

    let num_cols = (num_materials as f32).sqrt().round() as usize;
    let num_rows = (num_materials + num_cols - 1) / num_cols;

    let cell = spacing;
    let x_offset = (num_cols as f32 - 1.0) * cell * 0.5;
    let z_offset = (num_rows as f32 - 1.0) * cell * 0.5;

    for i in 0..num_materials {

        let col = i % num_cols;
        let row = i / num_cols;

        let x = -x_offset + col as f32 * cell;
        let z = -z_offset + row as f32 * cell;
        
        let pos = vec3f(x, 0.0, z);

        commands.spawn((
            Instance {
                pos: Position(pos),
                rot: Rotation(Quatf::identity()),
                scale: Scale(splat3f(size)),
                world_matrix: WorldMatrix(Mat34f::identity()),
                parent: Parent(parent),
            },
            InstanceIds {
                entity_id: entity_itr,
                material_id: i as u32,
            },
            Extents {
                aabb_min: mesh.aabb_min,
                aabb_max: mesh.aabb_max,
            },
        ));

        entity_itr += 1;
    }

    // write material data to world buffers
    let mut material_data = Vec::new();
    for material in &materials {
        material_data.push(MaterialData {
            albedo_id: material.albedo.get_srv_index().unwrap() as u32,
            normal_id: material.normal.get_srv_index().unwrap() as u32,
            roughness_id: material.roughness.get_srv_index().unwrap() as u32,
            padding: 0,
        });
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_itr as usize,
        extent_capacity: entity_itr as usize,
        material_capacity: materials.len(),
        ..Default::default()
    });

    pmfx.get_world_buffers_mut().material.write(0, &material_data);

    for material in materials {
        commands.spawn(material);
    }

    commands.insert_resource(IblData { cubemap_srv, lut_srv });

    Ok(())
}

#[export_render_fn]
pub fn render_meshes_bindless_ibl(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    ibl_data: &Res<IblData>,
    queries: (
        Query<(&InstanceBuffer, &MeshComponent)>,
        Query<(&MeshComponent, &WorldMatrix), Without<InstanceBuffer>>
    )
) -> Result<(), hotline_rs::Error> {

    let (instance_draw_query, single_draw_query) = queries;

    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    cmd_buf.set_render_pipeline(&pipeline);

    // bind view push constants
    let slot = pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        cmd_buf.push_render_constants(slot.index, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        cmd_buf.push_render_constants(slot.index, 4, 16, gfx::as_u8_slice(&camera.view_position));
    }

    // bind world buffer info with IBL indices in user_data
    let mut world_buffer_info = pmfx.get_world_buffer_info();
    world_buffer_info.user_data[0] = ibl_data.cubemap_srv;
    world_buffer_info.user_data[1] = ibl_data.lut_srv;
    let slot = pipeline.get_pipeline_slot(2, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        cmd_buf.push_render_constants(
            slot.index, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

    // bind the shader resource heap
    cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    // instance batch draw calls
    for (instance_batch, mesh) in &instance_draw_query {
        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.set_vertex_buffer(&instance_batch.buffer, 1);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    // single draw calls
    for (mesh, world_matrix) in &single_draw_query {
        let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            cmd_buf.push_render_constants(slot.index, 12, 0, &world_matrix.0);
        }
        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}
