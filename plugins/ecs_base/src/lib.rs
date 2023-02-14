// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use bevy_ecs::prelude::*;

use hotline_rs::client;
use hotline_rs::pmfx;
use hotline_rs::imdraw;
use hotline_rs::gfx_platform;
use hotline_rs::os_platform;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

pub struct SheduleInfo {
    pub setup: Vec<String>,
    pub update: Vec<String>,
    pub render: Vec<String>
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
macro_rules! view_func {
    ($func:expr, $view:literal) => {
        Some(view_func_closure![$func, $view].into_descriptor())
    }
}

#[macro_export]
macro_rules! view_func_closure {
    ($func:expr, $view:literal) => {
        move |
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>,
            qmesh: Query::<(&WorldMatrix, &MeshComponent)>| {
                $func(
                    pmfx,
                    "render_world_view".to_string(),
                    qvp,
                    qmesh
                );
        }
    }
}