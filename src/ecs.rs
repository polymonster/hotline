use crate::*;

use maths_rs::Vec3f;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;

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
pub struct ImGuiRes(pub imgui::ImGui::<gfx_platform::Device, os_platform::App>);

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
pub struct MeshComponent(pub pmfx::Mesh<gfx_platform::Device>);


pub fn add_world_resources(world: &mut World, ctx: Context<gfx_platform::Device, os_platform::App>) {
    // move hotline resource into world
    world.insert_resource(DeviceRes {0: ctx.device});
    world.insert_resource(AppRes {0: ctx.app});
    world.insert_resource(MainWindowRes {0: ctx.main_window});
    world.insert_resource(PmfxRes {0: ctx.pmfx});
    world.insert_resource(ImDrawRes {0: ctx.imdraw});
    world.insert_resource(ImGuiRes {0: ctx.imgui});
}

pub fn remove_world_resources(world: &mut World, mut ctx: Context<gfx_platform::Device, os_platform::App>)  
    -> Context<gfx_platform::Device, os_platform::App> {
    // move resources back out into ctx
    ctx.device = world.remove_resource::<DeviceRes>().unwrap().0;
    ctx.app = world.remove_resource::<AppRes>().unwrap().0;
    ctx.main_window = world.remove_resource::<MainWindowRes>().unwrap().0;
    ctx.pmfx = world.remove_resource::<PmfxRes>().unwrap().0;
    ctx.imdraw = world.remove_resource::<ImDrawRes>().unwrap().0;
    ctx.imgui = world.remove_resource::<ImGuiRes>().unwrap().0;
    ctx
}