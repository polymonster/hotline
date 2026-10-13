///
/// glTF Model
///

use crate::prelude::*;
use crate::bindless_material_ibl::{IblData, load_ibl_data};

/// Index into `GltfModel::model.materials` for a submesh entity
#[derive(Component)]
pub struct GltfMaterial(u32);

/// Keeps the model's gpu resources alive and holds the structured buffer of `PbrMaterialData`
#[derive(Resource)]
pub struct GltfModel {
    pub model: model::Model<gfx_platform::Device>,
    pub materials: gfx_platform::Buffer
}

/// Loads a glTF chess set with a node hierarchy, a submesh per piece, and pbr materials with bindless textures.
/// Each submesh spawns an entity with its node's world matrix and draws lit with image based lighting
#[no_mangle]
pub fn gltf_model(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/util").as_str()).unwrap();
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_gltf_model"
        ],
        render_graph: "mesh_gltf_pbr",
        ..Default::default()
    }
}

#[export_update_fn]
pub fn setup_gltf_model(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let data = model::load_gltf(
        &hotline_rs::get_data_path("models/ABeautifulGame.glb"), &model::GltfLoadInfo::default())?;
    let model = model::Model::create(&mut device.0, &data, Some(&mut pmfx.shader_heap))?;

    let materials = device.create_buffer_with_heap(&gfx::BufferInfo {
        usage: gfx::BufferUsage::SHADER_RESOURCE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<model::PbrMaterialData>(),
        num_elements: model.materials.len(),
        initial_state: gfx::ResourceState::ShaderResource
    }, hotline_rs::data![model.materials.as_slice()], &mut pmfx.shader_heap)?;

    // the chess board is 0.7m wide, scale it up to match the other examples
    let root = Mat34f::from_scale(splat3f(150.0));
    let nodes = &model.nodes;
    for node in 0..nodes.len() {
        let first = nodes.submesh_first[node] as usize;
        for submesh in first..first + nodes.submesh_count[node] as usize {
            commands.spawn((
                MeshComponent(model.meshes[submesh].clone()),
                WorldMatrix(root * nodes.world_matrix[node]),
                GltfMaterial(model.submesh_material[submesh])
            ));
        }
    }

    let ibl_data = load_ibl_data(&mut device, &mut pmfx, &mut commands)?;
    commands.insert_resource(ibl_data);
    commands.insert_resource(GltfModel { model, materials });

    Ok(())
}

/// Draws each submesh entity, pushing its world matrix and material index (`draw_indices.x`)
#[export_render_fn]
pub fn render_meshes_gltf(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    cmd_buf: &mut <gfx_platform::Device as Device>::CmdBuf,
    ibl_data: &Res<IblData>,
    gltf_model: &Res<GltfModel>,
    draw_query: Query<(&MeshComponent, &WorldMatrix, &GltfMaterial)>
) -> Result<(), hotline_rs::Error> {

    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    cmd_buf.set_render_pipeline(pipeline);

    cmd_buf.push_render_constants(pipeline, 0, 0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    cmd_buf.push_render_constants(pipeline, 0, 0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    // ibl textures and the material buffer go in user_data
    let mut world_buffer_info = pmfx.get_world_buffer_info();
    world_buffer_info.user_data[0] = ibl_data.cubemap_srv;
    world_buffer_info.user_data[1] = ibl_data.lut_srv;
    world_buffer_info.user_data[2] = gltf_model.materials.get_srv_index().unwrap() as u32;
    cmd_buf.push_render_constants(
        pipeline, 2, 0, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));

    cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    for (mesh, world_matrix, material) in &draw_query {
        cmd_buf.push_render_constants(pipeline, 1, 0, 12, 0, &world_matrix.0);
        cmd_buf.push_render_constants(pipeline, 1, 0, 1, 16, &[material.0]);
        cmd_buf.set_index_buffer(&mesh.0.ib);
        cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}
