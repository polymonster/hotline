// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;
use rand::prelude::*;

#[derive(Clone, Copy)]
enum MeshType {
    Normal,
    Billboard,
    CylindricalBillboard
}

///
/// geometry_primitives
/// 

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
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

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
                            //Rotation(Quatf::identity()),
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
pub fn rotate_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<&mut Rotation, Without<Billboard>>) {
    for mut rotation in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
    }
}

///
/// tangent_space_normal_maps
/// 

/// Init function for tangent space normal maps to debug tangents 
#[no_mangle]
pub fn tangent_space_normal_maps(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_geometry_primitives",
            "setup_tangent_space_normal_maps"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_debug_tangent_space",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_tangent_space_normal_maps(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands
) {
    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/pbr/antique-grate1/antique-grate1_normal.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap())
    ];

    for tex in textures {
        commands.spawn(
            tex
        );
    }
}

/// Renders all scene meshes with a constant normal map texture, used to debug tangent space on meshes
#[no_mangle]
pub fn render_meshes_debug_tangent_space(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    queries: (
        Query<&TextureComponent>,
        Query<(&WorldMatrix, &MeshComponent)>
    )) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    let (texture_query, mesh_draw_query) = queries;

    // bind first texture
    for texture in &texture_query {
        let usrv = texture.get_srv_index().unwrap() as u32;
        view.cmd_buf.push_render_constants(1, 1, 16, gfx::as_u8_slice(&usrv));
        break;
    }

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///
/// point_lights
/// 

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
            LightComponent {
                light_type: LightType::Point,
                ..Default::default()
            }
        ));
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        point_light_capacity: num_lights,
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
                Rotation(Quatf::identity()),
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

