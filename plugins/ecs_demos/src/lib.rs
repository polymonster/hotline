// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemConfig;

use hotline_rs::gfx::Buffer;

mod primitives;
mod test;
mod dev;
mod draw;

use crate::draw::*;
use crate::primitives::*;
use crate::test::*;

#[no_mangle]
fn rotate_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<&mut Rotation, Without<Billboard>>) {
    for mut rotation in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
    }
}

#[no_mangle]
fn animate_textures(
    time: Res<TimeRes>,
    mut animated_texture_query: Query<(&mut AnimatedTexture, &mut TimeComponent)>) {
    let frame_length = 1.0 / 24.0;
    for (mut animated_texture, mut timer) in &mut animated_texture_query {
        timer.0 += time.0.delta;
        if timer.0 > frame_length {
            timer.0 = 0.0;
            animated_texture.frame = (animated_texture.frame + 1) % animated_texture.frame_count;
        }
    }
}

fn vec_rotate(v: Vec2f, angle: f32) -> Vec2f {
    let c = cos(angle);
    let s = sin(angle);
    return vec2f(c * v.x - s * v.y, s * v.x + c * v.y); // anti-clockwise rotation
}

#[no_mangle]
fn animate_lights(
    time: Res<TimeRes>, 
    mut light_query: Query<&mut Position, With<LightType>>) {
    
    let t = time.accumulated;
    let r = sin(t);
    let tau = 6.28318507;

    let rot0 = sin(t);
    let rot1 = sin(-t);
    let rot2 = sin(t * 0.5);
    let rot3 = sin(-t * 0.5);
    
    let step = 1.0 / 16.0;
    let mut f = 0.0;
    let mut i = 0;
    for mut position in &mut light_query {
        if i < 16 {
            position.x = r * cos(tau * f) * 1000.0;
            position.z = r * sin(tau * f) * 1000.0;
            let pr = vec_rotate(position.xz(), rot0);
            position.set_xz(pr);
            f += step;
        }
        else if i < 32 {
            position.x = (r + 1.0) * cos(tau * f) * 1000.0;
            position.z = (r + 1.0) * sin(tau * f) * 1000.0;
            let pr = vec_rotate(position.xz(), rot2);
            position.set_xz(pr);
            f += step;
        }
        else if i < 48 {
            position.x = (r - 1.0) * cos(tau * f) * 1000.0;
            position.z = (r - 1.0) * sin(tau * f) * 1000.0;
            let pr = vec_rotate(position.xz(), rot3);
            position.set_xz(pr);
            f += step;
        }
        else if i < 64 {
            position.x = r * 2.0 * cos(tau * f) * 1000.0;
            position.z = r * 2.0 * sin(tau * f) * 1000.0;
            let pr = vec_rotate(position.xz(), rot1);
            position.set_xz(pr);
            f += step;
        }
        i += 1;
    }
}


