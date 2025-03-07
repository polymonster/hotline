// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

/// Contains basic examples and unit tests of rendering and ecs functionality
mod error_tests;
mod draw;
mod draw_indexed;
mod draw_push_constants;
mod draw_indirect;
mod geometry_primitives;
mod draw_vertex_buffer_instanced;
mod draw_cbuffer_instanced;
mod bindless_texture;
mod tangent_space_normal_maps;
mod bindless_material;
mod point_lights;
mod spot_lights;
mod directional_lights;
mod gpu_frustum_culling;
mod cubemap;
mod texture2d_array;
mod texture3d;
mod read_write_texture;
mod multiple_render_targets;
mod raster_states;
mod blend_states;
mod generate_mip_maps;
mod shadow_map;
mod omni_shadow_map;
mod dynamic_cubemap;
mod pbr;
mod raytraced_shadows;

use prelude::*;

pub fn load_material(
    device: &mut gfx_platform::Device,
    pmfx: &mut Pmfx<gfx_platform::Device>,
    dir: &str) -> Result<MaterialResources, hotline_rs::Error> {
    let maps = vec![
        "_albedo.dds",
        "_normal.dds",
        "_roughness.dds"
    ];

    let mut textures = Vec::new();
    for map in maps {
        let paths = std::fs::read_dir(dir).unwrap();
        let map_path = paths.into_iter()
            .filter(|p| p.as_ref().unwrap().file_name().to_string_lossy().ends_with(map))
            .map(|p| String::from(p.as_ref().unwrap().file_name().to_string_lossy()))
            .collect::<Vec<_>>();

        if !map_path.is_empty() {
            textures.push(
                image::load_texture_from_file(
                    device, 
                    &format!("{}/{}", dir, map_path[0]), 
                    Some(&mut pmfx.shader_heap)
                ).unwrap()
            );
        }
    }

    if textures.len() != 3 {
        return Err(hotline_rs::Error {
            msg: format!(
                "hotline_rs::ecs:: error: material '{}' does not contain enough maps ({}/3)", 
                dir,
                textures.len()
            )
        });
    }

    Ok(MaterialResources {
        albedo: textures.remove(0),
        normal: textures.remove(0),
        roughness: textures.remove(0)
    })
}

#[no_mangle]
#[export_update_fn(in_set(SystemSets::Batch))]
pub fn batch_lights(
    mut pmfx: ResMut<PmfxRes>,
    light_query: Query<(&Position, &Colour, &LightComponent)>) -> Result<(), hotline_rs::Error> {

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
                    colour: colour.0,
                    shadow_map_info: light.shadow_map_info
                });
            },
            LightType::Spot => {
                world_buffers.spot_light.push(&SpotLightData{
                    pos: pos.0,
                    cutoff: light.cutoff,
                    dir: light.direction,
                    falloff: light.falloff,
                    colour: colour.0,
                    shadow_map_info: light.shadow_map_info
                });
            },
            LightType::Directional => {
                world_buffers.directional_light.push(&DirectionalLightData{
                    dir: light.direction,
                    colour: colour.0,
                    shadow_map_info: light.shadow_map_info
                });
            }
        }
    }

    Ok(())
}

/// Batch updates instance world matrices into the `InstanceBuffer`
#[no_mangle]
#[export_update_fn(in_set(SystemSets::Batch))]
pub fn batch_world_matrix_instances(
    instances_query: Query<(&Parent, &WorldMatrix)>,
    mut instance_batch_query: Query<(Entity, &mut InstanceBuffer)>) -> Result<(), hotline_rs::Error> {
    for (entity, mut instance_batch) in &mut instance_batch_query {
        let mut mats = Vec::new();
        for (parent, world_matrix) in &instances_query {
            if parent.0 == entity {
                mats.push(world_matrix.0);
            }
        }
        instance_batch.buffer.update(0, &mats).unwrap();
    }

    Ok(())
}

