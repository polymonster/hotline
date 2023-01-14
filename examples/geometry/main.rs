use hotline_rs::*;

use gfx::CmdBuf;
use gfx::SwapChain;

use os::App;
use os::Window;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

use primitives;

use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;

//use std::collections::HashMap;

//
// Components
//

#[derive(Component)]
struct Position(Vec3f);

#[derive(Component)]
struct Velocity(Vec3f);

#[derive(Component)]
struct WorldMatrix(Mat4f);

#[derive(Component)]
struct Rotation(Vec3f);

#[derive(Component)]
struct ViewProjectionMatrix(Mat4f);

#[derive(Component)]
struct Camera;

#[derive(Component)]
struct MeshComponent(pmfx::Mesh<gfx_platform::Device>);

//
// Stages
//

#[derive(StageLabel)]
pub struct StageUpdate;

#[derive(StageLabel)]
pub struct StageRender;

//
// Resources
//

#[derive(Resource)]
struct HotlineResource<T> {
    res: T
}

type ImDrawRes = HotlineResource::<imdraw::ImDraw<gfx_platform::Device>>;
type PmfxRes = HotlineResource::<pmfx::Pmfx<gfx_platform::Device>>;
type DeviceRes = HotlineResource::<gfx_platform::Device>;
type AppRes = HotlineResource::<os_platform::App>;
type MainWindowRes = HotlineResource::<os_platform::Window>;
type ImGuiRes = HotlineResource::<imgui::ImGui::<gfx_platform::Device, os_platform::App>>;


fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.0 += velocity.0;
    }
}

fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>, 
    mut query: Query<(&mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.res;
    for (mut position, mut rotation, mut view_proj) in &mut query {

        if main_window.res.is_focused() {
            // get keyboard position movement
            let keys = app.get_keys_down();
            let mut cam_move_delta = Vec3f::zero();
            if keys['A' as usize] {
                cam_move_delta.x -= 1.0;
            }
            if keys['D' as usize] {
                cam_move_delta.x += 1.0;
            }
            if keys['Q' as usize] {
                cam_move_delta.y -= 1.0;
            }
            if keys['E' as usize] {
                cam_move_delta.y += 1.0;
            }
            if keys['W' as usize] {
                cam_move_delta.z -= 1.0;
            }
            if keys['S' as usize] {
                cam_move_delta.z += 1.0;
            }

            // get mouse rotation
            if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
                let mouse_delta = app.get_mouse_pos_delta();
                rotation.0.x -= mouse_delta.y as f32;
                rotation.0.y -= mouse_delta.x as f32;
            }

            // construct rotation matrix
            let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rotation.0.x));
            let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rotation.0.y));
            let mat_rot = mat_rot_y * mat_rot_x;

            // move relative to facing directions
            position.0 += mat_rot * cam_move_delta;
        }

        // generate proj matrix
        let window_rect = main_window.res.get_viewport_rect();
        let aspect = window_rect.width as f32 / window_rect.height as f32;
        let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 0.001, 10000.0);

        // construct rotation matrix
        let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(rotation.0.x));
        let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(rotation.0.y));
        let mat_rot = mat_rot_y * mat_rot_x;

        // build view / proj matrix
        let translate = Mat4f::from_translation(position.0);
        let view = translate * mat_rot;
        let view = view.inverse();
       
        // assign view proj
        view_proj.0 = proj * view;
    }
}

fn render_grid(
    mut device: ResMut<DeviceRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: Res<PmfxRes>,
    view_name: String,
    mut query: Query<&ViewProjectionMatrix> ) {

    let arc_view = pmfx.res.get_view(&view_name).unwrap();
    let mut view = arc_view.lock().unwrap();
    let bb = view.cmd_buf.get_backbuffer_index();

    // reset and transition
    let rt = pmfx.res.get_texture("main_colour").unwrap();
    view.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(rt),
        buffer: None,
        state_before: gfx::ResourceState::ShaderResource,
        state_after: gfx::ResourceState::RenderTarget,
    });

    for view_proj in &mut query {
        // render grid
        let imdraw = &mut imdraw.res;
        let pmfx = &pmfx.res;

        let scale = 1000.0;
        let divisions = 10.0;
        for i in 0..((scale * 2.0) /divisions) as usize {
            let offset = -scale + i as f32 * divisions;
            imdraw.add_line_3d(Vec3f::new(offset, 0.0, -scale), Vec3f::new(offset, 0.0, scale), Vec4f::from(0.3));
            imdraw.add_line_3d(Vec3f::new(-scale, 0.0, offset), Vec3f::new(scale, 0.0, offset), Vec4f::from(0.3));
        }

        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 0.0, 1000.0), Vec4f::blue());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(1000.0, 0.0, 0.0), Vec4f::red());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 1000.0, 0.0), Vec4f::green());

        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, 0.0, -1000.0), Vec4f::yellow());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(-1000.0, 0.0, 0.0), Vec4f::cyan());
        imdraw.add_line_3d(Vec3f::zero(), Vec3f::new(0.0, -1000.0, 0.0), Vec4f::magenta());

        imdraw.submit(&mut device.res, bb as usize).unwrap();

        view.cmd_buf.begin_render_pass(&view.pass);
        view.cmd_buf.set_viewport(&view.viewport);
        view.cmd_buf.set_scissor_rect(&view.scissor_rect);

        view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_3d").unwrap());
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);

        imdraw.draw_3d(&mut view.cmd_buf, bb as usize);

        view.cmd_buf.end_render_pass();
    }

    let rt = pmfx.res.get_texture("main_colour").unwrap();
    view.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(rt),
        buffer: None,
        state_before: gfx::ResourceState::RenderTarget,
        state_after: gfx::ResourceState::ShaderResource,
    });
}