#[no_mangle]
pub fn animate_lights(
    time: Res<TimeRes>, 
    mut light_query: Query<&mut Position, With<LightComponent>>) {
    
    let t = time.accumulated;
    let r = sin(t);

    let rot0 = sin(t);
    let rot1 = sin(-t);
    let rot2 = sin(t * 0.5);
    let rot3 = sin(-t * 0.5);
    
    let step = 1.0 / 16.0;
    let mut f = 0.0;
    let mut i = 0;
    for mut position in &mut light_query {
        if i < 16 {
            position.x = r * cos(f32::tau() * f) * 1000.0;
            position.z = r * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
            f += step;
        }
        else if i < 32 {
            position.x = (r + 1.0) * cos(f32::tau() * f) * 1000.0;
            position.z = (r + 1.0) * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot2);
            position.set_xz(pr);
            f += step;
        }
        else if i < 48 {
            position.x = (r - 1.0) * cos(f32::tau() * f) * 1000.0;
            position.z = (r - 1.0) * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot3);
            position.set_xz(pr);
            f += step;
        }
        else if i < 64 {
            position.x = r * 2.0 * cos(f32::tau() * f) * 1000.0;
            position.z = r * 2.0 * sin(f32::tau() * f) * 1000.0;
            let pr = rotate_2d(position.xz(), rot1);
            position.set_xz(pr);
            f += step;
        }
        i += 1;
    }
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
            LightComponent {
                light_type: LightType::Spot,
                ..Default::default()
            }
        ));
    }

    // fixed spots
    commands.spawn((
        Position(vec3f(0.0, 2000.0, 0.0)),
        Colour(Vec4f::cyan() * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    let height = 1000.0;
    let edge = 1500.0;
    let col = vec4f(1.0, 0.5, 0.1, 1.0);

    commands.spawn((
        Position(vec3f(-edge, height, -edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(edge, height, -edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(edge, height, edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    commands.spawn((
        Position(vec3f(-edge, height, edge)),
        Colour(col * 0.33),
        LightComponent {
            light_type: LightType::Spot,
            ..Default::default()
        }
    ));

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        spot_light_capacity: num_lights + 5,
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

#[no_mangle]
pub fn animate_lights2(
    time: Res<TimeRes>, 
    mut light_query: Query<&mut Position, With<LightComponent>>) {
    
    let t = time.accumulated;
    let rot0 = t;
    
    let mut i = 0;
    for mut position in &mut light_query {
        if i < 16 {
            let fi = i as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
        }
        else if i < 32 {
            let fi = (i-16) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = -sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), -rot0);
            position.set_xz(pr);
        }
        else if i < 48 {
            let fi = (i-32) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = -cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), -rot0);
            position.set_xz(pr);
        }
        else if i < 64 {
            let fi = (i-48) as f32 / 16.0;
            let ts = 1.0 - ((t + (1.0-fi)) % 1.0);

            let ss = 300.0 * ts;
            position.x = -sin(fi * f32::two_pi()) * f32::tau() * ss;
            position.z = -cos(fi * f32::two_pi()) * f32::tau() * ss;
            
            let pr = rotate_2d(position.xz(), rot0);
            position.set_xz(pr);
        }
        else {
            let pr = rotate_2d(position.xz(), sin(rot0));
            position.set_xz(pr);
        }
        i += 1;
    }
}

///
/// directional_lights
/// 

/// Init function for primitives demo
#[no_mangle]
pub fn directional_lights(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_directional_lights"
        ],
        update: systems![
            "animate_lights3",
            "batch_lights"
        ],
        render_graph: "mesh_lit",
        ..Default::default()
    }
}

/// Sets up one of each primitive, evenly spaced and tiled so its easy to extend and add more
#[no_mangle]
pub fn setup_directional_lights(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {
    let plane = hotline_rs::primitives::create_plane_mesh(&mut device.0, 1);

    // animating lights
    let num_lights = 4;
    for i in 0..num_lights {
        let pos = vec3f(0.0, 128.0, 0.0);

        let col = match i {
            0 => vec4f(0.25, 0.0, 0.25, 0.5),
            1 => vec4f(0.25, 0.25, 0.0, 0.5),
            2 => vec4f(0.0, 0.25, 0.25, 0.5),
            _ => vec4f(0.25, 0.0, 0.5, 0.5)
        };

        commands.spawn((
            Position(pos),
            Colour(col),
            LightComponent {
                light_type: LightType::Directional,
                direction: normalize(vec3f(0.5, -0.5, 0.5)),
                ..Default::default()
            }
        ));
    }

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        directional_light_capacity: num_lights,
        ..Default::default()
    });

    let meshes = vec![
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
    ];

    // square number of rows and columns
    let rc = 100.0;
    let irc = rc as i32;

    let size = 10.0;
    let height = 10.0;
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
                Rotation(Quatf::identity()),
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

#[no_mangle]
pub fn animate_lights3(
    time: Res<TimeRes>, 
    mut light_query: Query<(&mut Position, &mut LightComponent)>) {
    
    let t = time.accumulated;
    let r = sin(t);
    let rot0 = sin(t);
    
    let step = 1.0 / 4.0;
    let mut f = 0.0;
    for (mut position, mut light) in &mut light_query {
        position.x = r * (cos(f32::tau() * f) * 2.0 - 1.0) * 500.0;
        position.z = r * (sin(f32::tau() * f) * 2.0 - 1.0) * 500.0;
        
        let pr = rotate_2d(position.xz(), rot0);
        position.set_xz(pr);

        // derive direction from position, always look at the origin
        light.direction = normalize(-position.0);

        f += step;
    }
}

///
/// draw
/// 

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

/// Adds a single triangle mesh
#[no_mangle]
pub fn setup_draw(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let pos = Mat34f::identity();
    let scale = Mat34f::from_scale(splat3f(100.0));

    let cube_mesh = hotline_rs::primitives::create_triangle_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}

/// Renders meshes with a draw call (non-indexed) (single triangle)
#[no_mangle]
pub fn draw_meshes(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_instanced(3, 1, 0, 0);
    }

    Ok(())
}

///
/// draw_indexed
/// 

/// Sets up a single cube mesh to test draw indexed call with a single enity
#[no_mangle]
pub fn draw_indexed(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indexed(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let pos = Mat34f::from_translation(Vec3f::unit_y() * 10.0);
    let scale = Mat34f::from_scale(splat3f(10.0));

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn draw_indexed_push_constants(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_push_constants"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indexed_push_constants(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    let dim = 64;
    let dim2 = dim / 2;
    let cube_size = 2.5;

    let half_extent = dim2 as f32 * cube_size;

    for y in 0..dim {    
        for x in 0..dim {    
            let wave_x = f32::abs(f32::sin((x as f32) / 20.0 as f32)) * 20.0;
            let wave_y = f32::abs(f32::sin((y as f32) / 20.0 as f32)) * 20.0;
            let wave_h = f32::cos(y as f32) + f32::sin(x as f32 / 0.5);

            let pos = Mat34f::from_translation(
                vec3f(
                    x as f32 * cube_size - half_extent, 
                    50.0, 
                    y as f32 * cube_size - cube_size * dim as f32 + half_extent
                )
            );

            let scale = Mat34::from_scale(vec3f(1.0, wave_x + wave_y + wave_h, 1.0));

            commands.spawn((
                Position(Vec3f::zero()),
                Velocity(Vec3f::one()),
                MeshComponent(cube_mesh.clone()),
                WorldMatrix(pos * scale)
            ));
        }
    }
}

///
/// draw_indirect
/// 

/// draws 2 meshes one with draw indirect and one witg draw indexed indirect.
/// no root binds are changed or buffers updated, this is just simply to test the execute indirect call
#[no_mangle]
pub fn draw_indirect(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indirect"
        ],
        render_graph: "mesh_draw_indirect",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_indirect(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {
    
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
        MeshComponent(tri.clone()),
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
}

/// Renders meshes indirectly in a basic way, we issues some execute indirect draw whit arguments pre-populated in a buffer
#[no_mangle]
pub fn draw_meshes_indirect(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_indirect_query: Query<(&WorldMatrix, &MeshComponent, &CommandSignatureComponent, &BufferComponent)>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
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

///
/// draw_indexed_vertex_buffer_instanced
/// 

/// Creates a instance batch, where the `InstanceBatch` parent will update a vertex buffer containing
/// it's child (instance) entities. The vertex shader layput steps the instance buffer per instance
#[no_mangle]
pub fn draw_indexed_vertex_buffer_instanced(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_vertex_buffer_instanced"
        ],
        update: systems![
            "rotate_meshes",
            "batch_world_matrix_instances"
        ],
        render_graph: "mesh_debug_vertex_buffer_instanced"
    }
}

#[no_mangle]
pub fn setup_draw_indexed_vertex_buffer_instanced(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_cube_mesh(&mut device.0),
        hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
    ];

    // square number of rows and columns
    let mut rng = rand::thread_rng();

    let size = 2.0;
    let num = 64;
    let instance_count = (num*num) as u32;
    let range = size * size * (num as f32);

    for mesh in meshes {
        let parent = commands.spawn(InstanceBatch {
            mesh: MeshComponent(mesh.clone()),
            pipeline: PipelineComponent("mesh_debug_vertex_buffer_instanced".to_string()),
            instance_buffer: InstanceBuffer { 
                buffer: device.create_buffer(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::VERTEX,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<Mat34f>(),
                    num_elements: instance_count as usize,
                    initial_state: gfx::ResourceState::VertexConstantBuffer
                }, hotline_rs::data![]).unwrap(),
                instance_count,
                heap: None
            }
        }).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised 
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                commands.spawn(Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                });
            }
        }
    }
}

