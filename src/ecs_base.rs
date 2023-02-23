// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::{client, pmfx, imdraw, gfx_platform, os_platform};

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use maths_rs::{Vec3f, Mat4f};

pub struct ScheduleInfo {
    pub setup: Vec<String>,
    pub update: Vec<String>,
    pub render_graph: String
}

/// Empty schedule info
impl Default for ScheduleInfo {
    fn default() -> ScheduleInfo {
        ScheduleInfo {
            setup: Vec::new(),
            update: Vec::new(),
            render_graph: String::new(),
        }
    }
}

/// Serialisable camera info
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct CameraInfo {
    pub pos: (f32, f32, f32),
    pub rot: (f32, f32, f32),
    pub aspect: f32,
    pub fov: f32,
}

/// Sensible default values for a 3D perspective camera looking downward in the y-axis to an xz-plane
impl Default for CameraInfo {
    fn default() -> CameraInfo {
        CameraInfo {
            pos: (0.0, 100.0, 0.0),
            rot: (-45.0, 0.0, 0.0),
            aspect: 16.0/9.0,
            fov: 60.0
        }
    }
}

/// Seriablisable user info for maintaining state between reloads and sessions
#[derive(Serialize, Deserialize, Default, Resource, Clone)]
pub struct SessionInfo {
    pub active_demo: String,
    pub main_camera: Option<CameraInfo>
}

//
// Stages
//

#[derive(StageLabel)]
pub struct StageStartup;

#[derive(StageLabel)]
pub struct StageUpdate;

#[derive(StageLabel)]
pub struct StageRender;

//
// Resources
//

#[derive(Resource)]
pub struct DeviceRes(pub gfx_platform::Device);

#[derive(Resource)]
pub struct AppRes(pub os_platform::App);

#[derive(Resource)]
pub struct MainWindowRes(pub os_platform::Window);

#[derive(Resource)]
pub struct PmfxRes(pub pmfx::Pmfx<gfx_platform::Device>);

#[derive(Resource)]
pub struct ImDrawRes(pub imdraw::ImDraw<gfx_platform::Device>);

#[derive(Resource)]
pub struct UserConfigRes(pub client::UserConfig);

//
// Components
//

#[derive(Component)]
pub struct Position(pub Vec3f);

#[derive(Component)]
pub struct Velocity(pub Vec3f);

#[derive(Component)]
pub struct WorldMatrix(pub Mat4f);

#[derive(Component)]
pub struct Rotation(pub Vec3f);

#[derive(Component)]
pub struct ViewProjectionMatrix(pub Mat4f);

#[derive(Component)]
pub struct Camera;

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct MeshComponent(pub pmfx::Mesh<gfx_platform::Device>);

#[macro_export]
macro_rules! system_func {
    ($func:expr) => {
        Some($func.into_descriptor())
    }
}

#[macro_export]
macro_rules! render_func {
    ($func:expr, $view:expr) => {
        Some(render_func_closure![$func, $view].into_descriptor())
    }
}

#[macro_export]
macro_rules! render_func_closure {
    ($func:expr, $view:expr) => {
        move |
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>,
            qmesh: Query::<(&WorldMatrix, &MeshComponent)>| {
                $func(
                    pmfx,
                    $view.to_string(),
                    qvp,
                    qmesh
                );
        }
    }
}