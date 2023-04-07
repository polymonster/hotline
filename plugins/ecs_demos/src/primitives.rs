// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::{prelude::*, gfx::Buffer};
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;

use crate::draw::{self, LightData};

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

/// Init function for primitives demo
#[no_mangle]
pub fn light_primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_light_primitives"
        ],
        update: systems![
            "animate_lights",
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

#[no_mangle]
pub fn batch_lights(
    light_query: Query<(&Position, &Colour), With<LightType>>,
    mut world_buffers_query: Query<&mut draw::WorldBuffers>) {
    let data_size = std::mem::size_of::<LightData>();
    for mut buffers in &mut world_buffers_query {
        let mut offset = 0;
        for (pos, colour) in &light_query {
            buffers.light.write(
                offset, 
                gfx::as_u8_slice(&LightData{
                    pos: pos.0,
                    radius: 64.0,
                    colour: colour.0
            })).unwrap();
            offset += data_size as isize
        }
        break;
    }
}

/// returns a vec4 of rgba in 0-1 range from a packed `rgba` which is inside u32 (4 bytes, R8G8B8A8)
pub fn _rgba8_to_vec4<T: Float + FloatOps<T> + Cast<T>>(rgba: u32) -> Vec4<T> {
    let one_over_255 = T::from_f32(1.0 / 255.0);
    Vec4 {
        x: T::from_u32((rgba >> 24) & 0xff) * one_over_255,
        y: T::from_u32((rgba >> 16) & 0xff) * one_over_255,
        z: T::from_u32((rgba >> 8) & 0xff) * one_over_255,
        w: T::from_u32(rgba & 0xff) * one_over_255
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_light_primitives(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    let num_lights = 64;
    let mut light_data = Vec::new();
    for i in 0..num_lights {
        let pos = vec3f(0.0, 32.0, 0.0);
        let col = match i {
            i if i < 16 => _rgba8_to_vec4(0xf89f5bff),
            i if i < 32 => _rgba8_to_vec4(0xe53f71ff),
            i if i < 48 => _rgba8_to_vec4(0x9c3587ff),
            _ => _rgba8_to_vec4(0x66023cff),
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightType::Point
        ));

        light_data.push(LightData{
            pos: pos,
            radius: 64.0,
            colour: col
        });
    }

    let draw_buf = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::SHADER_RESOURCE,
        cpu_access: gfx::CpuAccessFlags::WRITE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<draw::DrawData>(),
        num_elements: 1,
        initial_state: gfx::ResourceState::ShaderResource
    }, hotline_rs::data![], &mut pmfx.shader_heap).unwrap();

    let material_buf = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::SHADER_RESOURCE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<draw::MaterialData>(),
        num_elements: 1,
        initial_state: gfx::ResourceState::ShaderResource
    }, hotline_rs::data![], &mut pmfx.shader_heap).unwrap();

    let light_buf = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::SHADER_RESOURCE,
        cpu_access: gfx::CpuAccessFlags::WRITE | gfx::CpuAccessFlags::PERSISTENTLY_MAPPED,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<draw::LightData>(),
        num_elements: light_data.len(),
        initial_state: gfx::ResourceState::ShaderResource
    }, hotline_rs::data![], &mut pmfx.shader_heap).unwrap();

    // spawn the world buffer entity
    commands.spawn(draw::WorldBuffers {
        draw: draw_buf,
        material: material_buf,
        light: light_buf
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