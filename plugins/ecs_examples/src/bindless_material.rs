// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

/// 
/// Bindless Material
/// 

use crate::prelude::*; 

/// Creates instance batches for each mesh and makes an instanced draw call per mesh
/// entity id's for lookups are stored in vertex buffers
/// instance data is stored in a structured buffer (world matrix, material id?)
/// material data is stored in a structured buffer  
#[no_mangle]
pub fn bindless_material(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_bindless_material"
        ],
        update: systems![
            "rotate_meshes",
            "batch_lights",
            "batch_material_instances",
            "batch_bindless_draw_data"
        ],
        render_graph: "mesh_instanced_bindless_material"
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_bindless_material(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
        hotline_rs::primitives::create_teapot_mesh(&mut device.0, 8)
    ];

    let materials = vec![
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/angled-tiled-floor"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/antique-grate1"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/cracking-painted-asphalt"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/dirty-padded-leather"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/green-ceramic-tiles"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/office-carpet-fabric1"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/rusting-lined-metal2"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/simple-basket-weave"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/stone-block-wall"))?,
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/worn-painted-cement"))?
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
                    parent: Parent(parent),
                },
                InstanceIds {
                    entity_id: entity_itr,
                    material_id: material_dist.sample(&mut rng) as u32
                },
                Extents {
                    aabb_min: mesh.aabb_min,
                    aabb_max: mesh.aabb_max
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

    let num_lights = 16;
    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_itr as usize,
        extent_capacity: entity_itr as usize,
        material_capacity: materials.len(),
        point_light_capacity: num_lights,
        ..Default::default()
    });

    // add lights
    let light_buffer = &mut pmfx.get_world_buffers_mut().point_light;
    light_buffer.clear();

    for _ in 0..num_lights {
        let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
        let col = rgba8_to_vec4(0xf89f5bff);
        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Point,
                radius: 64.0,
                ..Default::default()
            }
        ));
        light_buffer.push(&PointLightData{
            pos: pos,
            radius: 64.0,
            colour: col,
            shadow_map_info: ShadowMapInfo::default()
        });
    }

    pmfx.get_world_buffers_mut().material.write(0, &material_data);

    for material in materials {
        commands.spawn(material);
    }

    Ok(())
}