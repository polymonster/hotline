// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;

use maths_rs::prelude::*;
use rand::prelude::*;
use bevy_ecs::prelude::*;

///
/// draw
/// 

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

/// Adds a single triangle mesh
#[no_mangle]
pub fn setup_draw(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let pos = Mat34f::identity();
    let scale = Mat34f::from_scale(splat3f(100.0));

    let cube_mesh = hotline_rs::primitives::create_triangle_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}

/// Renders meshes with a draw call (non-indexed) (single triangle)
#[no_mangle]
pub fn draw_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_instanced(3, 1, 0, 0);
    }

    Ok(())
}

///
/// draw_indexed
/// 

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw_indexed(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indexed(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let pos = Mat34f::from_translation(Vec3f::unit_y() * 10.0);
    let scale = Mat34f::from_scale(splat3f(10.0));

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn draw_indexed_push_constants(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_push_constants"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indexed_push_constants(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let dim = 64;
    let dim2 = dim / 2;
    let cube_size = 2.5;

    let half_extent = dim2 as f32 * cube_size;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;
            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            let pos = Mat34f::from_translation(
                vec3f(
                    x as f32 * cube_size - half_extent, 
                    50.0, 
                    y as f32 * cube_size - cube_size * dim as f32 + half_extent
                )
            );

            let scale = Mat34::from_scale(vec3f(1.0, wave_x + wave_y + wave_h, 1.0));

            commands.spawn((
                Position(Vec3f::zero()),
                Velocity(Vec3f::one()),
                MeshComponent(cube_mesh.clone()),
                WorldMatrix(pos * scale)
            ));
        }
    }
}

///
/// draw_indirect
/// 

/// draws 2 meshes one with draw indirect and one witg draw indexed indirect.
/// no root binds are changed or buffers updated, this is just simply to test the execute indirect call
#[no_mangle]
pub fn draw_indirect(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indirect"
        ],
        render_graph: "mesh_draw_indirect",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indirect(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {
    
    let scalar_scale = 10.0;
    let scale = Mat34f::from_scale(splat3f(scalar_scale));

    // draw indirect
    let tri = hotline_rs::primitives::create_triangle_mesh(&mut device.0);
    let pos = Mat34f::from_translation(vec3f(-scalar_scale, scalar_scale, 0.0)); 

    let args = gfx::DrawArguments {
        vertex_count_per_instance: 3,
        instance_count: 1,
        start_vertex_location: 0,
        start_instance_location: 0
    };

    let draw_args = device.create_buffer(&gfx::BufferInfo{
        usage: gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<gfx::DrawArguments>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: 1
    }, hotline_rs::data!(gfx::as_u8_slice(&args))).unwrap();

    let command_signature = device.create_indirect_render_command::<gfx::DrawArguments>(
        vec![gfx::IndirectArgument{
            argument_type: gfx::IndirectArgumentType::Draw,
            arguments: None
        }], 
        None
    ).unwrap();

    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(tri.clone()),
        WorldMatrix(pos * scale),
        BufferComponent(draw_args),
        CommandSignatureComponent(command_signature)
    ));

    // draw indexed indirect
    let teapot = hotline_rs::primitives::create_teapot_mesh(&mut device.0, 8);
    let pos = Mat34f::from_translation(vec3f(scalar_scale, scalar_scale, 0.0)); 

    let args = gfx::DrawIndexedArguments {
        index_count_per_instance: teapot.num_indices,
        instance_count: 1,
        start_index_location: 0,
        base_vertex_location: 0,
        start_instance_location: 0
    };

    let draw_indexed_args = device.create_buffer(&gfx::BufferInfo{
        usage: gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<gfx::DrawIndexedArguments>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: 1
    }, hotline_rs::data!(gfx::as_u8_slice(&args))).unwrap();

    let command_signature = device.create_indirect_render_command::<gfx::DrawIndexedArguments>(
        vec![gfx::IndirectArgument{
            argument_type: gfx::IndirectArgumentType::DrawIndexed,
            arguments: None
        }], 
        None
    ).unwrap();

    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(teapot.clone()),
        WorldMatrix(pos * scale),
        BufferComponent(draw_indexed_args),
        CommandSignatureComponent(command_signature)
    ));
}

/// Renders meshes indirectly in a basic way, we issues some execute indirect draw whit arguments pre-populated in a buffer
#[no_mangle]
pub fn draw_meshes_indirect(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_indirect_query: Query<(&WorldMatrix, &MeshComponent, &CommandSignatureComponent, &BufferComponent)>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    for (world_matrix, mesh, command, args) in &mesh_draw_indirect_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);

        view.cmd_buf.execute_indirect(
            &command.0, 
            1, 
            &args.0, 
            0, 
            None, 
            0
        );
    }

    Ok(())
}