/// Renders all scene instance batches with vertex instance buffer
#[no_mangle]
pub fn render_meshes_vertex_buffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        
        // bind the shader resource heap for t0 (if exists)
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
        if let Some(slot) = slot {
            view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
        }

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.set_vertex_buffer(&instance_batch.buffer, 1);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

///
/// draw_indexed_cbuffer_instanced
/// 

/// Creates a instance batch, where the `InstanceBatch` parent will update a cbuffer containing 
/// the cbuffer is created in a separate heap and the matrices and indexed into using the instance id system value semantic
#[no_mangle]
pub fn draw_indexed_cbuffer_instanced(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indexed_cbuffer_instanced"
        ],
        update: systems![
            "rotate_meshes",
            "batch_world_matrix_instances"
        ],
        render_graph: "mesh_debug_cbuffer_instanced"
    }
}

#[no_mangle]
pub fn setup_draw_indexed_cbuffer_instanced(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 3, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_prism_mesh(&mut device.0, 5, false, true, 1.0, 1.0),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
        hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 5, false, true),
    ];

    // square number of rows and columns
    let mut rng = rand::thread_rng();

    let size = 2.0;
    let num = 32; // max number of bytes in cbuffer is 65536
    let instance_count = (num*num) as u32;
    let range = size * size * (num as f32);

    for mesh in meshes {
        let mut heap = device.create_heap(&gfx::HeapInfo {
            heap_type: gfx::HeapType::Shader,
            num_descriptors: instance_count as usize
        });
        let parent = commands.spawn(InstanceBatch {
            mesh: MeshComponent(mesh.clone()),
            pipeline: PipelineComponent("mesh_debug_cbuffer_instanced".to_string()),
            instance_buffer: InstanceBuffer { 
                buffer: device.create_buffer_with_heap(&gfx::BufferInfo{
                    usage: gfx::BufferUsage::CONSTANT_BUFFER,
                    cpu_access: gfx::CpuAccessFlags::WRITE,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<Mat34f>(),
                    num_elements: instance_count as usize,
                    initial_state: gfx::ResourceState::VertexConstantBuffer
                }, hotline_rs::data![], &mut heap).unwrap(),
                instance_count,
                heap: Some(heap)
            }
        }).id();
        for _ in 0..num {
            for _ in 0..num {
                // spawn a bunch of entites with slightly randomised 
                let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * splat3f(range) * 2.0 - vec3f(range, 0.0, range);
                let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                commands.spawn(Instance {
                    pos: Position(pos),
                    rot: Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                    scale: Scale(splat3f(size)),
                    world_matrix: WorldMatrix(Mat34f::identity()),
                    parent: Parent(parent)
                });
            }
        }
    }
}

/// Renders all scene instance batches with cbuffer instance buffer
#[no_mangle]
pub fn render_meshes_cbuffer_instanced(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    instance_draw_query: Query<(&InstanceBuffer, &MeshComponent, &PipelineComponent)>
) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (instance_batch, mesh, pipeline) in &instance_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

        view.cmd_buf.set_render_heap(1, instance_batch.heap.as_ref().unwrap(), 0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);

        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, instance_batch.instance_count, 0, 0, 0);
    }

    Ok(())
}

/// 
/// draw_push_constants_texture
///

