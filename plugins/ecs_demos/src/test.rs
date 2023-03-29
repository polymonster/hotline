// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use rand::prelude::*;
use bevy_ecs::prelude::*;

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

/// Sets up a few primitives with designated pipelines to verify raster state conformance
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
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size, -half_extent * 4.0);

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
                    Pipeline(meshes[i].0.to_string())
                ));
            }
            i = i + 1;
        }
    }
}

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

/// Sets up a few primitives with designated pipelines to verify raster state conformance
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
    let half_extent = rc * half_size;
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
                    let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * vec3f(10.0, 50.0, 10.0);
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
                        Pipeline(pipelines[i].to_string()),
                        Colour(Vec4f::from((maths_rs::hsv_to_rgb(vec3f(h, 1.0, v)), alpha)))
                    ));
                }
            }
            i = i + 1;
        }
    }
}

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

/// Sets up a few spheres, to render cubemap mip-levels
#[no_mangle]
pub fn setup_cubemap_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let sphere_mesh = hotline_rs::primitives::create_sphere_mesh(&mut device.0, 64);

    // square number of rows and columns
    let rc = 3.0;
    let irc = (rc + 0.5) as i32; 

    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size * 1.8, -half_extent * 4.0);

    let cubemap_filepath = hotline_rs::get_data_path("textures/cubemap.dds");
    let cubemap = image::load_texture_from_file(&mut device.0, &cubemap_filepath).unwrap();

    for y in 0..irc {
        for x in 0..irc {
            let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
            commands.spawn((
                MeshComponent(sphere_mesh.clone()),
                Position(iter_pos),
                Rotation(Quatf::identity()),
                Scale(splat3f(10.0)),
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

/// Test cubemap loading (including mip-maps) and rendering
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

/// Sets up a few spheres, to render cubemap mip-levels
#[no_mangle]
pub fn setup_texture2d_array_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    let billboard_mesh = hotline_rs::primitives::create_billboard_mesh(&mut device.0);

    let texture_array_filepath = hotline_rs::get_data_path("textures/bear.dds");

    let texture_array_info = image::load_from_file(&texture_array_filepath).unwrap();
    let texture_array = device.0.create_texture(&texture_array_info.info, Some(texture_array_info.data.as_slice())).unwrap();
    let aspect = (texture_array_info.info.width / texture_array_info.info.height) as f32;
    let size = vec2f(20.0 * aspect, 20.0);

    let num_instances = 32;
    let range = vec3f(200.0, 0.0, 200.0);
    let mut rng = rand::thread_rng();

    // randomly spawn some cylindrical billboards
    for _ in 0..num_instances {
        let mut pos = (vec3f(rng.gen(), rng.gen(), rng.gen()) * (range * 2.0)) - range;
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

/// Test cubemap loading (including mip-maps) and rendering
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

/// Sets up a few spheres, to render cubemap mip-levels
#[no_mangle]
pub fn setup_texture3d_test(
    mut device: ResMut<DeviceRes>,
    mut commands: Commands) {

    println!("setup_texture3d_test!");

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);

    let volume_info = image::load_from_file(&hotline_rs::get_data_path("textures/sdf_shadow.dds")).unwrap();
    let volume = device.0.create_texture(&volume_info.info, Some(volume_info.data.as_slice())).unwrap();

    let dim = 50.0;

    commands.spawn((
        MeshComponent(cube_mesh.clone()),
        Position(vec3f(0.0, dim, 0.0)),
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
    _device: &bevy_ecs::prelude::Res<DeviceRes>,
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    _: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
    pmfx.get_camera_constants("missing")?;
    Ok(())
}

#[no_mangle]
pub fn render_missing_pipeline(
    _device: &bevy_ecs::prelude::Res<DeviceRes>,
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
    let fmt = view.pass.get_format_hash();
    pmfx.get_render_pipeline_for_format("missing", fmt)?;
    Ok(())
}