/// Batch updates lookup id's into the instance buffer
#[no_mangle]
#[export_update_fn(in_set(SystemSets::Batch))]
pub fn batch_material_instances(
    mut instances_query: Query<(&Parent, &InstanceIds)>,
    mut instance_batch_query: Query<(Entity, &mut InstanceBuffer)>) -> Result<(), hotline_rs::Error> {
    for (entity, mut instance_batch) in &mut instance_batch_query {
        let mut indices = Vec::new();
        for (parent, ids) in &mut instances_query {
            if parent.0 == entity {
                indices.push(vec4u(ids.entity_id, ids.material_id, 0, 0));
            }
        }
        instance_batch.buffer.update(0, &indices).unwrap();
    }

    Ok(())
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
#[export_update_fn(in_set(SystemSets::Batch))]
pub fn batch_bindless_draw_data(
    mut pmfx: ResMut<PmfxRes>,
    draw_query: Query<(&WorldMatrix, &Extents)>) -> Result<(), hotline_rs::Error> {
    
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

    Ok(())
}

/// Renders all meshes, either instanced or single calls providing bindless lookup info
#[no_mangle]
#[export_render_fn]
pub fn render_meshes_bindless(
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
    let slot = pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(slot.index, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_render_constants(slot.index, 4, 16, gfx::as_u8_slice(&camera.view_position));
    }

    // bind the world buffer info
    let world_buffer_info = pmfx.get_world_buffer_info();
    let slot = pipeline.get_pipeline_slot(2, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(
            slot.index, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

    // bind resource uses
    let using_slot = pipeline.get_pipeline_slot(0, 1, gfx::DescriptorType::PushConstants);
    if let Some(slot) = using_slot {
        for i in 0..view.use_indices.len() {
            let num_constants = gfx::num_32bit_constants(&view.use_indices[i]);
            view.cmd_buf.push_compute_constants(
                0, 
                num_constants, 
                i as u32 * num_constants, 
                gfx::as_u8_slice(&view.use_indices[i])
            );
        }
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
        let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.index, 12, 0, &world_matrix.0);
        }
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///Renders all meshes generically with a single pipeline which and be specified in the .pmfx view
#[no_mangle]
#[export_render_fn]
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
        let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.index, 12, 0, &world_matrix.0);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    for (world_matrix, mesh) in &billboard_draw_query {
        let bbmat = world_matrix.0 * Mat4f::from(inv_rot);
        let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.index, 12, 0, &bbmat);
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
        let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_render_constants(slot.index, 12, 0, &bbmat);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Renders all meshes generically with a single pipeline which and be specified in the .pmfx view
#[no_mangle]
#[export_render_fn]
pub fn render_debug(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mut imdraw: ResMut<ImDrawRes>,
    mut device: ResMut<DeviceRes>,
    session_info: ResMut<SessionInfo>,
    draw_query: Query<(&WorldMatrix, &Extents)>,
    camera_query: Query<(&Name, &Camera)>
) -> Result<(), hotline_rs::Error> {

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

    // cameras
    if session_info.debug_draw_flags.contains(DebugDrawFlags::CAMERAS) {
        for (name, camera) in &camera_query {
            let constants = pmfx.get_camera_constants(name)?;
            imdraw.add_frustum(constants.view_projection_matrix, Vec4f::white());
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

/// Blit a single fullscreen texture into the render target
#[no_mangle]
#[export_render_fn]
pub fn blit(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format("imdraw_blit", fmt)?;

    if view.use_indices.len() != 1 {
        return Err(hotline_rs::Error {
            msg: "blit expects a single read resource specified in the `pmfx` uses".to_string()
        });
    }
    let srv = view.use_indices[0].index;

    view.cmd_buf.set_render_pipeline(pipeline);

    let slot = pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(slot.index, 2, 0, &view.blit_dimension);
    }

    let slot = pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_binding(pipeline, &pmfx.shader_heap, slot.index, srv as usize);
    }

    view.cmd_buf.set_index_buffer(&pmfx.0.unit_quad_mesh.ib);
    view.cmd_buf.set_vertex_buffer(&pmfx.0.unit_quad_mesh.vb, 0);
    view.cmd_buf.draw_indexed_instanced(6, 1, 0, 0, 0);

    Ok(())
}

/// Blit a single fullscreen texture into the render target
#[no_mangle]
#[export_render_fn]
pub fn cubemap_clear(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format("cubemap_clear", fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    if view.use_indices.len() != 1 {
        return Err(hotline_rs::Error {
            msg: "blit expects a single read resource specified in the `pmfx` uses".to_string()
        });
    }
    let srv = view.use_indices[0].index;

    view.cmd_buf.set_render_pipeline(pipeline);

    let slot = pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        let inv = camera.view_projection_matrix.inverse();
        view.cmd_buf.push_render_constants(slot.index, 16, 0, &inv);
    }

    let slot = pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_binding(pipeline, &pmfx.shader_heap, slot.index, srv as usize);
    }

    view.cmd_buf.set_index_buffer(&pmfx.0.unit_quad_mesh.ib);
    view.cmd_buf.set_vertex_buffer(&pmfx.0.unit_quad_mesh.vb, 0);
    view.cmd_buf.draw_indexed_instanced(6, 1, 0, 0, 0);

    Ok(())
}

/// Generic compute dispatch which binds usage information supplied in pmfx files
#[no_mangle]
#[export_compute_fn]
pub fn dispatch_compute(
    pmfx: &Res<PmfxRes>,
    pass: &pmfx::ComputePass<gfx_platform::Device>
) -> Result<(), hotline_rs::Error> {

    let pipeline = pmfx.get_compute_pipeline(&pass.pass_pipline)?;
    pass.cmd_buf.set_compute_pipeline(&pipeline);

    let using_slot = pipeline.get_pipeline_slot(0, 1, gfx::DescriptorType::PushConstants);
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

    pass.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);
    
    pass.cmd_buf.dispatch(
        pass.group_count,
        pass.numthreads
    );

    Ok(())
}

/// Register demos / examples by name... this assumes a function exists of the same name
#[no_mangle]
pub fn get_demos_ecs_examples() -> Vec<String> {
    demos![
        "draw",
        "draw_indexed",
        "draw_push_constants",
        "draw_indirect",
        "geometry_primitives",
        "draw_vertex_buffer_instanced",
        "draw_cbuffer_instanced",
        "bindless_texture",
        "tangent_space_normal_maps",
        "bindless_material",
        "point_lights",
        "spot_lights",
        "directional_lights",
        "gpu_frustum_culling",
        "cubemap",
        "texture2d_array",
        "texture3d",
        "read_write_texture",
        "multiple_render_targets",
        "raster_states",
        "blend_states",
        "generate_mip_maps",
        "shadow_map",
        "dynamic_cubemap",
        "omni_shadow_map",
        "pbr",
        "raytraced_shadows"
    ]
}

pub mod prelude {
    #[doc(hidden)]
    pub use hotline_rs::prelude::*;
    pub use maths_rs::prelude::*;
    pub use rand::prelude::*;
    pub use bevy_ecs::prelude::*;
    pub use bevy_ecs::schedule::SystemConfig;
    pub use bevy_ecs::schedule::SystemConfigs;
    pub use export_macros;
    pub use export_macros::export_update_fn;
    pub use export_macros::export_render_fn;
    pub use export_macros::export_compute_fn;
    pub use crate::load_material;
    pub use hotline_rs::pmfx::ShadowMapInfo;
}