fn render_world_view(
    pmfx: Res<PmfxRes>,
    view_name: String,
    view_proj_query: Query<&ViewProjectionMatrix>,
    mesh_draw_query: Query<(&WorldMatrix, &MeshComponent)>) {
    
    // unpack
    let pmfx = &pmfx.res;

    let arc_view = pmfx.get_view(&view_name).unwrap();
    let mut view = arc_view.lock().unwrap();

    let rt = pmfx.get_texture("main_colour").unwrap();
    view.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(rt),
        buffer: None,
        state_before: gfx::ResourceState::ShaderResource,
        state_after: gfx::ResourceState::RenderTarget,
    });

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);
    view.cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_mesh").unwrap());

    for view_proj in &view_proj_query {
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);
        for (world_matrix, mesh) in &mesh_draw_query {
            // draw

            let mm = world_matrix.0 * vec4f(1.0, 1.0, 1.0, 1.0);
            let pm = view_proj.0 * mm;
            println!("depth = {}", pm.z / pm.w);


            view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
            view.cmd_buf.set_index_buffer(&mesh.0.ib);
            view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
            view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        }
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();

    let rt = pmfx.get_texture("main_colour").unwrap();
    view.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
        texture: Some(rt),
        buffer: None,
        state_before: gfx::ResourceState::RenderTarget,
        state_after: gfx::ResourceState::ShaderResource,
    });
}

fn main() -> Result<(), hotline_rs::Error> {    

    //
    // create context
    //

    let mut ctx : Context<gfx_platform::Device, os_platform::App> = Context::create(HotlineInfo {
        ..Default::default()
    })?;

    //
    // create pmfx
    //

    ctx.pmfx.load(&hotline_rs::get_asset_path("data/shaders/imdraw").as_str())?;

    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_2d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_3d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_mesh", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_blit", ctx.swap_chain.get_backbuffer_pass())?;

    ctx.pmfx.create_graph(& mut ctx.device, "forward")?;

    //
    // main loop
    //

    let mut world = World::new();

    let cube_mesh = primitives::create_cube_mesh(&mut ctx.device);

    world.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: cube_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_translation(Vec3f::zero()) }
    ));

    world.spawn((
        Position { 0: Vec3f::zero() },
        Velocity { 0: Vec3f::one() },
        MeshComponent {0: cube_mesh.clone()},
        WorldMatrix { 0: Mat4f::from_translation(Vec3f::unit_z() * 2.5) }
    ));
    
    world.spawn((
        Position { 0: Vec3f::new(0.0, 100.0, 0.0) },
        Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ViewProjectionMatrix { 0: Mat4f::identity()},
        Camera,
    ));

    let mut schedule = Schedule::default();

    schedule.add_stage(StageUpdate, SystemStage::parallel()
        .with_system(update_cameras)
        .with_system(movement)
    );

    // create a "constructor" closure, which can initialize
    // our data and move it into a closure that bevy can run as a system
    let view_constructor = |view: String| {
        move |
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>,
            qmesh: Query::<(&WorldMatrix, &MeshComponent)>| {
                render_world_view(
                    pmfx,
                    view.to_string(),
                    qvp,
                    qmesh
                );
        }
    };

    let grid_constructor = || {
        move |
            device: ResMut<DeviceRes>,
            imdraw: ResMut<ImDrawRes>,
            pmfx: Res<PmfxRes>,
            qvp: Query<&ViewProjectionMatrix>| {
                render_grid(
                    device,
                    imdraw,
                    pmfx,
                    "render_grid".to_string(),
                    qvp,
                );
        }
    };
    // end temp

    schedule.add_stage(StageRender, SystemStage::single_threaded()
        .with_system_set(
            SystemSet::new().label("render_debug_3d")
            .with_system(grid_constructor())
        )
        .with_system_set(
            SystemSet::new().label("render_main")
            .with_system(view_constructor("render_world_view".to_string()))
        )
    );

    while ctx.app.run() {

        ctx.new_frame();

        // move hotline resource into world
        world.insert_resource(AppRes {res: ctx.app});
        world.insert_resource(MainWindowRes {res: ctx.main_window});
        world.insert_resource(DeviceRes {res: ctx.device});
        world.insert_resource(ImDrawRes {res: ctx.imdraw});
        world.insert_resource(PmfxRes {res: ctx.pmfx});
        world.insert_resource(ImGuiRes {res: ctx.imgui});

        // run systems
        schedule.run(&mut world);
    
        // move resources back out
        ctx.app = world.remove_resource::<AppRes>().unwrap().res;
        ctx.main_window = world.remove_resource::<MainWindowRes>().unwrap().res;
        ctx.device = world.remove_resource::<DeviceRes>().unwrap().res;
        ctx.imdraw = world.remove_resource::<ImDrawRes>().unwrap().res;
        ctx.pmfx = world.remove_resource::<PmfxRes>().unwrap().res;
        ctx.imgui = world.remove_resource::<ImGuiRes>().unwrap().res;

        // present to back buffer
        ctx.present();
    }

    ctx.wait_for_last_frame();

    // exited with code 0
    Ok(())
}