#[no_mangle]
pub fn draw_push_constants_texture(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_push_constants_texture"
        ],
        render_graph: "mesh_push_constants_texture",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_draw_push_constants_texture(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let sphere = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    let textures = [
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_albedo.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/metalgrid2_normal.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/bluechecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap()),
        TextureComponent(image::load_texture_from_file(&mut device, 
            &hotline_rs::get_data_path("textures/redchecker01.dds"), 
            Some(&mut pmfx.shader_heap)
        ).unwrap())
    ];

    // square number of rows and columns
    let rc = sqrt(textures.len() as f32);
    let irc = (rc + 0.5) as usize; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(textures[y * irc + x].get_srv_index().unwrap() as u32),
            ));
        }
    }

    
    // spawn entities to keep hold of textures
    for tex in textures {
        commands.spawn(
            tex
        );
    }

    // dbeug prims uvs
    let debug_uvs = false;
    if debug_uvs {
        let meshes = vec![
            hotline_rs::primitives::create_plane_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_mesh(&mut device.0),
            hotline_rs::primitives::create_sphere_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
            hotline_rs::primitives::create_cylinder_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_billboard_mesh(&mut device.0),
            hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
            hotline_rs::primitives::create_capsule_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
            hotline_rs::primitives::create_icosasphere_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_cube_subdivision_mesh(&mut device.0, 1),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 1.0, 1.0),
            hotline_rs::primitives::create_pyramid_mesh(&mut device.0, 4, false, true),
            hotline_rs::primitives::create_cone_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_tourus_mesh(&mut device.0, 16),
            hotline_rs::primitives::create_prism_mesh(&mut device.0, 4, false, true, 0.25, 0.5),
            hotline_rs::primitives::create_helix_mesh(&mut device.0, 16, 4),
        ];
    
        let uv_debug_tex = TextureComponent(image::load_texture_from_file(
            &mut device, 
            &hotline_rs::get_src_data_path("textures/blend_test_fg.png"),
            Some(&mut pmfx.shader_heap)
        ).unwrap());
    
        let rc = ceil(sqrt(meshes.len() as f32));
        let irc = (rc + 0.5) as usize; 
    
        let size = 10.0;
        let half_size = size * 0.5;    
        let step = size * half_size;
        let half_extent = (rc-1.0) * step * 0.5;
        let start_pos = vec3f(-half_extent, size, -half_extent);
    
        let mut i = 0;
        for y in 0..irc {
            for x in 0..irc {
                if i < meshes.len() {
                    let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                    commands.spawn((
                        MeshComponent(meshes[i].clone()),
                        Position(iter_pos),
                        Rotation(Quatf::identity()),
                        Scale(splat3f(size)),
                        WorldMatrix(Mat34f::identity()),
                        TextureInstance(uv_debug_tex.get_srv_index().unwrap() as u32),
                    ));
                    i += 1;
                }
            }
        }

        commands.spawn(
            uv_debug_tex
        );
    }
}

/// Renders all scene meshes with a material instance component, using push constants to push texture ids
#[no_mangle]
pub fn render_meshes_push_constants_texture(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    for (world_matrix, mesh, texture) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 1, 16, gfx::as_u8_slice(&texture.0));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///
/// draw_material
/// 

/// Creates instance batches for each mesh and makes an instanced draw call per mesh
/// entity id's for lookups are stored in vertex buffers
/// instance data is stored in a structured buffer (world matrix, material id?)
/// material data is stored in a structured buffer  
#[no_mangle]
pub fn draw_material(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/debug").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_material"
        ],
        update: systems![
            "rotate_meshes",
            "batch_material_instances",
            "batch_bindless_draw_data"
        ],
        render_graph: "mesh_material"
    }
}

fn load_material(
    device: &mut gfx_platform::Device,
    pmfx: &mut Pmfx<gfx_platform::Device>,
    dir: &str) -> Result<MaterialResources, hotline_rs::Error> {
    let maps = vec![
        "_albedo.dds",
        "_normal.dds",
        "_roughness.dds"
    ];

    let mut textures = Vec::new();
    for map in maps {
        let paths = std::fs::read_dir(dir).unwrap();
        let map_path = paths.into_iter()
            .filter(|p| p.as_ref().unwrap().file_name().to_string_lossy().ends_with(map))
            .map(|p| String::from(p.as_ref().unwrap().file_name().to_string_lossy()))
            .collect::<Vec<_>>();

        if !map_path.is_empty() {
            textures.push(
                image::load_texture_from_file(
                    device, 
                    &format!("{}/{}", dir, map_path[0]), 
                    Some(&mut pmfx.shader_heap)
                ).unwrap()
            );
        }
    }

    if textures.len() != 3 {
        return Err(hotline_rs::Error {
            msg: format!(
                "hotline_rs::ecs:: error: material '{}' does not contain enough maps ({}/3)", 
                dir,
                textures.len()
            )
        });
    }

    Ok(MaterialResources {
        albedo: textures.remove(0),
        normal: textures.remove(0),
        roughness: textures.remove(0)
    })
}

