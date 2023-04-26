// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

/// Contains basic examples and unit tests of rendering and ecs functionality
mod examples;

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemConfig;

use crate::examples::*;

#[no_mangle]
pub fn batch_lights(
    mut pmfx: ResMut<PmfxRes>,
    light_query: Query<(&Position, &Colour, &LightComponent)>) {

    let world_buffers = pmfx.get_world_buffers_mut();
    world_buffers.point_light.clear();
    world_buffers.spot_light.clear();
    world_buffers.directional_light.clear();
    for (pos, colour, light) in &light_query {
        match light.light_type {
            LightType::Point => {
                world_buffers.point_light.push(&PointLightData{
                    pos: pos.0,
                    radius: light.radius,
                    colour: colour.0
                });
            },
            LightType::Spot => {
                world_buffers.spot_light.push(&SpotLightData{
                    pos: pos.0,
                    cutoff: light.cutoff,
                    dir: light.direction,
                    falloff: light.falloff,
                    colour: colour.0,
                });
            },
            LightType::Directional => {
                world_buffers.directional_light.push(&DirectionalLightData{
                    dir: Vec4f::from((light.direction, 0.0)),
                    colour: colour.0
                });
            }
        }
    }
}

/// Batch updates instance world matrices into the `InstanceBuffer`
#[no_mangle]
pub fn batch_world_matrix_instances(
    instances_query: Query<(&Parent, &WorldMatrix)>,
    mut instance_batch_query: Query<(Entity, &mut InstanceBuffer)>) {
    for (entity, mut instance_batch) in &mut instance_batch_query {
        let mut mats = Vec::new();
        for (parent, world_matrix) in &instances_query {
            if parent.0 == entity {
                mats.push(world_matrix.0);
            }
        }
        instance_batch.buffer.update(0, &mats).unwrap();
    }
}

/// Batch updates lookup id's into the instance buffer
#[no_mangle]
pub fn batch_material_instances(
    mut instances_query: Query<(&Parent, &InstanceIds)>,
    mut instance_batch_query: Query<(Entity, &mut InstanceBuffer)>) {
    for (entity, mut instance_batch) in &mut instance_batch_query {
        let mut indices = Vec::new();
        for (parent, ids) in &mut instances_query {
            if parent.0 == entity {
                indices.push(vec4u(ids.entity_id, ids.material_id, 0, 0));
            }
        }
        instance_batch.buffer.update(0, &indices).unwrap();
    }
}

const fn unit_aabb_corners() -> [Vec3f; 8] {
    [
        // front face
        Vec3f { x: 0.0, y: 0.0, z: 0.0 },
        Vec3f { x: 0.0, y: 1.0, z: 0.0 },
        Vec3f { x: 1.0, y: 1.0, z: 0.0 },
        Vec3f { x: 1.0, y: 0.0, z: 0.0 },

        // back face
        Vec3f { x: 0.0, y: 0.0,  z: 1.0 },
        Vec3f { x: 0.0, y: 1.0,  z: 1.0 },
        Vec3f { x: 1.0, y: 1.0,  z: 1.0 },
        Vec3f { x: 1.0, y: 0.0,  z: 1.0 },
    ]
}

/// Batches draw calls into a structured buffer which can be looked up into, and also batches extents
/// to perform GPU culling
#[no_mangle]
pub fn batch_bindless_draw_data(
    mut pmfx: ResMut<PmfxRes>,
    draw_query: Query<(&WorldMatrix, &Extents)>) {
    
    let world_buffers = pmfx.get_world_buffers_mut();    
    world_buffers.draw.clear();
    world_buffers.extent.clear();

    // for transforming the extents
    let corners = unit_aabb_corners();

    for (world_matrix, extents) in &draw_query {
        world_buffers.draw.push(&DrawData {
            world_matrix: world_matrix.0
        });

        let emin = extents.aabb_min;
        let emax = extents.aabb_max;
        
        let transform_min = corners.iter().fold( Vec3f::max_value(), |acc, x| min(acc, world_matrix.0 * (emin + (emax - emin) * *x)));
        let transform_max = corners.iter().fold(-Vec3f::max_value(), |acc, x| max(acc, world_matrix.0 * (emin + (emax - emin) * *x)));

        let extent_pos = transform_min + (transform_max - transform_min) * 0.5;
        let extent = transform_max - extent_pos;

        world_buffers.extent.push(&pmfx::ExtentData {
            pos: extent_pos,
            extent: extent
        });
    }
}