///
/// draw_indexed_vertex_buffer_instanced
/// 

/// Creates a instance batch, where the `InstanceBatch` parent will update a vertex buffer containing
/// it's child (instance) entities. The vertex shader layput steps the instance buffer per instance
#[no_mangle]
pub fn draw_indexed_vertex_buffer_instanced(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_vertex_buffer_instanced"
        ],
        update: systems![
            "rotate_meshes",
            "batch_world_matrix_instances"
        ],
        render_graph: "mesh_debug_vertex_buffer_instanced"
    }
}

#[no_mangle]
pub fn setup_draw_indexed_vertex_buffer_instanced(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_cube_mesh(&mut device.0),
        hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
    ];

    // square number of rows and columns
    let mut rng = rand::thread_rng();

    let size = 2.0;
    let num = 64;
    let instance_count = (num*num) as u32;
    let range = size * size * (num as f32);

    for mesh in meshes {
        let parent = commands.spawn(InstanceBatch {
            mesh: MeshComponent(mesh.clone()),
            pipeline: PipelineComponent("mesh_debug_vertex_buffer_instanced".to_string()),
            instance_buffer: InstanceBuffer { 
                buffer: device.create_buffer(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::VERTEX,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<Mat34f>(),
                    num_elements: instance_count as usize,
                    initial_state: gfx::ResourceState::VertexConstantBuffer
                }, hotline_rs::data![]).unwrap(),
                instance_count,
                heap: None
            }
        }).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised 
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                commands.spawn(Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                });
            }
        }
    }
}

/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_vertex_buffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        
        // bind the shader resource heap for t0 (if exists)
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
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

///
/// draw_indexed_cbuffer_instanced
/// 

/// Creates a instance batch, where the `InstanceBatch` parent will update a cbuffer containing 
/// the cbuffer is created in a separate heap and the matrices and indexed into using the instance id system value semantic
#[no_mangle]
pub fn draw_indexed_cbuffer_instanced(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_cbuffer_instanced"
        ],
        update: systems![
            "rotate_meshes",
            "batch_world_matrix_instances"
        ],
        render_graph: "mesh_debug_cbuffer_instanced"
    }
}

#[no_mangle]
pub fn setup_draw_indexed_cbuffer_instanced(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 5, false, true),
    ];

    // square number of rows and columns
    let mut rng = rand::thread_rng();

    let size = 2.0;
    let num = 32; // max number of bytes in cbuffer is 65536
    let instance_count = (num*num) as u32;
    let range = size * size * (num as f32);

    for mesh in meshes {
        let mut heap = device.create_heap(&gfx::HeapInfo {
            heap_type: gfx::HeapType::Shader,
            num_descriptors: instance_count as usize
        });
        let parent = commands.spawn(InstanceBatch {
            mesh: MeshComponent(mesh.clone()),
            pipeline: PipelineComponent("mesh_debug_cbuffer_instanced".to_string()),
            instance_buffer: InstanceBuffer { 
                buffer: device.create_buffer_with_heap(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::CONSTANT_BUFFER,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<Mat34f>(),
                    num_elements: instance_count as usize,
                    initial_state: gfx::ResourceState::VertexConstantBuffer
                }, hotline_rs::data![], &mut heap).unwrap(),
                instance_count,
                heap: Some(heap)
            }
        }).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised 
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                commands.spawn(Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                });
            }
        }
    }
}

/// Renders all scene instance batches with cbuffer instance buffer
#[no_mangle]
pub fn render_meshes_cbuffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

        view.cmd_buf.set_render_heap(1, instance_batch.heap.as_ref().unwrap(), 0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);

        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// 
/// draw_push_constants_texture
///