#[no_mangle]
pub fn setup_draw_material(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let meshes = vec![
        hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64),
        hotline_rs::primitives::create_chamfer_cube_mesh(&mut device.0, 0.4, 8),
        hotline_rs::primitives::create_sphere_mesh_truncated(&mut device.0, 16, 8, true),
        hotline_rs::primitives::create_teapot_mesh(&mut device.0, 8)
    ];

    let materials = vec![
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/angled-tiled-floor")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/antique-grate1")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/cracking-painted-asphalt")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/dirty-padded-leather")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/green-ceramic-tiles")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/office-carpet-fabric1")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/rusting-lined-metal2")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/simple-basket-weave")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/stone-block-wall")).unwrap(),
        load_material(&mut device, &mut pmfx.0, &hotline_rs::get_data_path("textures/pbr/worn-painted-cement")).unwrap()
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

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_itr as usize,
        extent_capacity: entity_itr as usize,
        material_capacity: materials.len(),
        ..Default::default()
    });

    pmfx.get_world_buffers_mut().material.write(0, &material_data);

    for material in materials {
        commands.spawn(material);
    }
}

///
/// draw_indirect_gpu_frustum_culling
/// 

// cpu 80ms, gpu 20ms
// 22503776 ia verts
// 7501392 ia primitives
// 13924796 vs invocations

// copy data to a uav buffer in shader

// struct of world matrix, local aabb?
// compute frustum cull + build uav
// aabb from meshes

// - CopyBufferRegion to clear the UAV counter
// - buffer counter passed to execute indirect

pub struct DrawIndirectArgs {
    pub vertex_buffer: gfx::VertexBufferView,
    pub index_buffer: gfx::IndexBufferView,
    pub draw_id: u32,
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
pub fn draw_indirect_gpu_frustum_culling(
    client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_draw_indirect_gpu_frustum_culling"
        ],
        update: systems![
            "swirling_meshes",
            "batch_bindless_draw_data"
        ],
        render_graph: "mesh_draw_indirect_culling"
    }
}

#[no_mangle]
pub fn setup_draw_indirect_gpu_frustum_culling(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

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

    let irc = 256;
    let size = 10.0;
    let frc = 1.0 / irc as f32;
    let mut rng = rand::thread_rng();
    let entity_count = irc * irc;

    let pipeline = pmfx.get_render_pipeline("mesh_test_indirect").unwrap();
    
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
                        slot: pipeline.get_descriptor_slot(1, gfx::DescriptorType::PushConstants).unwrap().slot,
                        offset: 0,
                        num_values: 1
                    }
                })
            },
            gfx::IndirectArgument{
                argument_type: gfx::IndirectArgumentType::DrawIndexed,
                arguments: None
            }
        ], 
        Some(pipeline)
    ).unwrap();

    let mut indirect_args = Vec::new();

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            let offset = rng.gen::<f32>() * 750.0;
            let mut iter_pos = vec3f(cos(x as f32 / frc), sin(y as f32 / frc), sin(x as f32 / frc)) * (1000.0 - offset);
            iter_pos.y += 1000.0;
            let imesh = mesh_dist.sample(&mut rng);
            let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::two_pi();
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
                draw_id: i,
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
    }, hotline_rs::data!(&indirect_args), &mut pmfx.shader_heap).unwrap();

    // dynamic buffer has a counter packed at the end
    let dynamic_buffer = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER | gfx::BufferUsage::UNORDERED_ACCESS | gfx::BufferUsage::APPEND_COUNTER,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<DrawIndirectArgs>(),
        initial_state: gfx::ResourceState::IndirectArgument,
        num_elements: indirect_args.len(),
    }, hotline_rs::data![], &mut pmfx.shader_heap).unwrap();

    let counter_reset = device.create_buffer_with_heap(&gfx::BufferInfo{
        usage: gfx::BufferUsage::NONE,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<u32>(),
        initial_state: gfx::ResourceState::CopySrc,
        num_elements: 1,
    }, hotline_rs::data![gfx::as_u8_slice(&0)], &mut pmfx.shader_heap).unwrap();

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        draw_capacity: entity_count as usize,
        extent_capacity: entity_count as usize,
        camera_capacity: 1 as usize, // main camera
        ..Default::default()
    });

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

    // keep hold of meshes
    for mesh in meshes {
        commands.spawn(
            MeshComponent(mesh.clone())
        );
    }
}

#[no_mangle]
pub fn swirling_meshes(
    time: Res<TimeRes>, 
    mut mesh_query: Query<(&mut Rotation, &mut Position)>) {

    let mut i = 0.0;
    for (mut rotation, mut position) in &mut mesh_query {
        rotation.0 *= Quat::from_euler_angles(0.0, f32::pi() * time.0.delta, 0.0);
        
        let pr = rotate_2d(position.0.xz(), time.accumulated * 0.0001);
        position.0.set_xz(pr);
        
        position.0.y += sin(time.accumulated + i) * 2.0;

        i += 1.0;
    }
}

