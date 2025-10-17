// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Dynamic Cubemap
/// 

use crate::prelude::*;

#[derive(Component)]
pub struct Probe;

#[no_mangle]
pub fn dynamic_cubemap(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/util").as_str()).unwrap();
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_dynamic_cubemap"
        ],
        update: systems![
            "orbit_meshes"
        ],
        render_graph: "dynamic_cubemap",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_dynamic_cubemap(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    // sphere for cubemap
    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);
    let sphere_size = 100.0;

    let tex = pmfx.get_texture("dynamic_cubemap").unwrap();
    let srv = tex.get_srv_index().unwrap() as u32;

    commands.spawn((
        Position(Vec3f::zero()),
        Scale(splat3f(sphere_size)),
        Rotation(Quatf::identity()),
        MeshComponent(sphere_mesh.clone()),
        WorldMatrix(Mat34f::identity()),
        TextureInstance(srv),
        Probe
    ));

    // orbiting primitives
    let orbit_meshes = [
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 3, 0, 3, false, true, 0.33, 0.66, 1.0),
        hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 8, true, true, 1.0, 0.66, 1.0),
        hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_teapot_mesh(&mut device.0, 4)
    ];

    let mut rng = rand::thread_rng();
    let dist = rand::distributions::Uniform::from(0..orbit_meshes.len());

    let num_primitves = 32; 
    for i in 0..num_primitves {
        let rv = normalize(vec3f(rng.gen(), rng.gen(), rng.gen()) * 2.0 - 1.0);
        let rr = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::tau();
        let offset : f32 = 20.0 + rng.gen::<f32>() * 20.0;
        let mi = dist.sample(&mut rng);

        commands.spawn((
            Position(rv * (sphere_size + offset)),
            Scale(splat3f(5.0)),
            Rotation(Quatf::from_euler_angles(rr.x, rr.y, rr.z)),
            MeshComponent(orbit_meshes[mi].clone()),
            WorldMatrix(Mat34f::identity())
        ));
    }

    pmfx.update_cubemap_camera_constants("cubemap_camera", Vec3f::zero(), 0.1, 1000.0);

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn orbit_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<(&mut Rotation, &mut Position), Without<Probe>>) -> Result<(), hotline_rs::Error> {

    for (mut rotation, mut position) in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
        position.0 = Quat::from_euler_angles(0.0, f32::pi() * time.0.delta * 0.2, 0.0) * position.0;
    }

    Ok(())
}

#[no_mangle]
#[export_render_fn]
pub fn render_orbit_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent), Without<TextureInstance>>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    let mut mip = 0;
    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        mip += 1;
    }

    Ok(())
}

#[no_mangle]
#[export_render_fn]
pub fn render_meshes_cubemap_reflect(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    view.cmd_buf.push_render_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    view.cmd_buf.set_heap(pipeline, &pmfx.shader_heap);

    let mut mip = 0;
    for (world_matrix, mesh, cubemap) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[cubemap.0, mip, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);

        mip += 1;
    }

    Ok(())
}
