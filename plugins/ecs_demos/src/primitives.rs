// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;

use pmfx::PointLightData;

#[derive(Clone, Copy)]
enum MeshType {
    Normal,
    Billboard,
    CylindricalBillboard
}

/// Init function for primitives demo
#[no_mangle]
pub fn geometry_primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
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
            i = i + 1;
        }
    }
}

#[no_mangle]
pub fn batch_lights(
    mut pmfx: ResMut<PmfxRes>,
    light_query: Query<(&Position, &Colour, &LightType)>) {
    let world_buffers = pmfx.get_world_buffers_mut();
    let mut point_offset = 0;
    let mut spot_offset = 0;
    let mut directional_offset = 0;
    for (pos, colour, light_type) in &light_query {
        match light_type {
            LightType::Point => {
                if let Some(light_buf) = &mut world_buffers.point_light {
                    light_buf.write(
                        point_offset, 
                        gfx::as_u8_slice(&PointLightData{
                            pos: pos.0,
                            radius: 64.0,
                            colour: colour.0
                        }
                    )).unwrap();
                    point_offset += std::mem::size_of::<PointLightData>();
                }
            },
            LightType::Spot => {
                if let Some(light_buf) = &mut world_buffers.spot_light {
                    light_buf.write(
                        spot_offset, 
                        gfx::as_u8_slice(&SpotLightData{
                            pos: pos.0,
                            cutoff: f32::pi() / 8.0,
                            dir: vec3f(0.0, -1.0, 0.0),
                            falloff: 0.5,
                            colour: colour.0,
                        }
                    )).unwrap();
                    spot_offset += std::mem::size_of::<SpotLightData>();
                }
            },
            LightType::Directional => {
                if let Some(light_buf) = &mut world_buffers.directional_light {
                    light_buf.write(
                        directional_offset, 
                        gfx::as_u8_slice(&DirectionalLightData{
                            dir: Vec3f::zero(),
                            colour: colour.0
                        }
                    )).unwrap();
                    directional_offset += std::mem::size_of::<PointLightData>();
                }
            }
        }
    }
}

/// Init function for primitives demo
#[no_mangle]
pub fn point_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_point_lights"
        ],
        update: systems![
            "animate_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_point_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    let num_lights = 64;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 32.0, 0.0);
        let col = match i {
            i if i < 16 => rgba8_to_vec4(0xf89f5bff),
            i if i < 32 => rgba8_to_vec4(0xe53f71ff),
            i if i < 48 => rgba8_to_vec4(0x9c3587ff),
            _ => rgba8_to_vec4(0x66023cff),
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightType::Point
        ));
    }

    pmfx.resize_world_buffers(&mut device, pmfx::WorldBufferResizeInfo{
        point_light_count: num_lights,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 32),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(meshes[0].clone()),
                Position(iter_pos),
                Rotation(Quatf::from_euler_angles(0.5, 0.0, 0.5)),
                Scale(splat3f(10.0)),
                WorldMatrix(Mat34f::identity())
            ));
        }
    }

    // ground plane
    commands.spawn((
        MeshComponent(plane.clone()),
        Position(Vec3f::zero()),
        Rotation(Quatf::identity()),
        Scale(splat3f(half_extent * 2.0)),
        WorldMatrix(Mat34f::identity())
    ));
}

/// Init function for primitives demo
#[no_mangle]
pub fn spot_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_spot_lights"
        ],
        update: systems![
            "animate_lights2",
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_spot_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    // animating lights
    let num_lights = 64;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 128.0, 0.0);
        let col = match i {
            i if i < 16 => rgba8_to_vec4(0xA3C9A8FF),
            i if i < 32 => rgba8_to_vec4(0xA3C9A8FF),
            i if i < 48 => rgba8_to_vec4(0xA3C9A8FF),
            _ => rgba8_to_vec4(0xA3C9A8FF),
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightType::Spot
        ));
    }

    // fixed spots
    commands.spawn((
        Position(vec3f(0.0, 2000.0, 0.0)),
        Colour(Vec4f::cyan() * 0.33),
        LightType::Spot
    ));

    let height = 1000.0;
    let edge = 1500.0;
    let col = vec4f(1.0, 0.5, 0.1, 1.0);

    commands.spawn((
        Position(vec3f(-edge, height, -edge)),
        Colour(col * 0.33),
        LightType::Spot
    ));

    commands.spawn((
        Position(vec3f(edge, height, -edge)),
        Colour(col * 0.33),
        LightType::Spot
    ));

    commands.spawn((
        Position(vec3f(edge, height, edge)),
        Colour(col * 0.33),
        LightType::Spot
    ));

    commands.spawn((
        Position(vec3f(-edge, height, edge)),
        Colour(col * 0.33),
        LightType::Spot
    ));


    pmfx.resize_world_buffers(&mut device, pmfx::WorldBufferResizeInfo{
        spot_light_count: num_lights + 5,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 32),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let height = 50.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, height, y as f32 * step);
            commands.spawn((
                MeshComponent(meshes[0].clone()),
                Position(iter_pos),
                Rotation(Quatf::from_euler_angles(0.0, 0.0, 0.0)),
                Scale(vec3f(size, height, size)),
                WorldMatrix(Mat34f::identity())
            ));
        }
    }

    // ground plane
    commands.spawn((
        MeshComponent(plane.clone()),
        Position(Vec3f::zero()),
        Rotation(Quatf::identity()),
        Scale(splat3f(half_extent * 2.0)),
        WorldMatrix(Mat34f::identity())
    ));
}