#[no_mangle]
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
        pass.cmd_buf.set_compute_pipeline(&pipeline);

        // resource index info for looking up input (draw all info) / output (culled draw call info)
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            // output uav
            pass.cmd_buf.push_compute_constants(slot.slot, 1, 0, 
                gfx::as_u8_slice(&indirect_draw.dynamic_buffer.get_uav_index().unwrap()));

            // input srv
            pass.cmd_buf.push_compute_constants(slot.slot, 1, 1, 
                gfx::as_u8_slice(&indirect_draw.arg_buffer.get_srv_index().unwrap()));
        }
        
        // world buffer info to lookup matrices and aabb info
        let world_buffer_info = pmfx.get_world_buffer_info();
        let slot = pipeline.get_descriptor_slot(2, gfx::DescriptorType::PushConstants);
        if let Some(slot) = slot {
            // println!("{}", world_buffer_info.draw.index);
            pass.cmd_buf.push_compute_constants(
                slot.slot, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
        }

        // bind the heap for un-ordered access and srvs, it should be on the same slot
        let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::UnorderedAccess);
        if let Some(slot) = slot {
            pass.cmd_buf.set_compute_heap(slot.slot, &pmfx.shader_heap);
        }

        pass.cmd_buf.dispatch(
            gfx::Size3 {
                x: indirect_draw.max_count / pass.thread_count.x,
                y: pass.thread_count.y,
                z: pass.thread_count.z
            },
            pass.thread_count
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
pub fn draw_meshes_indirect_culling(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    indirect_draw_query: Query<&DrawIndirectComponent>) 
    -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    
    // bind the shader resource heap for t0 (if exists)
    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // bind the shader resource heap for t1 (if exists)
    let slot = pipeline.get_descriptor_slot(1, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // bind the world buffer info
    let world_buffer_info = pmfx.get_world_buffer_info();
    let slot = pipeline.get_descriptor_slot(2, gfx::DescriptorType::PushConstants);
    if let Some(slot) = slot {
        view.cmd_buf.push_render_constants(
            slot.slot, gfx::num_32bit_constants(&world_buffer_info), 0, gfx::as_u8_slice(&world_buffer_info));
    }

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

//
// test_raster_states
//

/// Test various combinations of different rasterizer states
#[no_mangle]
pub fn test_raster_states(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_raster_test"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "raster_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_raster_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    // tuple (pipeline name, mesh)
    let meshes = vec![
        ("cull_none", hotline_rs::primitives::create_billboard_mesh(&mut device.0)),
        ("cull_back", hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0)),
        ("cull_front", hotline_rs::primitives::create_cube_mesh(&mut device.0)),
        ("wireframe_overlay", hotline_rs::primitives::create_octahedron_mesh(&mut device.0)),
        // TODO: alpha to coverage
    ];

    // square number of rows and columns
    let rc = ceil(sqrt(meshes.len() as f32));
    let irc = (rc + 0.5) as i32; 
    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;

    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < meshes.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                commands.spawn((
                    MeshComponent(meshes[i].1.clone()),
                    Position(iter_pos),
                    Rotation(Quatf::from_euler_angles(0.0, 0.0, 0.0)),
                    Scale(splat3f(10.0)),
                    WorldMatrix(Mat34f::identity()),
                    PipelineComponent(meshes[i].0.to_string())
                ));
            }
            i = i + 1;
        }
    }
}

///
/// test_blend_states
/// 

/// Test various combinations of different blend states
#[no_mangle]
pub fn test_blend_states(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_blend_test"
        ],
        update: systems![
            "rotate_meshes"
        ],
        render_graph: "blend_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_blend_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    // tuple (pipeline name, mesh)
    let meshes = vec![
        hotline_rs::primitives::create_tetrahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_cube_mesh(&mut device.0),
        hotline_rs::primitives::create_octahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_dodecahedron_mesh(&mut device.0),
        hotline_rs::primitives::create_icosahedron_mesh(&mut device.0),
    ];

    let pipelines = vec![
        "blend_disabled",
        "blend_additive",
        "blend_alpha",
        "blend_min",
        "blend_subtract",
        "blend_rev_subtract",
        "blend_max"
    ];

    // square number of rows and columns
    let rc = ceil(sqrt(pipelines.len() as f32));
    let irc = (rc + 0.5) as i32; 
    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    let mut rng = rand::thread_rng();

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < pipelines.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                // spawn a bunch vof entites with slightly randomised 
                for _ in 0..32 {
                    let mesh : usize = rng.gen::<usize>() % meshes.len();
                    let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * vec3f(size, 50.0, size) - vec3f(half_size, 0.0, half_size);
                    let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                    let h = rng.gen();

                    let alpha = match i {
                        0 => 0.0, // blend disabled alpha should have no effect
                        1 => 0.1, // additive, will accumulate
                        2 => 0.5, // alpha blend, semi-transparent
                        3 => 0.0, // min blend semi transparent
                        _ => saturate(0.1 + rng.gen::<f32>())
                    };

                    let v = match i {
                        6 => 0.6, // blend max is darker
                        _ => 1.0
                    };

                    commands.spawn((
                        MeshComponent(meshes[mesh].clone()),
                        Position(iter_pos + pos),
                        Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                        Scale(splat3f(2.0)),
                        WorldMatrix(Mat34f::identity()),
                        PipelineComponent(pipelines[i].to_string()),
                        Colour(Vec4f::from((maths_rs::hsv_to_rgb(vec3f(h, 1.0, v)), alpha)))
                    ));
                }
            }
            i = i + 1;
        }
    }
}

