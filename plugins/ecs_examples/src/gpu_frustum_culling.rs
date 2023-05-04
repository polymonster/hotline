// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::prelude::*;

///
/// GPU Frustum Culling
/// 

#[repr(packed)]
pub struct DrawIndirectArgs {
    pub vertex_buffer: gfx::VertexBufferView,
    pub index_buffer: gfx::IndexBufferView,
    pub ids: Vec4u,
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
pub fn gpu_frustum_culling(
    client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_gpu_frustum_culling"
        ],
        update: systems![
            "swirling_meshes",
            "batch_material_instances",
            "batch_lights",
            "batch_bindless_draw_data"
        ],
        render_graph: "execute_indirect_culling"
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_gpu_frustum_culling(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

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

    let num_lights = 16;

    let irc = 180;
    let size = 10.0;
    let frc = 1.0 / irc as f32;
    let mut rng = rand::thread_rng();
    let entity_count = irc * irc;

    let pipeline = pmfx.get_render_pipeline("mesh_indirect_push_constants")?;
    
    // creates an idirect command signature, 
    // we change index and vertex buffer each draw
    // we push the draw_id and material_id unique for each draw
    // and we make a draw inexed call
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
                        slot: pipeline.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants).unwrap().index,
                        offset: 0,
                        num_values: 4
                    }
                })
            },
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::DrawIndexed,
                arguments: None
            }
        ], 
        Some(pipeline)
    )?;

    let mut indirect_args = Vec::new();
    let range = 900.0;

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            let offset = rng.gen::<f32>() * range;
            let mut iter_pos = vec3f(cos(x as f32 / frc), sin(y as f32 / frc), sin(x as f32 / frc)) * (1000.0 - offset);
            iter_pos.y += 1000.0;
            let imesh = mesh_dist.sample(&mut rng);
            let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::two_pi();
            let mat_id = material_dist.sample(&mut rng) as u32;
            commands.spawn((
                Position(iter_pos),
                Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                Scale(splat3f(size + rng.gen::<f32>() * 10.0)),
                WorldMatrix(Mat34f::identity()),
                Extents {
                    aabb_min: meshes[imesh].aabb_min,
                    aabb_max: meshes[imesh].aabb_max
                }
            ));
            indirect_args.push(DrawIndirectArgs {
                ids: Vec4u::new(i, mat_id, 0, 0),
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
    }, hotline_rs::data!(&indirect_args), &mut pmfx.shader_heap)?;

    // dynamic buffer has a counter packed at the end
    let dynamic_buffer = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER | gfx::BufferUsage::UNORDERED_ACCESS | gfx::BufferUsage::APPEND_COUNTER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<DrawIndirectArgs>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: indirect_args.len(),
    }, hotline_rs::data![], &mut pmfx.shader_heap)?;

    // create a buffer with 0, so we can clear the counter each frame by copy buffer rgion
    let counter_reset = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::NONE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<u32>(),
        initial_state: gfx::ResourceState::CopySrc,
        num_elements: 1,
    }, hotline_rs::data![gfx::as_u8_slice(&0)], &mut pmfx.shader_heap)?;

    // spwan the indirect draw entity, which will draw all of the entities
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

    // allocate world buffers
    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_count as usize,
        extent_capacity: entity_count as usize,
        camera_capacity: 1 as usize, // main camera
        material_capacity: materials.len(),
        point_light_capacity: num_lights,
        ..Default::default()
    });

    // add lights
    let light_buffer = &mut pmfx.get_world_buffers_mut().point_light;
    light_buffer.clear();

    let cols : [Vec4f; 4] = [
        rgba8_to_vec4(0x6c698dff),
        rgba8_to_vec4(0xd4d2d5ff),
        rgba8_to_vec4(0xbfafa6ff),
        rgba8_to_vec4(0xe7cee3ff)
    ];
    let col_dist = rand::distributions::Uniform::from(0..cols.len());

    for l in 0..num_lights {
        let mut pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
        
        let mut radius = 64.0;
        let mut col = cols[col_dist.sample(&mut rng)];

        if l == 0 {
            radius = 256.0;
            pos = vec3f(0.0, range, 0.0);
            col = rgba8_to_vec4(0xffffffff);
        }

        if l == 1 {
            radius = 256.0;
            pos = vec3f(0.0, 0.0, 0.0);
            col = rgba8_to_vec4(0xffffffff);
        }
        
        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Point,
                radius: radius,
                ..Default::default()
            }
        ));
        light_buffer.push(&PointLightData{
            pos: pos,
            radius: radius,
            colour: col,
            shadow_map_info: ShadowMapInfo::default()
        });
    }

    // add materials
    let material_buffer = &mut pmfx.get_world_buffers_mut().material;
    material_buffer.clear();

    for material in &materials {
        material_buffer.push(
            &MaterialData {
                albedo_id: material.albedo.get_srv_index().unwrap() as u32,
                normal_id: material.normal.get_srv_index().unwrap() as u32,
                roughness_id: material.roughness.get_srv_index().unwrap() as u32,
                padding: 0
            }
        );
    }

    // keep hold of meshes
    for mesh in meshes {
        commands.spawn(
            MeshComponent(mesh.clone())
        );
    }

    // keep hold of materials
    for material in materials {
        commands.spawn(material);
    }

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn swirling_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<(&mut Rotation, &mut Position)>) -> Result<(), hotline_rs::Error> {

    for (mut rotation, mut position) in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
        let pr = rotate_2d(position.0.xz(), time.0.delta);
        position.0.set_xz(pr);
    }

    Ok(())
}

#[no_mangle]
#[export_compute_fn]
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
        pass.cmd_buf.set_compute_pipeline(pipeline);

        // resource index info for looking up input (draw all info) / output (culled draw call info)
        let slot = pipeline.get_pipeline_slot(0, 1, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            // output uav
            pass.cmd_buf.push_compute_constants(slot.index, 1, 0, 
                gfx::as_u8_slice(&indirect_draw.dynamic_buffer.get_uav_index().unwrap()));

            // input srv
            pass.cmd_buf.push_compute_constants(slot.index, 1, 4, 
                gfx::as_u8_slice(&indirect_draw.arg_buffer.get_srv_index().unwrap()));
        }
        
        // world buffer info to lookup matrices and aabb info
        let world_buffer_info = pmfx.get_world_buffer_info();
        let slot = pipeline.get_pipeline_slot(2, 0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            pass.cmd_buf.push_compute_constants(
                slot.index, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
        }

        pass.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

        pass.cmd_buf.dispatch(
            gfx::Size3 {
                x: indirect_draw.max_count / pass.numthreads.x,
                y: pass.numthreads.y,
                z: pass.numthreads.z
            },
            pass.numthreads
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
#[export_render_fn]
pub fn draw_meshes_indirect_culling(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    indirect_draw_query: Query<&DrawIndirectComponent>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;

    view.cmd_buf.set_render_pipeline(&pipeline);

    // bind the world buffer info
    let world_buffer_info = pmfx.get_world_buffer_info();
    let slot = pipeline.get_pipeline_slot(2, 0, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(
            slot.index, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

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