/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_bindless_material(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
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
    view.cmd_buf.set_render_pipeline(&pipeline);

    // bind view push constants
    let slot = pipeline.get_descriptor_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(slot.slot, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_render_constants(slot.slot, 4, 16, gfx::as_u8_slice(&camera.view_position));
    }

    // bind the world buffer info
    let world_buffer_info = pmfx.get_world_buffer_info();
    let slot = pipeline.get_descriptor_slot(2, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        // println!("{:?}", world_buffer_info);
        view.cmd_buf.push_render_constants(
            slot.slot, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

    // bind the shader resource heap
    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    // instance batch draw calls
    for (instance_batch, mesh) in &instance_draw_query {
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.set_vertex_buffer(&instance_batch.buffer, 1);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    // single draw calls
    for (mesh, world_matrix) in &single_draw_query {
        // set the world matrix push constants
        let slot = pipeline.get_descriptor_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.slot, 12, 0, &world_matrix.0);
        }
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///Renders all meshes generically with a single pipeline which and be specified in the .pmfx view
#[no_mangle]
pub fn render_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    queries: (
        Query<(&WorldMatrix, &MeshComponent), Without<Billboard>>,
        Query<(&WorldMatrix, &MeshComponent), (With<Billboard>, Without<CylindricalBillboard>)>,
        Query<(&WorldMatrix, &MeshComponent), With<CylindricalBillboard>>,
    )) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);

    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let (mesh_draw_query, billboard_draw_query, cylindrical_draw_query) = queries;

    for (world_matrix, mesh) in &mesh_draw_query {
        let slot = pipeline.get_descriptor_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.slot, 12, 0, &world_matrix.0);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    for (world_matrix, mesh) in &billboard_draw_query {
        let bbmat = world_matrix.0 * Mat4f::from(inv_rot);
        let slot = pipeline.get_descriptor_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.slot, 12, 0, &bbmat);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // spherical billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    let cyl_rot = Mat3f::new(
        inv_rot[0], 0.0, inv_rot[2],
        0.0, 1.0, 0.0,
        inv_rot[6], 0.0, inv_rot[8],
    );
    for (world_matrix, mesh) in &cylindrical_draw_query {
        let bbmat = world_matrix.0 * Mat4f::from(cyl_rot);
        let slot = pipeline.get_descriptor_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.slot, 12, 0, &bbmat);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///Renders all meshes generically with a single pipeline which and be specified in the .pmfx view
#[no_mangle]
pub fn render_debug(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    args: (
        ResMut<ImDrawRes>,
        ResMut<DeviceRes>,
        Res<SessionInfo>,
        Query<(&WorldMatrix, &Extents)>
    )
) -> Result<(), hotline_rs::Error> {
    let (mut imdraw, mut device, session_info, draw_query) = args;

    // skip over rendering if we supply no flags
    if session_info.debug_draw_flags.is_empty() {
        return Ok(());
    }
    
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format("imdraw_3d", fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;
    let bb = view.cmd_buf.get_backbuffer_index();

    // grid
    if session_info.debug_draw_flags.contains(DebugDrawFlags::GRID) {
        let scale = 1000.0;
        let divisions = 10.0;
        for i in 0..((scale * 2.0) /divisions) as usize {
            let offset = -scale + i as f32 * divisions;
            let mut tint = 0.3;
            if i % 5 == 0 {
                tint *= 0.5;
            }
            if i % 10 == 0 {
                tint *= 0.25;
            }
            if i % 20 == 0 {
                tint *= 0.125;
            }
    
            imdraw.add_line_3d(Vec3f::new(offset, 0.0, -scale), Vec3f::new(offset, 0.0, scale), Vec4f::from(tint));
            imdraw.add_line_3d(Vec3f::new(-scale, 0.0, offset), Vec3f::new(scale, 0.0, offset), Vec4f::from(tint));
        }
    }

    // aabb
    if session_info.debug_draw_flags.contains(DebugDrawFlags::AABB) {
        let corners = unit_aabb_corners();
        for (world_matrix, extents) in &draw_query {
            let emin = extents.aabb_min;
            let emax = extents.aabb_max;
            let (tmin, tmax) = corners.iter().fold((Vec3f::max_value(), -Vec3f::max_value()), |acc, x| min_max(world_matrix.0 * (emin + (emax - emin) * x), acc));
            imdraw.add_aabb_3d(tmin, tmax, Vec4f::white());
        }
    }

    // obb
    if session_info.debug_draw_flags.contains(DebugDrawFlags::OBB) {
        let corners = unit_aabb_corners();
        for (world_matrix, extents) in &draw_query {
            let emin = extents.aabb_min;
            let emax = extents.aabb_max;
            let obb = corners.iter().map(|x| world_matrix.0 * (emin + (emax - emin) * x)).collect::<Vec<Vec3f>>();
            imdraw.add_obb_3d(obb, Vec4f::green());
        }
    }
    
    // submit the buffers
    imdraw.submit(&mut device.0, bb as usize).unwrap();

    // draw
    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, &camera.view_projection_matrix);
    imdraw.draw_3d(&view.cmd_buf, bb as usize);

    Ok(())
}