#[no_mangle]
pub fn draw_push_constants_texture(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_push_constants_texture"
        ],
        render_graph: "mesh_push_constants_texture",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_push_constants_texture(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let sphere = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_albedo.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_normal.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/bluechecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/redchecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap())
    ];

    // square number of rows and columns
    let rc = sqrt(textures.len() as f32);
    let irc = (rc + 0.5) as usize; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(textures[y * irc + x].get_srv_index().unwrap() as u32),
            ));
        }
    }

    
    // spawn entities to keep hold of textures
    for tex in textures {
        commands.spawn(
            tex
        );
    }

    // dbeug prims uvs
    let debug_uvs = false;
    if debug_uvs {
        let meshes = vec![
            hotline_rs::primitives::create_plane_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_mesh(&mut device.0),
            hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
            hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_billboard_mesh(&mut device.0),
            hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
            hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
            hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
            hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5),
            hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        ];
    
        let uv_debug_tex = TextureComponent(image::load_texture_from_file(
            &mut device, 
            &hotline_rs::get_src_data_path("textures/blend_test_fg.png"),
            Some(&mut pmfx.shader_heap)
        ).unwrap());
    
        let rc = ceil(sqrt(meshes.len() as f32));
        let irc = (rc + 0.5) as usize; 
    
        let size = 10.0;
        let half_size = size * 0.5;    
        let step = size * half_size;
        let half_extent = (rc-1.0) * step * 0.5;
        let start_pos = vec3f(-half_extent, size, -half_extent);
    
        let mut i = 0;
        for y in 0..irc {
            for x in 0..irc {
                if i < meshes.len() {
                    let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                    commands.spawn((
                        MeshComponent(meshes[i].clone()),
                        Position(iter_pos),
                        Rotation(Quatf::identity()),
                        Scale(splat3f(size)),
                        WorldMatrix(Mat34f::identity()),
                        TextureInstance(uv_debug_tex.get_srv_index().unwrap() as u32),
                    ));
                    i += 1;
                }
            }
        }

        commands.spawn(
            uv_debug_tex
        );
    }
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
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    for (world_matrix, mesh, texture) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 1, 16, gfx::as_u8_slice(&texture.0));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///
/// draw_material
/// 

/// Creates instance batches for each mesh and makes an instanced draw call per mesh
/// entity id's for lookups are stored in vertex buffers
/// instance data is stored in a structured buffer (world matrix, material id?)
/// material data is stored in a structured buffer  
#[no_mangle]
pub fn draw_material(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_material"
        ],
        update: systems![
            "rotate_meshes",
            "batch_material_instances",
            "batch_bindless_world_matrix_instances"
        ],
        render_graph: "mesh_material"
    }
}

fn load_material(
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
pub fn setup_draw_material(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
        hotline_rs::primitives::create_teapot_mesh(&mut device.0, 8)
    ];

    let materials = vec![
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/angled-tiled-floor")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/antique-grate1")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/cracking-painted-asphalt")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/dirty-padded-leather")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/green-ceramic-tiles")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/office-carpet-fabric1")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/rusting-lined-metal2")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/simple-basket-weave")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/stone-block-wall")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/worn-painted-cement")).unwrap()
    ];
    let material_dist = rand::distributions::Uniform::from(0..materials.len());

    // square number of rows and columns
    let mut rng = rand::thread_rng();

    let size = 2.0;
    let num = 64;
    let range = size * size * (num as f32);
    let mut entity_itr = 0;

    for mesh in meshes {
        let instance_count = (num*num) as u32;
        let parent = commands.spawn(InstanceBatch {
            mesh: MeshComponent(mesh.clone()),
            pipeline: PipelineComponent(String::new()),
            instance_buffer: InstanceBuffer { 
                buffer: device.create_buffer(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::VERTEX,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<Vec4u>(),
                    num_elements: instance_count as usize,
                    initial_state: gfx::ResourceState::VertexConstantBuffer
                }, hotline_rs::data![]).unwrap(),
                instance_count,
                heap: None
        }}).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                commands.spawn((Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                },
                InstanceIds {
                    entity_id: entity_itr,
                    material_id: material_dist.sample(&mut rng) as u32
                }));

                // increment
                entity_itr += 1;
            }
        }
    }

    let mut material_data = Vec::new();
    for material in &materials {
        material_data.push(
            MaterialData {
                albedo_id: material.albedo.get_srv_index().unwrap() as u32,
                normal_id: material.normal.get_srv_index().unwrap() as u32,
                roughness_id: material.roughness.get_srv_index().unwrap() as u32,
                padding: 100
            }
        );
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_itr as usize,
        material_capacity: materials.len(),
        ..Default::default()
    });

    pmfx.get_world_buffers_mut().material.write(0, &material_data);

    for material in materials {
        commands.spawn(material);
    }
}

///
/// draw_indirect_gpu_frustum_culling
/// 

// cpu 80ms, gpu 20ms
// 22503776 ia verts
// 7501392 ia primitives
// 13924796 vs invocations

// copy data to a uav buffer in shader

// struct of world matrix, local aabb?
// compute frustum cull + build uav
// aabb from meshes

// - CopyBufferRegion to clear the UAV counter
// - buffer counter passed to execute indirect

pub struct DrawIndirectArgs {
    pub vertex_buffer: gfx::VertexBufferView,
    pub index_buffer: gfx::IndexBufferView,
    pub draw_id: u32,
    pub args: gfx::DrawIndexedArguments,
}

