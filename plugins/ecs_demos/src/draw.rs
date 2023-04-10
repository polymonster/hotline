// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use hotline_rs::pmfx::WorldBufferResizeInfo;

use maths_rs::prelude::*;
use rand::prelude::*;
use bevy_ecs::prelude::*;

use maths_rs::Vec4u;

#[derive(Component)]
pub struct InstanceBuffer {
    pub heap: Option<gfx_platform::Heap>,
    pub buffer: gfx_platform::Buffer,
    pub instance_count: u32
}

#[derive(Bundle)]
pub struct InstanceBatch {
    mesh: MeshComponent,
    pipeline: PipelineComponent,
    instance_buffer: InstanceBuffer
}

#[derive(Bundle)]
pub struct Instance {
    pos: Position,
    rot: Rotation,
    scale: Scale,
    world_matrix: WorldMatrix,
    parent: Parent
}

#[derive(Component)]
pub struct InstanceIds {
    entity_id: u32,
    material_id: u32
}

#[derive(Component)]
pub struct MaterialResources {
    pub albedo: gfx_platform::Texture,
    pub normal: gfx_platform::Texture,
    pub roughness: gfx_platform::Texture
}

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

#[no_mangle]
pub fn batch_bindless_world_matrix_instances(
    mut pmfx: ResMut<PmfxRes>,
    instances_query: Query<(&InstanceIds, &WorldMatrix), With<Parent>>) {
    let world_buffers = pmfx.get_world_buffers_mut();
    let mut offset = 0;
    if let Some(buf) = &mut world_buffers.draw {
        for (_, world_matrix) in &instances_query {
            buf.write(
                offset,
                &world_matrix.0
            ).unwrap();
            offset += std::mem::size_of::<DrawData>();
        }
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

///
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

    pmfx.resize_world_buffers(&mut device, WorldBufferResizeInfo {
        draw_count: entity_itr as usize,
        material_count: materials.len(),
        ..Default::default()
    });

    if let Some(buf) = &mut pmfx.get_world_buffers_mut().material {
        buf.write(
            0,
            &material_data
        ).unwrap();
    }

    for material in materials {
        commands.spawn(material);
    }
}