/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_bindless_material(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    queries: (
        Query<(&draw::InstanceBuffer, &MeshComponent)>,
        Query<(&MeshComponent, &WorldMatrix), Without<draw::InstanceBuffer>>,
        Query<&draw::WorldBuffers>
    )
) -> Result<(), hotline_rs::Error> {
    
    let (instance_draw_query, single_draw_query, world_buffers_query) = queries;

    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    view.cmd_buf.push_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    // bind the shader resource heap for t0 (if exists)
    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // bind the shader resource heap for t1 (if exists)
    let slot = pipeline.get_heap_slot(1, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // set the world buffer ids in push constants
    let mut set_world_buffers = false;
    for world_buffers in &world_buffers_query {
        let buffer_ids = vec4u(
            world_buffers.draw.get_srv_index().unwrap() as u32,
            world_buffers.material.get_srv_index().unwrap() as u32, 
            world_buffers.light.get_srv_index().unwrap() as u32,
            0
        );

        // set the buffer ids push constants
        let slot = pipeline.get_heap_slot(2, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_constants(slot.slot, 4, 0, &buffer_ids);
        }

        set_world_buffers = true;
        break;
    }

    if !set_world_buffers {
        return Err(hotline_rs::Error {
            msg: "hotline_rs::ecs:: world buffers not set!".to_string()
        });
    }

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
        let slot = pipeline.get_heap_slot(1, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            view.cmd_buf.push_constants(slot.slot, 12, 0, &world_matrix.0);
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

    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let (mesh_draw_query, billboard_draw_query, cylindrical_draw_query) = queries;

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    for (world_matrix, mesh) in &billboard_draw_query {
        let bbmat = world_matrix.0 * Mat4f::from(inv_rot);
        view.cmd_buf.push_constants(1, 12, 0, &bbmat);
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
        view.cmd_buf.push_constants(1, 12, 0, &bbmat);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

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
        view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene meshes with a pipeline component, binding a new pipeline each draw with matrix + colour push constants
#[no_mangle]
pub fn render_meshes_pipeline_coloured(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &PipelineComponent, &Colour)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (world_matrix, mesh, pipeline, colour) in &mesh_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_constants(1, 4, 12, &colour.0);

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene meshes with a material instance component, using push constants to push texture ids
#[no_mangle]
pub fn render_meshes_push_constants_texture(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    for (world_matrix, mesh, texture) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_constants(1, 1, 16, gfx::as_u8_slice(&texture.0));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_vertex_buffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&draw::InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        
        // bind the shader resource heap for t0 (if exists)
        let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
        if let Some(slot) = slot {
            view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.set_vertex_buffer(&instance_batch.buffer, 1);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene instance batches with cbuffer instance buffer
#[no_mangle]
pub fn render_meshes_cbuffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&draw::InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

        view.cmd_buf.set_render_heap(1, instance_batch.heap.as_ref().unwrap(), 0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);

        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// Renders a texture2d test passing the texture index and frame index to the shader for sampling along with a world matrix.
#[no_mangle]
pub fn render_meshes_texture2d_array_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance, &AnimatedTexture), With<CylindricalBillboard>>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // spherical billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    let cyl_rot = Mat3f::new(
        inv_rot[0], 0.0, inv_rot[2],
        0.0, 1.0, 0.0,
        inv_rot[6], 0.0, inv_rot[8],
    );

    for (world_matrix, mesh, texture, animated_texture) in &mesh_query {
        let bbmat = world_matrix.0 * Mat4f::from(cyl_rot);
        view.cmd_buf.push_constants(1, 12, 0, &bbmat);
        view.cmd_buf.push_constants(1, 2, 16, gfx::as_u8_slice(&[texture.0, animated_texture.frame, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Renders all scene meshes with a cubemap applied and samples the separate mip levels in the shader per entity
#[no_mangle]
pub fn render_meshes_cubemap_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    let mut mip = 0;
    for (world_matrix, mesh, cubemap) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_constants(1, 2, 16, gfx::as_u8_slice(&[cubemap.0, mip, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);

        mip += 1;
    }

    Ok(())
}

/// Renders a texture3d test from a loaded (pre-generated signed distance field), the shader ray marches the volume
#[no_mangle]
pub fn render_meshes_texture3d_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    view.cmd_buf.push_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    for (world_matrix, mesh, tex) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_constants(1, 2, 16, gfx::as_u8_slice(&[tex.0, 0, 0, 0]));
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Register demos
#[no_mangle]
pub fn get_demos_ecs_demos() -> Vec<String> {
    demos![
        // primitive examples
        "geometry_primitives",
        "light_primitives",

        // draw tests
        "draw_indexed",
        "draw_indexed_push_constants",
        "draw_indexed_vertex_buffer_instanced",
        "draw_indexed_cbuffer_instanced",
        "draw_push_constants_texture",
        "draw_material",

        // render tests
        "test_raster_states",
        "test_blend_states",
        "test_cubemap",
        "test_texture2d_array",
        "test_texture3d",
        "test_compute",

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
pub fn get_system_ecs_demos(name: String, pass_name: String) -> Option<SystemConfig> {
    match name.as_str() {
        // primitive setup functions
        "setup_geometry_primitives" => system_func![setup_geometry_primitives],
        "setup_light_primitives" => system_func![setup_light_primitives],

        // draw tests
        "setup_draw_indexed" => system_func![setup_draw_indexed],
        "setup_draw_indexed_push_constants" => system_func![setup_draw_indexed_push_constants],
        "setup_draw_indexed_vertex_buffer_instanced" => system_func![setup_draw_indexed_vertex_buffer_instanced],
        "setup_draw_indexed_cbuffer_instanced" => system_func![setup_draw_indexed_cbuffer_instanced],
        "setup_draw_push_constants_texture" => system_func![setup_draw_push_constants_texture],
        "setup_draw_material" => system_func![setup_draw_material],

        // render state tests
        "setup_raster_test" => system_func![setup_raster_test],
        "setup_blend_test" => system_func![setup_blend_test],
        "setup_cubemap_test" => system_func![setup_cubemap_test],
        "setup_texture2d_array_test" => system_func![setup_texture2d_array_test],
        "setup_texture3d_test" => system_func![setup_texture3d_test],
        "setup_compute_test" => system_func![setup_compute_test],
        
        // updates
        "rotate_meshes" => system_func![
            rotate_meshes.in_base_set(SystemSets::Update)
        ],
        "animate_textures" => system_func![
            animate_textures.in_base_set(SystemSets::Update)
        ],
        "animate_lights" => system_func![
            animate_lights.in_base_set(SystemSets::Update)
        ],

        // batches
        "batch_world_matrix_instances" => system_func![
            draw::batch_world_matrix_instances.after(SystemSets::Batch)
        ],
        "batch_material_instances" => system_func![
            draw::batch_material_instances.after(SystemSets::Batch)
        ],
        "batch_bindless_world_matrix_instances" => system_func![
            draw::batch_bindless_world_matrix_instances.after(SystemSets::Batch)
        ],
        "batch_lights" => system_func![
            primitives::batch_lights.after(SystemSets::Batch)
        ],

        // render functions
        "render_meshes_bindless_material" => render_func![
            render_meshes_bindless_material, 
            pass_name,
            (
                Query<(&draw::InstanceBuffer, &MeshComponent)>,
                Query<(&MeshComponent, &WorldMatrix), Without<draw::InstanceBuffer>>,
                Query<&draw::WorldBuffers>
            )
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
            Query<(&draw::InstanceBuffer, &MeshComponent, &PipelineComponent)>
        ],
        "render_meshes_cbuffer_instanced" => render_func![
            render_meshes_cbuffer_instanced, 
            pass_name, 
            Query<(&draw::InstanceBuffer, &MeshComponent, &PipelineComponent)>
        ],
        "render_meshes_push_constants_texture" => render_func![
            render_meshes_push_constants_texture, 
            pass_name, 
            Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>
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