/// Renders all scene meshes with a pipeline component, binding a new pipeline each draw
#[no_mangle]
pub fn render_meshes_pipeline(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &PipelineComponent)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (world_matrix, mesh, pipeline) in &mesh_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

#[no_mangle]
pub fn blit(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    _: Query<()>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format("imdraw_blit", fmt)?;

    if view.use_indices.len() != 1 {
        return Err(hotline_rs::Error {
            msg: "blit expects a single read resource specified in the `pmfx` uses".to_string()
        });
    }
    let srv = view.use_indices[0];

    view.cmd_buf.set_render_pipeline(pipeline);

    let slot = pipeline.get_descriptor_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(slot.slot, 2, 0, &view.blit_dimension);
    }

    // TODO_BINDING
    let slot = pipeline.get_descriptor_slot(1, 0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, srv as usize);
    }

    view.cmd_buf.set_index_buffer(&pmfx.0.unit_quad_mesh.ib);
    view.cmd_buf.set_vertex_buffer(&pmfx.0.unit_quad_mesh.vb, 0);
    view.cmd_buf.draw_indexed_instanced(6, 1, 0, 0, 0);

    Ok(())
}

/// Register demos / examples by name... this assuems a function exists of the same name
#[no_mangle]
pub fn get_demos_ecs_examples() -> Vec<String> {
    demos![
        // primitive entities
        "geometry_primitives",
        "point_lights",
        "spot_lights",
        "directional_lights",
        "tangent_space_normal_maps",

        // draw tests
        "draw",
        "draw_indexed",
        "draw_indexed_push_constants",
        "draw_indexed_vertex_buffer_instanced",
        "draw_indexed_cbuffer_instanced",
        "draw_push_constants_texture",
        "draw_material",
        "draw_indirect",
        "draw_indirect_gpu_frustum_culling",

        // render tests
        "test_raster_states",
        "test_blend_states",
        "test_cubemap",
        "test_texture2d_array",
        "test_texture3d",
        "test_compute",
        "test_multiple_render_targets",
        
        // basic tests
        "test_missing_demo",
        "test_missing_systems",
        "test_missing_render_graph",
        "test_missing_view",    
        "test_missing_pipeline",
        "test_failing_pipeline",
        "test_missing_camera"
    ]
}

