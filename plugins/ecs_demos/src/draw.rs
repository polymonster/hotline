// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::{prelude::*, gfx::Buffer};

use maths_rs::prelude::*;
use rand::prelude::*;
use bevy_ecs::prelude::*;

#[derive(Component)]
pub struct InstanceBuffer {
    pub heap: Option<gfx_platform::Heap>,
    pub buffer: gfx_platform::Buffer,
    pub instance_count: u32
}

#[derive(Component)]
pub struct BufferComponent(pub gfx_platform::Buffer);

#[derive(Bundle)]
pub struct InstanceBatch {
    mesh: MeshComponent,
    pipeline: PipelineComponent,
    instance_buffer: InstanceBuffer
}

#[derive(Component)]
pub struct Parent(Entity);

#[derive(Bundle)]
pub struct Instance {
    pos: Position,
    rot: Rotation,
    scale: Scale,
    world_matrix: WorldMatrix,
    parent: Parent
}

#[derive(Bundle)]
pub struct InstanceMaterial {
    buffer: BufferComponent,
    entity_id: EntityId,
    material_id: MaterialInstance
}

pub struct EntityInstanceData {
    pub world_matrix: WorldMatrix,
    pub ids: Vec4u
}

#[derive(Component)]
pub struct EntityId(u32);

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
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

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
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands:  bevy_ecs::system::Commands) {

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


#[no_mangle]
pub fn batch_material_instances(
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

// material ID or entity instance ID, material ID in vertex buffer
// draw structured buffer per entity (world matrix)
// structured buffer per material (albedo, normal, metalness, roughness id's)
// cbuffer for view constants

/*
#[derive(Component)]
pub struct AlbedoMap(pub gfx_platform::Texture);

#[derive(Component)]
pub struct NormalMap(pub gfx_platform::Texture);

#[derive(Bundle)]
pub struct Material {
    pub albedo: AlbedoMap,
    pub normal: NormalMap
}

#[derive(Component)]
pub struct MaterialInstance {
    pub albedo: u32,
    pub normal: u32
}
*/

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

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_draw_push_constants_texture(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let sphere = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_albedo.dds")).unwrap()
        ),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_normal.dds")).unwrap()
        ),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/bluechecker01.dds")).unwrap()
        ),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/redchecker01.dds")).unwrap()
        )
    ];

    // square number of rows and columns
    let rc = 2.0;
    let irc = (rc + 0.5) as usize; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size * 1.8, -half_extent * 4.0);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(10.0)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(textures[y * irc + x].get_srv_index().unwrap() as u32),
            ));
        }
    }

    // spawn entity to keep hold of material
    for tex in textures {
        commands.spawn(
            tex
        );
    }
}

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
            "rotate_meshes"
        ],
        render_graph: "mesh_material_instanced"
    }
}

#[no_mangle]
pub fn setup_draw_material(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true)
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
            pipeline: PipelineComponent("mesh_material_instanced".to_string()),
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
            }
        }).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised 
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                let buf = device.create_buffer(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::SHADER_RESOURCE,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<EntityInstanceData>(),
                    num_elements: 1,
                    initial_state: gfx::ResourceState::ShaderResource
                }, hotline_rs::data![]).unwrap();
                let srv_index = buf.get_srv_index().unwrap();
                commands.spawn((Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                }, InstanceMaterial {
                    buffer: BufferComponent(buf),
                    material_id: MaterialInstance(0),
                    entity_id: EntityId(srv_index as u32),
                }));
            }
        }
    }
}