/// Renders all scene meshes with a pipeline component, binding a new pipeline each draw with matrix + colour push constants
#[no_mangle]
pub fn render_meshes_pipeline_coloured(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &PipelineComponent, &Colour)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx;
    let fmt = view.pass.get_format_hash();
    let camera = pmfx.get_camera_constants(&view.camera)?;

    for (world_matrix, mesh, pipeline, colour) in &mesh_draw_query {
        // set pipeline per mesh
        let pipeline = pmfx.get_render_pipeline_for_format(&pipeline.0, fmt)?;
        view.cmd_buf.set_render_pipeline(&pipeline);
        view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 4, 12, &colour.0);

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

///
/// test_cubemap
/// 

/// Test cubemap loading (including mip-maps) and rendering
#[no_mangle]
pub fn test_cubemap(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_cubemap_test"
        ],
        render_graph: "cubemap_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_cubemap_test(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    // square number of rows and columns
    let rc = 3.0;
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = (rc-1.0) * step * 0.5;
    let start_pos = vec3f(-half_extent, size, -half_extent);

    let cubemap_filepath = hotline_rs::get_data_path("textures/cubemap.dds");
    let cubemap = image::load_texture_from_file(&mut device.0, &cubemap_filepath, Some(&mut pmfx.shader_heap)).unwrap();

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere_mesh.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(size)),
                WorldMatrix(Mat34f::identity()),
                TextureInstance(cubemap.get_srv_index().unwrap() as u32)
            ));
        }
    }

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(cubemap)
    );
}

/// Renders all scene meshes with a cubemap applied and samples the separate mip levels in the shader per entity
#[no_mangle]
pub fn render_meshes_cubemap_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

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

///
/// test_texture2d_array
/// 

/// Test texture2d_array loading, loads a dds texture2d_array generated from an image sequence
#[no_mangle]
pub fn test_texture2d_array(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_texture2d_array_test"
        ],
        update: systems![
            "animate_textures"
        ],
        render_graph: "texture2d_array_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_texture2d_array_test(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let billboard_mesh = hotline_rs::primitives::create_billboard_mesh(&mut device.0);

    let texture_array_filepath = hotline_rs::get_data_path("textures/bear.dds");

    let texture_array_info = image::load_from_file(&texture_array_filepath).unwrap();
    let texture_array = device.0.create_texture_with_heaps(
        &texture_array_info.info,
        gfx::TextureHeapInfo {
            shader: Some(&mut pmfx.shader_heap),
            ..Default::default()
        },
        Some(texture_array_info.data.as_slice())
    ).unwrap();
    let aspect = (texture_array_info.info.width / texture_array_info.info.height) as f32;
    let size = vec2f(20.0 * aspect, 20.0);

    let num_instances = 64;

    let mut rng = rand::thread_rng();
    let dist = rand::distributions::Uniform::from(-200..200);

    // randomly spawn some cylindrical billboards
    for _ in 0..num_instances {
        let mut pos = vec3f(
            dist.sample(&mut rng) as f32, 
            dist.sample(&mut rng) as f32, 
            dist.sample(&mut rng) as f32
        ) * vec3f(1.0, 0.0, 1.0);
        pos.y = size.y * 0.7;
        commands.spawn((
            MeshComponent(billboard_mesh.clone()),
            Position(pos),
            Rotation(Quatf::identity()),
            Scale(vec3f(size.x, size.y, size.x)),
            WorldMatrix(Mat34f::identity()),
            Billboard,
            CylindricalBillboard,
            TextureInstance(texture_array.get_srv_index().unwrap() as u32),
            TimeComponent(0.0),
            AnimatedTexture {
                frame: floor(rng.gen::<f32>() as f32 * texture_array_info.info.array_layers as f32) as u32,
                frame_count: texture_array_info.info.array_layers
            }
        ));
    }

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(texture_array)
    );
}

/// Renders a texture2d test passing the texture index and frame index to the shader for sampling along with a world matrix.
#[no_mangle]
pub fn render_meshes_texture2d_array_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance, &AnimatedTexture), With<CylindricalBillboard>>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    // spherical billboard
    let inv_rot = Mat3f::from(camera.view_matrix.transpose());
    let cyl_rot = Mat3f::new(
        inv_rot[0], 0.0, inv_rot[2],
        0.0, 1.0, 0.0,
        inv_rot[6], 0.0, inv_rot[8],
    );

    for (world_matrix, mesh, texture, animated_texture) in &mesh_query {
        let bbmat = world_matrix.0 * Mat4f::from(cyl_rot);
        view.cmd_buf.push_render_constants(1, 12, 0, &bbmat);
        view.cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[texture.0, animated_texture.frame, 0, 0]));

        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