/// Register plugin system functions
#[no_mangle]
pub fn get_system_ecs_examples(name: String, pass_name: String) -> Option<SystemConfig> {
    match name.as_str() {
        // primitive setup functions
        "setup_geometry_primitives" => system_func![setup_geometry_primitives],
        "setup_point_lights" => system_func![setup_point_lights],
        "setup_spot_lights" => system_func![setup_spot_lights],
        "setup_directional_lights" => system_func![setup_directional_lights],
        "setup_tangent_space_normal_maps" => system_func![setup_tangent_space_normal_maps],

        // draw tests
        "setup_draw" => system_func![setup_draw],
        "setup_draw_indexed" => system_func![setup_draw_indexed],
        "setup_draw_indexed_push_constants" => system_func![setup_draw_indexed_push_constants],
        "setup_draw_indexed_vertex_buffer_instanced" => system_func![setup_draw_indexed_vertex_buffer_instanced],
        "setup_draw_indexed_cbuffer_instanced" => system_func![setup_draw_indexed_cbuffer_instanced],
        "setup_draw_push_constants_texture" => system_func![setup_draw_push_constants_texture],
        "setup_draw_material" => system_func![setup_draw_material],
        "setup_draw_indirect" => system_func![setup_draw_indirect],
        "setup_draw_indirect_gpu_frustum_culling" => system_func![setup_draw_indirect_gpu_frustum_culling],

        // render state tests
        "setup_raster_test" => system_func![setup_raster_test],
        "setup_blend_test" => system_func![setup_blend_test],
        "setup_cubemap_test" => system_func![setup_cubemap_test],
        "setup_texture2d_array_test" => system_func![setup_texture2d_array_test],
        "setup_texture3d_test" => system_func![setup_texture3d_test],
        "setup_compute_test" => system_func![setup_compute_test],
        "setup_multiple_render_targets_test" => system_func![setup_multiple_render_targets_test],
        
        // updates
        "rotate_meshes" => system_func![rotate_meshes.in_base_set(SystemSets::Update)],
        "animate_textures" => system_func![animate_textures.in_base_set(SystemSets::Update)],
        "animate_lights" => system_func![animate_lights.in_base_set(SystemSets::Update)],
        "animate_lights2" => system_func![animate_lights2.in_base_set(SystemSets::Update)],
        "animate_lights3" => system_func![animate_lights3.in_base_set(SystemSets::Update)],
        "swirling_meshes" => system_func![swirling_meshes.in_base_set(SystemSets::Update)],

        // batches
        "batch_bindless_draw_data" => system_func![
            batch_bindless_draw_data.in_base_set(SystemSets::Batch)
        ],
        "batch_world_matrix_instances" => system_func![
            batch_world_matrix_instances.in_base_set(SystemSets::Batch)
        ],
        "batch_material_instances" => system_func![
            batch_material_instances.in_base_set(SystemSets::Batch)
        ],
        "batch_lights" => system_func![
            batch_lights.in_base_set(SystemSets::Batch)
        ],

        // render functions
        "render_debug" => render_func![
            render_debug, 
            pass_name,
            (
                ResMut<ImDrawRes>,
                ResMut<DeviceRes>,
                Res<SessionInfo>,
                Query<(&WorldMatrix, &Extents)>
            )
        ],
        "blit" => render_func![
            blit, 
            pass_name,
            Query<()>
        ],
        "render_meshes_bindless_material" => render_func![
            render_meshes_bindless_material, 
            pass_name,
            (
                Query<(&InstanceBuffer, &MeshComponent)>,
                Query<(&MeshComponent, &WorldMatrix), Without<InstanceBuffer>>
            )
        ],
        "draw_meshes" => render_func![
            draw_meshes, 
            pass_name,
            Query<(&WorldMatrix, &MeshComponent)>
        ],
        "draw_meshes_indirect" => render_func![
            draw_meshes_indirect, 
            pass_name,
            Query<(&WorldMatrix, &MeshComponent, &CommandSignatureComponent, &BufferComponent)>
        ],
        "draw_meshes_indirect_culling" => render_func![
            draw_meshes_indirect_culling, 
            pass_name,
            Query<&DrawIndirectComponent>
        ],
        "render_meshes" => render_func![
            render_meshes, 
            pass_name,
            (
                Query<(&WorldMatrix, &MeshComponent), Without<Billboard>>,
                Query<(&WorldMatrix, &MeshComponent), (With<Billboard>, Without<CylindricalBillboard>)>,
                Query<(&WorldMatrix, &MeshComponent), With<CylindricalBillboard>>,
            )
        ],
        "render_meshes_pipeline" => render_func![
            render_meshes_pipeline, 
            pass_name, 
            Query<(&WorldMatrix, &MeshComponent, &PipelineComponent)>
        ],
        "render_meshes_pipeline_coloured" => render_func![
            render_meshes_pipeline_coloured, 
            pass_name, 
            Query<(&WorldMatrix, &MeshComponent, &PipelineComponent, &Colour)>
        ],
        "render_meshes_vertex_buffer_instanced" => render_func![
            render_meshes_vertex_buffer_instanced, 
            pass_name, 
            Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
        ],
        "render_meshes_cbuffer_instanced" => render_func![
            render_meshes_cbuffer_instanced, 
            pass_name, 
            Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
        ],
        "render_meshes_push_constants_texture" => render_func![
            render_meshes_push_constants_texture, 
            pass_name, 
            Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>
        ],
        "render_meshes_debug_tangent_space" => render_func![
            render_meshes_debug_tangent_space,
            pass_name, 
            (
                Query<&TextureComponent>,
                Query<(&WorldMatrix, &MeshComponent)>
            )
        ],
        "render_meshes_cubemap_test" => render_func![
            render_meshes_cubemap_test,
            pass_name,
            Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>
        ],
        "render_meshes_texture2d_array_test" => render_func![
            render_meshes_texture2d_array_test,
            pass_name,
            Query<(&WorldMatrix, &MeshComponent, &TextureInstance, &AnimatedTexture), With<CylindricalBillboard>>
        ],
        "render_meshes_texture3d_test" => render_func![
            render_meshes_texture3d_test,
            pass_name,
            Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>
        ],
        "dispatch_compute" => compute_func!(pass_name),
        "dispatch_compute_frustum_cull" => compute_func_query![
            dispatch_compute_frustum_cull,
            pass_name,
            Query<&DrawIndirectComponent>
        ],
        
        // basic tests
        "render_missing_camera" => render_func![
            render_missing_camera, 
            pass_name,
            Query::<(&WorldMatrix, &MeshComponent)>
        ],
        "render_missing_pipeline" => render_func![
            render_missing_pipeline, 
            pass_name,
            Query::<(&WorldMatrix, &MeshComponent)>
        ],
        _ => std::hint::black_box(None)
    }
}