#[derive(Component)]
pub struct DrawIndirectComponent {
    /// Command signature for a particular setup
    signature: gfx_platform::CommandSignature,
    /// Max count inside the buffers
    max_count: u32,
    /// Contains all possible daw args for rendering objects of `max_count`
    arg_buffer: gfx_platform::Buffer,
    /// the buffer is generated by the GPU it may be less than `max_count` if the entities are culled
    dynamic_buffer: gfx_platform::Buffer,
    /// a buffer to clear the append counter
    counter_reset: gfx_platform::Buffer
}

#[no_mangle]
pub fn draw_indirect_gpu_frustum_culling(
    client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indirect_gpu_frustum_culling"
        ],
        update: systems![
            "swirling_meshes",
            "batch_bindless_world_matrices"
        ],
        render_graph: "mesh_draw_indirect_culling"
    }
}

#[no_mangle]
pub fn setup_draw_indirect_gpu_frustum_culling(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1),
        hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1),
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 5, false, true),
        hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 0.25, 0.7),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 0.25, 0.5),
        hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 16, true, true, 1.0, 0.66, 1.0),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 8, true, true, 1.0, 0.66, 1.0),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 8, true, true, 0.33, 0.66, 0.33),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 3, 0, 3, false, true, 0.33, 0.66, 1.0),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 4, 0, 4, false, true, 0.33, 0.9, 1.0),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 5, 0, 4, false, true, 0.33, 0.33, 1.0),
    ];
    let mesh_dist = rand::distributions::Uniform::from(0..meshes.len());

    let irc = 256;
    let size = 10.0;
    let frc = 1.0 / irc as f32;
    let mut rng = rand::thread_rng();
    let entity_count = irc * irc;

    let pipeline = pmfx.get_render_pipeline("mesh_test_indirect").unwrap();
    
    let command_signature = device.create_indirect_render_command::<DrawIndirectArgs>(
        vec![
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::VertexBuffer,
                arguments: Some(gfx::IndirectTypeArguments {
                    buffer: gfx::IndirectBufferArguments {
                        slot: 0
                    }
                })
            },
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::IndexBuffer,
                arguments: None
            },
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::PushConstants,
                arguments: Some(gfx::IndirectTypeArguments {
                    push_constants: gfx::IndirectPushConstantsArguments {
                        slot: pipeline.get_descriptor_slot(1, gfx::DescriptorType::PushConstants).unwrap().slot,
                        offset: 0,
                        num_values: 1
                    }
                })
            },
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::DrawIndexed,
                arguments: None
            }
        ], 
        Some(pipeline)
    ).unwrap();

    let mut indirect_args = Vec::new();

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            let offset = rng.gen::<f32>() * 750.0;
            let mut iter_pos = vec3f(cos(x as f32 / frc), sin(y as f32 / frc), sin(x as f32 / frc)) * (1000.0 - offset);
            iter_pos.y += 1000.0;
            let imesh = mesh_dist.sample(&mut rng);
            let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::two_pi();
            commands.spawn((
                Position(iter_pos),
                Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity())
            ));

            indirect_args.push(DrawIndirectArgs {
                draw_id: i,
                vertex_buffer: meshes[imesh].vb.get_vbv().unwrap(),
                index_buffer: meshes[imesh].ib.get_ibv().unwrap(),
                args: gfx::DrawIndexedArguments {
                    index_count_per_instance: meshes[imesh].num_indices,
                    instance_count: 1,
                    start_index_location: 0,
                    base_vertex_location: 0,
                    start_instance_location: 0
                }
            });
            i += 1;
        }
    }

    // read data from the arg_buffer in compute shader to generate the `dynamic_buffer`
    let arg_buffer = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::SHADER_RESOURCE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<DrawIndirectArgs>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: indirect_args.len()
    }, hotline_rs::data!(&indirect_args), &mut pmfx.shader_heap).unwrap();

    // dynamic buffer has a counter packed at the end
    let dynamic_buffer = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER | gfx::BufferUsage::UNORDERED_ACCESS | gfx::BufferUsage::APPEND_COUNTER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<DrawIndirectArgs>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: indirect_args.len(),
    }, hotline_rs::data![], &mut pmfx.shader_heap).unwrap();

    let counter_reset = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::NONE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<u32>(),
        initial_state: gfx::ResourceState::CopySrc,
        num_elements: 1,
    }, hotline_rs::data![gfx::as_u8_slice(&0)], &mut pmfx.shader_heap).unwrap();

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_count as usize,
        camera_capacity: 1 as usize, // main camera
        ..Default::default()
    });

    commands.spawn((
        DrawIndirectComponent {
            signature: command_signature,
            arg_buffer: arg_buffer,
            dynamic_buffer: dynamic_buffer,
            max_count: entity_count,
            counter_reset: counter_reset
        },
        MeshComponent(meshes[0].clone())
    ));

    // keep hold of meshes
    for mesh in meshes {
        commands.spawn(
            MeshComponent(mesh.clone())
        );
    }
}

