// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Geometry Primitives
/// 

use crate::prelude::*;

#[derive(Clone, Copy)]
enum MeshType {
    Normal,
    Billboard,
    CylindricalBillboard
}

/// Init function for primitives demo
#[no_mangle]
pub fn geometry_primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_geometry_primitives"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_wireframe_overlay"
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
#[export_update_fn]
pub fn setup_geometry_primitives(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let meshes = vec![
        (hotline_rs::primitives::create_billboard_mesh(&mut device.0), MeshType::Billboard),
        (hotline_rs::primitives::create_billboard_mesh(&mut device.0), MeshType::CylindricalBillboard),
        (hotline_rs::primitives::create_plane_mesh(&mut device.0, 1), MeshType::Normal),
        (hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0), MeshType::Normal),
        (hotline_rs::primitives::create_cube_mesh(&mut device.0), MeshType::Normal),
        (hotline_rs::primitives::create_octahedron_mesh(&mut device.0), MeshType::Normal),
        (hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0), MeshType::Normal),
        (hotline_rs::primitives::create_icosahedron_mesh(&mut device.0), MeshType::Normal),
        (hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1), MeshType::Normal),
        (hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1), MeshType::Normal),
        (hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16), MeshType::Normal),
        (hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 1.0, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 1.0, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16), MeshType::Normal),
        (hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true), MeshType::Normal),
        (hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 5, false, true), MeshType::Normal),
        (hotline_rs::primitives::create_cone_mesh(&mut device.0, 16), MeshType::Normal),
        (hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16), MeshType::Normal),
        (hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16), MeshType::Normal),
        (hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 0.25, 0.7), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5), MeshType::Normal),
        (hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 0.25, 0.5), MeshType::Normal),
        (hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 16, true, true, 1.0, 0.66, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 8, true, true, 1.0, 0.66, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 16, 0, 8, true, true, 0.33, 0.66, 0.33), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 3, 0, 3, false, true, 0.33, 0.66, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 4, 0, 4, false, true, 0.33, 0.9, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_tube_prism_mesh(&mut device.0, 5, 0, 4, false, true, 0.33, 0.33, 1.0), MeshType::Normal),
        (hotline_rs::primitives::create_teapot_mesh(&mut device.0, 4), MeshType::Normal),
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
                let mesh_type = meshes[i].1;
                match mesh_type {
                    MeshType::Normal => {
                        commands.spawn((
                            MeshComponent(meshes[i].0.clone()),
                            Position(iter_pos),
                            Rotation(Quatf::from_euler_angles(0.5, 0.0, 0.5)),
                            Scale(splat3f(10.0)),
                            WorldMatrix(Mat34f::identity())
                        ));
                    }
                    MeshType::Billboard => {
                        commands.spawn((
                            MeshComponent(meshes[i].0.clone()),
                            Position(iter_pos),
                            Rotation(Quatf::identity()),
                            Scale(splat3f(10.0)),
                            WorldMatrix(Mat34f::identity()),
                            Billboard
                        ));
                    }
                    MeshType::CylindricalBillboard => {
                        commands.spawn((
                            MeshComponent(meshes[i].0.clone()),
                            Position(iter_pos),
                            Rotation(Quatf::identity()),
                            Scale(vec3f(5.0, 10.0, 5.0)),
                            WorldMatrix(Mat34f::identity()),
                            Billboard,
                            CylindricalBillboard
                        ));
                    }
                }
            }
            i += 1;
        }
    }

    Ok(())
}

/// Rotate all meshes continuously about the y-axis by delta time
#[no_mangle]
#[export_update_fn]
pub fn rotate_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<&mut Rotation, Without<Billboard>>) -> Result<(), hotline_rs::Error> {
    for mut rotation in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
    }

    Ok(())
}