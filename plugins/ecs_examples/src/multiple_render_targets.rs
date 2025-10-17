// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Multiple Render Targets
/// 

use crate::prelude::*;

#[no_mangle]
pub fn multiple_render_targets(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_multiple_render_targets"
        ],
        render_graph: "multiple_render_targets",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_multiple_render_targets(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let dim = 1024;
    let plane_mesh = hotline_rs::primitives::create_plane_mesh(&mut device.0, 128);

    commands.spawn((
        MeshComponent(plane_mesh.clone()),
        Position(vec3f(0.0, 0.0, 0.0)),
        Rotation(Quatf::identity()),
        Scale(splat3f(dim as f32)),
        WorldMatrix(Mat34f::identity()),
        PipelineComponent("heightmap_mrt".to_string())
    ));

    Ok(())
}