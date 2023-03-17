// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;

/// Init function for primitives demo
#[no_mangle]
pub fn geometry_primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_geometry_primitives"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_geometry_primitives(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        (hotline_rs::primitives::create_plane_mesh(&mut device.0, 1), false),
        (hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0), false),
        (hotline_rs::primitives::create_cube_mesh(&mut device.0), false),
        (hotline_rs::primitives::create_octahedron_mesh(&mut device.0), false),
        (hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0), false),
        (hotline_rs::primitives::create_icosahedron_mesh(&mut device.0), false),
        (hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1), false),
        (hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1), false),
        (hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16), false),
        (hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true), false),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 1.0, 1.0), false),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0), false),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 1.0, 1.0), false),
        (hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16), false),
        (hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true), false),
        (hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 5, false, true), false),
        (hotline_rs::primitives::create_cone_mesh(&mut device.0, 16), false),
        (hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16), false),
        (hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16), false),
        (hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8), false),
        (hotline_rs::primitives::create_billboard_mesh(&mut device.0), true),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 0.25, 0.7), false),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5), false),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 0.25, 0.5), false),
        (hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4), false)
    ];

    // square number of rows and columns
    let rc = ceil(sqrt(meshes.len() as f32));
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size * 1.8, -half_extent * 4.0);

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < meshes.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                let billboard = meshes[i].1;
                if !billboard {
                    commands.spawn((
                        MeshComponent(meshes[i].0.clone()),
                        Position(iter_pos),
                        Rotation(Quatf::from_euler_angles(0.5, 0.0, 0.5)),
                        Scale(splat3f(10.0)),
                        WorldMatrix(Mat34f::identity())
                    ));
                }
                else {
                    commands.spawn((
                        MeshComponent(meshes[i].0.clone()),
                        Position(iter_pos),
                        Rotation(Quatf::identity()),
                        Scale(splat3f(10.0)),
                        WorldMatrix(Mat34f::identity()),
                        Billboard
                    ));
                }
            }
            i = i + 1;
        }
    }
}