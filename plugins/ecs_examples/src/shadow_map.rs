// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

///
/// Shadow Map
/// 

use crate::prelude::*;
use hotline_rs::pmfx::CameraConstants;

pub fn get_aabb_corners(min_extent: Vec3f, max_extent: Vec3f) -> Vec<Vec3f> {
    let offsets = [
        Vec3f::zero(),
        Vec3f::one(),
        Vec3f::unit_x(),
        Vec3f::unit_y(),
        Vec3f::unit_z(),
        Vec3f::new(1.0, 0.0, 1.0),
        Vec3f::new(1.0, 1.0, 0.0),
        Vec3f::new(0.0, 1.0, 1.0)
    ];

    let size = max_extent - min_extent;
    offsets.iter().map(|offset| min_extent + offset * size).collect()
}

pub fn fit_shadow_camera_to_extents(light_dir: Vec3f, min_extent: Vec3f, max_extent: Vec3f) -> CameraConstants {
    let right = cross(light_dir, Vec3f::unit_y());
    let up = cross(right, light_dir);

    let emin = min_extent;
    let emax = max_extent;
    let corners = get_aabb_corners(emin, emax);

    let view = Mat34f::from((
        Vec4f::from((right, 0.0)),
        Vec4f::from((up, 0.0)),
        Vec4f::from((light_dir, 0.0))
    ));

    let view = Mat4f::from((
        Vec4f::from((right, 0.0)),
        Vec4f::from((up, 0.0)),
        Vec4f::from((-light_dir, 0.0)),
        Vec4f::from((0.0, 0.0, 0.0, 1.0)),
    ));

    /*
    let (cmin, cmax) = corners
        .iter()
        .fold((Vec3f::max_value(), -Vec3f::max_value()), |acc, x|
    );
    */

    let mut cmin = Vec3f::max_value();
    let mut cmax =-Vec3f::max_value();

    for corner in corners {
        let mut p = view * corner;
        p.z *= -1.0;
        (cmin, cmax) = min_max(p, (cmin, cmax));
    }

    let proj = Mat4f::create_ortho_matrix(cmin.x, cmax.x, cmin.y, cmax.y, cmin.z, cmax.z).transpose();

    CameraConstants {
        view_matrix: view,
        view_projection_matrix: proj * view,
        view_position: Vec4f::from(((cmin + cmax) * 0.5, 0.0))
    }
}

/// Setup multiple draw calls with draw indexed and per draw call push constants for transformation matrix etc.
#[no_mangle]
pub fn shadow_map(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    client.pmfx.load(hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_shadow_map"
        ],
        update: systems![
            "batch_lights",
            "batch_shadow_matrices"
        ],
        render_graph: "mesh_lit_single_shadow_map",
        ..Default::default()
    }
}

#[no_mangle]
#[export_update_fn]
pub fn setup_shadow_map(
    mut device: ResMut<DeviceRes>,
    mut pmfx: ResMut<PmfxRes>,
    mut commands: Commands) -> Result<(), hotline_rs::Error> {

    let hex_mesh = hotline_rs::primitives::create_prism_mesh(&mut device.0, 6, false, true, 1.0, 1.0);
    
    let dim = 128;
    let dim2 = dim / 2;
    let tile_size = 5.0;

    let half_extent = dim as f32 * tile_size;

    let sm = pmfx.get_texture("single_shadow_map").unwrap();

    // directional light
    let light_dir = normalize(vec3f(0.5, -0.5, 0.5));
    commands.spawn((
        Position(Vec3f::zero()),
        Colour(vec4f(0.5, 0.25, 0.125, 1.0)),
        LightComponent {
            light_type: LightType::Directional,
            direction: light_dir,
            shadow_map_info: pmfx::ShadowMapInfo {
                srv_index: sm.get_srv_index().unwrap() as u32,
                matrix_index: 0
            },
            ..Default::default()
        }
    ));

    // shadow map camera
    let extents3 = vec3f(half_extent, tile_size * 10.0, half_extent);
    let cam_constants = fit_shadow_camera_to_extents(light_dir, -extents3, extents3);
    
    pmfx.update_camera_constants("single_shadow_map_camera", &cam_constants);

    let shadow_cam = Camera {
        rot: Vec3f::zero(),
        focus: Vec3f::zero(),
        zoom: 0.0,
        camera_type: CameraType::None
    };

    commands.spawn((
        ViewProjectionMatrix(cam_constants.view_projection_matrix),
        Position(cam_constants.view_position.xyz()),
        shadow_cam,
        Name(String::from("single_shadow_map_camera"))
    ));

    pmfx.reserve_world_buffers(&mut device, WorldBufferReserveInfo {
        directional_light_capacity: 1,
        shadow_matrix_capacity: 1,
        ..Default::default()
    });

    let start = vec3f(-half_extent, 0.0, -half_extent);
    let mut pos = start;

    let mut rng = rand::thread_rng();

    let dist = rand::distributions::Uniform::from(tile_size..tile_size * 10.0);

    for y in 0..dim {    
        pos.x = start.x;
        for x in 0..dim {

            let h = dist.sample(&mut rng) as f32;

            commands.spawn((
                Position(pos),
                Scale(vec3f(tile_size, h, tile_size)),
                Rotation(Quatf::identity()),
                MeshComponent(hex_mesh.clone()),
                WorldMatrix(Mat34f::identity())
            ));

            pos.x += tile_size * 2.0;
        }

        pos.z += tile_size * 2.0
    }

    Ok(())
}

#[no_mangle]
#[export_update_fn]
pub fn batch_shadow_matrices(mut pmfx: ResMut<PmfxRes>) -> Result<(), hotline_rs::Error> {
    pmfx.get_world_buffers_mut().shadow_matrix.clear();
    
    let cam_constants = pmfx.get_camera_constants("single_shadow_map_camera")?.clone();
    pmfx.get_world_buffers_mut().shadow_matrix.push(&cam_constants.view_projection_matrix);

    Ok(())
}