#[no_mangle]
pub fn swirling_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<(&mut Rotation, &mut Position)>) {

    let mut i = 0.0;
    for (mut rotation, mut position) in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
        
        let pr = rotate_2d(position.0.xz(), time.accumulated * 0.0001);
        position.0.set_xz(pr);
        
        position.0.y += sin(time.accumulated + i) * 2.0;

        i += 1.0;
    }
}

#[no_mangle]
pub fn dispatch_compute_frustum_cull(
    pmfx: &Res<PmfxRes>,
    pass: &mut pmfx::ComputePass<gfx_platform::Device>,
    indirect_draw_query: Query<&DrawIndirectComponent>) 
    -> Result<(), hotline_rs::Error> {
    
    for indirect_draw in &indirect_draw_query {
        // clears the counter
        pass.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: None,
            buffer: Some(&indirect_draw.dynamic_buffer),
            state_before: gfx::ResourceState::IndirectArgument,
            state_after: gfx::ResourceState::CopyDst,
        });
    
        let offset = indirect_draw.dynamic_buffer.get_counter_offset().unwrap();
        pass.cmd_buf.copy_buffer_region(&indirect_draw.dynamic_buffer, offset, &indirect_draw.counter_reset, 0, std::mem::size_of::<u32>());
    
        pass.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: None,
            buffer: Some(&indirect_draw.dynamic_buffer),
            state_before: gfx::ResourceState::CopyDst,
            state_after: gfx::ResourceState::UnorderedAccess,
        });

        // run the shader to cull the entities
        let pipeline = pmfx.get_compute_pipeline(&pass.pass_pipline).unwrap();
        pass.cmd_buf.set_compute_pipeline(&pipeline);

        // resource index info for looking up input (draw all info) / output (culled draw call info)
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            // output uav
            pass.cmd_buf.push_compute_constants(slot.slot, 1, 0, 
                gfx::as_u8_slice(&indirect_draw.dynamic_buffer.get_uav_index().unwrap()));

            // input srv
            pass.cmd_buf.push_compute_constants(slot.slot, 1, 1, 
                gfx::as_u8_slice(&indirect_draw.arg_buffer.get_srv_index().unwrap()));
        }
        
        // world buffer info to lookup matrices and aabb info
        let world_buffer_info = pmfx.get_world_buffer_info();
        let slot = pipeline.get_descriptor_slot(2, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            // println!("{}", world_buffer_info.draw.index);
            pass.cmd_buf.push_compute_constants(
                slot.slot, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
        }

        // bind the heap for un-ordered access and srvs, it should be on the same slot
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::UnorderedAccess);
        if let Some(slot) = slot {
            pass.cmd_buf.set_compute_heap(slot.slot, &pmfx.shader_heap);
        }

        pass.cmd_buf.dispatch(
            gfx::Size3 {
                x: indirect_draw.max_count / pass.thread_count.x,
                y: pass.thread_count.y,
                z: pass.thread_count.z
            },
            pass.thread_count
        );

        // transition to `IndirectArgument`
        pass.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: None,
            buffer: Some(&indirect_draw.dynamic_buffer),
            state_before: gfx::ResourceState::UnorderedAccess,
            state_after: gfx::ResourceState::IndirectArgument,
        });
    }

    Ok(())
}

#[no_mangle]
pub fn draw_meshes_indirect_culling(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    indirect_draw_query: Query<&DrawIndirectComponent>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    
    // bind the shader resource heap for t0 (if exists)
    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // bind the shader resource heap for t1 (if exists)
    let slot = pipeline.get_descriptor_slot(1, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // bind the world buffer info
    let world_buffer_info = pmfx.get_world_buffer_info();
    let slot = pipeline.get_descriptor_slot(2, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(
            slot.slot, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

    for indirect_draw in &indirect_draw_query {
        view.cmd_buf.execute_indirect(
            &indirect_draw.signature,
            indirect_draw.max_count,
            &indirect_draw.dynamic_buffer,
            0,
            Some(&indirect_draw.dynamic_buffer),
            indirect_draw.dynamic_buffer.get_counter_offset().unwrap()
        );
    }

    Ok(())
}