// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use bevy_ecs::prelude::*;

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

    let slot = pipeline.get_heap_slot(0, gfx::DescriptorType::ShaderResource);
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

    pmfx.resize_world_buffers(&mut device, pmfx::WorldBufferResizeInfo{
        directinal_light_count: num_lights,
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