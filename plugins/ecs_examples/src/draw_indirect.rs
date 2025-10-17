// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Draw Indirect
///

use crate::prelude::*;

/// draws 2 meshes one with draw indirect and one with draw indexed indirect.
/// no root binds are changed or buffers updated, this is just simply to test the execute indirect call
#[no_mangle]
pub fn draw_indirect(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indirect"
        ],
        render_graph: "mesh_draw_indirect",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_draw_indirect(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {
    
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
        MeshComponent(tri),
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

    Ok(())
}

/// Renders meshes indirectly in a basic way, we issues some execute indirect draw whit arguments pre-populated in a buffer
#[no_mangle]
#[export_render_fn]
pub fn draw_meshes_indirect(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_indirect_query: Query<(&WorldMatrix, &MeshComponent, &CommandSignatureComponent, &BufferComponent)>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
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