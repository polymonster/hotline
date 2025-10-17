// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

///
/// Read-Write Texture
///

use crate::prelude::*;

/// Test compute shader by reading and writing from a 3d texture un-ordered access
#[no_mangle]
pub fn read_write_texture(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_read_write_texture"
        ],
        render_graph: "read_write_texture",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_read_write_texture(
    mut device: ResMut<DeviceRes>,
    pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);

    let tex = pmfx.get_texture("read_write_texture3d").unwrap();
    let srv = tex.get_srv_index().unwrap() as u32;

    let dim = 50.0;
    commands.spawn((
        MeshComponent(cube_mesh),
        Position(vec3f(0.0, dim * 0.5, 0.0)),
        Rotation(Quatf::identity()),
        Scale(splat3f(dim)),
        WorldMatrix(Mat34f::identity()),
        TextureInstance(srv)
    ));

    Ok(())
}