#[no_mangle]
pub fn animate_textures(
    time: Res<TimeRes>,
    mut animated_texture_query: Query<(&mut AnimatedTexture, &mut TimeComponent)>) {
    let frame_length = 1.0 / 24.0;
    for (mut animated_texture, mut timer) in &mut animated_texture_query {
        timer.0 += time.0.delta;
        if timer.0 > frame_length {
            timer.0 = 0.0;
            animated_texture.frame = (animated_texture.frame + 1) % animated_texture.frame_count;
        }
    }
}

///
/// test_texture3d
///

/// Test 3d texture loading and rendering using a pre-built sdf texture
#[no_mangle]
pub fn test_texture3d(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_texture3d_test"
        ],
        render_graph: "texture3d_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_texture3d_test(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);

    let volume_info = image::load_from_file(&hotline_rs::get_data_path("textures/sdf_shadow.dds")).unwrap();
    let volume = device.0.create_texture_with_heaps(
        &volume_info.info,
        gfx::TextureHeapInfo {
            shader: Some(&mut pmfx.shader_heap),
            ..Default::default()
        },
        Some(volume_info.data.as_slice())
    ).unwrap();

    let dim = 50.0;

    commands.spawn((
        MeshComponent(cube_mesh.clone()),
        Position(vec3f(0.0, dim * 0.5, 0.0)),
        Rotation(Quatf::identity()),
        Scale(splat3f(dim)),
        WorldMatrix(Mat34f::identity()),
        TextureInstance(volume.get_srv_index().unwrap() as u32)
    ));

    // spawn entity to keep hold of the texture
    commands.spawn(
        TextureComponent(volume)
    );
}

/// Renders a texture3d test from a loaded (pre-generated signed distance field), the shader ray marches the volume
#[no_mangle]
pub fn render_meshes_texture3d_test(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent, &TextureInstance)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let pipeline = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    view.cmd_buf.set_render_pipeline(&pipeline);
    view.cmd_buf.push_render_constants(0, 16, 0, gfx::as_u8_slice(&camera.view_projection_matrix));
    view.cmd_buf.push_render_constants(0, 4, 16, gfx::as_u8_slice(&camera.view_position));

    let slot = pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    if let Some(slot) = slot {
        view.cmd_buf.set_render_heap(slot.slot, &pmfx.shader_heap, 0);
    }

    for (world_matrix, mesh, tex) in &mesh_draw_query {
        view.cmd_buf.push_render_constants(1, 12, 0, &world_matrix.0);
        view.cmd_buf.push_render_constants(1, 2, 16, gfx::as_u8_slice(&[tex.0, 0, 0, 0]));
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    Ok(())
}

/// Test compute shader by reading and writing from a 3d texture un-ordered access
#[no_mangle]
pub fn test_compute(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_compute_test"
        ],
        render_graph: "compute_test",
        ..Default::default()
    }
}

#[no_mangle]
pub fn setup_compute_test(
    mut device: ResMut<DeviceRes>,
    pmfx: ResMut<PmfxRes>,
    mut commands: Commands) {

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);

    let srv = pmfx.get_texture("compute_texture3d").unwrap().get_srv_index().unwrap() as u32;

    let dim = 50.0;
    commands.spawn((
        MeshComponent(cube_mesh.clone()),
        Position(vec3f(0.0, dim * 0.5, 0.0)),
        Rotation(Quatf::identity()),
        Scale(splat3f(dim)),
        WorldMatrix(Mat34f::identity()),
        TextureInstance(srv)
    ));
}

/// Tests missing setup and updates are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_systems(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {   
    ScheduleInfo {
        setup: systems![
            "missing"
        ],
        update: systems![
            "missing"
        ],
        render_graph: "mesh_debug",
        ..Default::default()
    }
}

/// Tests missing render graphs are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_render_graph(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    ScheduleInfo {
        setup: systems![
            "setup_cube"
        ],
        render_graph: "missing",
        ..Default::default()
    }
}

/// Tests missing view specified in the render graph
#[no_mangle]
pub fn test_missing_view(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_view",
        ..Default::default()
    }
}

/// Tests case where render graph fails, in this case it is missing a pipeline, but the pipeline can also fail to build depending on the src data
#[no_mangle]
pub fn test_failing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_pipeline",
        ..Default::default()
    }
}

/// Tests missing pipeline specified in the render graph
#[no_mangle]
pub fn test_missing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_pipeline",
        ..Default::default()
    }
}

/// Tests missing camera specified in the render graph
#[no_mangle]
pub fn test_missing_camera(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_camera",
        ..Default::default()
    }
}

/// Tests missing view_function (system) specified in the render graph
#[no_mangle]
pub fn test_missing_view_function(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_function",
        ..Default::default()
    }
}

#[no_mangle]
pub fn render_missing_camera(
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    _: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
    pmfx.get_camera_constants("missing")?;
    Ok(())
}

#[no_mangle]
pub fn render_missing_pipeline(
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
    let fmt = view.pass.get_format_hash();
    pmfx.get_render_pipeline_for_format("missing", fmt)?;
    Ok(())
}