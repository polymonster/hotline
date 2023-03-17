// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;
use rand::prelude::*;

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
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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

/// Test various combinations of different rasterizer states
#[no_mangle]
pub fn test_raster_states(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

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
                    WorldMatrix(Mat4f::identity()),
                    Pipeline(meshes[i].0.to_string())
                ));
            }
            i = i + 1;
        }
    }
}

/// Sets up a few primitives with designated pipelines to verify raster state conformance
#[no_mangle]
pub fn setup_blend_test(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

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
        "blend_subtract",
    ];

    // square number of rows and columns
    let rc = ceil(sqrt(pipelines.len() as f32));
    let irc = (rc + 0.5) as i32; 
    let size = 10.0;
    let half_size = size * 0.5;    
    let step = size * half_size;
    let half_extent = rc * half_size;
    let start_pos = vec3f(-half_extent * 4.0, size, -half_extent * 4.0);

    let mut rng = rand::thread_rng();

    let mut i = 0;
    for y in 0..irc {
        for x in 0..irc {
            if i < pipelines.len() {
                let iter_pos = start_pos + vec3f(x as f32 * step, 0.0, y as f32 * step);
                // spawn a bunch vof entites with slightly randomised 
                for _ in 0..32 {
                    let mesh : usize = rng.gen::<usize>() % meshes.len();
                    let pos = vec3f(rng.gen(), rng.gen(), rng.gen()) * vec3f(15.0, 50.0, 15.0) + vec3f(2.0, 0.0, 2.0);
                    let rot = vec3f(rng.gen(), rng.gen(), rng.gen()) * f32::pi() * 2.0;
                    commands.spawn((
                        MeshComponent(meshes[mesh].clone()),
                        Position(iter_pos + pos),
                        Rotation(Quatf::from_euler_angles(rot.x, rot.y, rot.z)),
                        Scale(splat3f(2.0)),
                        WorldMatrix(Mat4f::identity()),
                        Pipeline(pipelines[i].to_string())
                    ));
                }
            }
            i = i + 1;
        }
    }
}


/// Test various combinations of different blend states
#[no_mangle]
pub fn test_blend_states(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
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

