use hotline_rs::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

use maths_rs::Vec2f;
use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

use bevy_ecs::prelude::*;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

#[derive(Component)]
struct Position {
    pos: Vec3f 
}

#[derive(Component)]
struct Velocity {
    vel: Vec3f 
}

#[derive(Component)]
struct Rotation(Vec3f);

#[derive(Component)]
struct ViewProjectionMatrix(Mat4f);

#[derive(Component)]
struct Camera;

#[derive(Resource)]
struct Context {
    app: os_platform::App,
    device: gfx_platform::Device,
    main_window: os_platform::Window,
    swap_chain: gfx_platform::SwapChain,
    pmfx: pmfx::Pmfx<gfx_platform::Device>,
    cmd_buf: gfx_platform::CmdBuf,
    imdraw: imdraw::ImDraw<gfx_platform::Device>
}

#[derive(Resource)]
struct HotlineResource<T> {
    res: T
}

type ImDrawRes = HotlineResource::<imdraw::ImDraw<gfx_platform::Device>>;
type PmfxRes = HotlineResource::<pmfx::Pmfx<gfx_platform::Device>>;
type SwapChainRes = HotlineResource::<gfx_platform::SwapChain>;
type DeviceRes = HotlineResource::<gfx_platform::Device>;
type AppRes = HotlineResource::<os_platform::App>;
type MainWindowRes = HotlineResource::<os_platform::Window>;
type CmdBufRes = HotlineResource::<gfx_platform::CmdBuf>;
type ImGuiRes = HotlineResource::<imgui::ImGui::<gfx_platform::Device, os_platform::App>>;

fn create_hotline_context() -> Result<Context, hotline_rs::Error> {
    let num_buffers = 2;

    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("hotline"),
        window: false,
        num_buffers: num_buffers,
        dpi_aware: true,
    });

    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });

    let window = app.create_window(os::WindowInfo {
        title: String::from("hotline"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: num_buffers as u32,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };

    let swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let cmd_buf = device.create_cmd_buf(num_buffers);

    let imdraw_info = imdraw::ImDrawInfo {
        initial_buffer_size_2d: 1024,
        initial_buffer_size_3d: 1024
    };
    let imdraw : imdraw::ImDraw<gfx_platform::Device> = imdraw::ImDraw::create(&imdraw_info).unwrap();

    Ok(Context {
        app,
        device,
        main_window: window,
        swap_chain,
        cmd_buf,
        pmfx: pmfx::Pmfx::create(),
        imdraw
    })
}

fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.pos += velocity.vel;
    }
}

fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>, 
    mut query: Query<(&mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.res;
    for (mut position, mut rotation, mut view_proj) in &mut query {

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
        position.pos += mat_rot * cam_move_delta;

        // generate proj matrix
        let window_rect = main_window.res.get_viewport_rect();
        let aspect = window_rect.width as f32 / window_rect.height as f32;
        let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 0.1, 100000.0);

        // build view / proj matrix
        let translate = Mat4f::from_translation(position.pos);
        let view = translate * mat_rot;
        let view = view.inverse();
       
        // assign view proj
        view_proj.0 = proj * view;
    }
}

fn render_2d(
    main_window: ResMut<MainWindowRes>,
    mut swap_chain: ResMut<SwapChainRes>, 
    mut device: ResMut<DeviceRes>,
    mut cmd_buf: ResMut<CmdBufRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: ResMut<PmfxRes>) {
    // render grid
    let cmd_buf = &mut cmd_buf.res;
    let swap_chain = &mut swap_chain.res;
    let main_window = &main_window.res;
    let imdraw = &mut imdraw.res;
    let pmfx = &pmfx.res;

    let draw_bb = swap_chain.get_backbuffer_index() as usize;
    let window_rect = main_window.get_viewport_rect();
    let viewport = gfx::Viewport::from(window_rect);
    let scissor = gfx::ScissorRect::from(window_rect);

    cmd_buf.begin_render_pass(swap_chain.get_backbuffer_pass_no_clear_mut());
    cmd_buf.set_viewport(&viewport);
    cmd_buf.set_scissor_rect(&scissor);

    let ortho = Mat4f::create_ortho_matrix(0.0, window_rect.width as f32, window_rect.height as f32, 0.0, 0.0, 1.0);
    imdraw.add_line_2d(Vec2f::new(600.0, 100.0), Vec2f::new(600.0, 500.0), Vec4f::new(1.0, 0.0, 1.0, 1.0));
    imdraw.add_tri_2d(Vec2f::new(100.0, 100.0), Vec2f::new(100.0, 400.0), Vec2f::new(400.0, 400.0), Vec4f::new(0.0, 1.0, 1.0, 1.0));
    imdraw.add_rect_2d(Vec2f::new(800.0, 100.0), Vec2f::new(200.0, 400.0), Vec4f::new(1.0, 0.5, 0.0, 1.0));

    cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_2d").unwrap());
    cmd_buf.push_constants(0, 16, 0, &ortho);
    imdraw.submit(&mut device.res, draw_bb).unwrap();
    imdraw.draw_2d(cmd_buf, draw_bb);

    cmd_buf.end_render_pass();
}

fn render_grid(
    main_window: ResMut<MainWindowRes>,
    mut swap_chain: ResMut<SwapChainRes>, 
    mut device: ResMut<DeviceRes>,
    mut cmd_buf: ResMut<CmdBufRes>,
    mut imdraw: ResMut<ImDrawRes>,
    pmfx: ResMut<PmfxRes>,
    mut query: Query<&ViewProjectionMatrix> ) {

    for view_proj in &mut query {
        // render grid
        let cmd_buf = &mut cmd_buf.res;
        let swap_chain = &mut swap_chain.res;
        let main_window = &main_window.res;
        let imdraw = &mut imdraw.res;
        let pmfx = &pmfx.res;

        let draw_bb = swap_chain.get_backbuffer_index() as usize;
        let window_rect = main_window.get_viewport_rect();
        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

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

        imdraw.submit(&mut device.res, draw_bb).unwrap();

        cmd_buf.begin_render_pass(swap_chain.get_backbuffer_pass_no_clear_mut());
        cmd_buf.set_viewport(&viewport);
        cmd_buf.set_scissor_rect(&scissor);

        cmd_buf.set_render_pipeline(&pmfx.get_render_pipeline("imdraw_3d").unwrap());
        cmd_buf.push_constants(0, 16, 0, &view_proj.0);

        imdraw.draw_3d(cmd_buf, draw_bb);

        cmd_buf.end_render_pass();
    }
}

fn render_imgui(
    mut app: ResMut<AppRes>,
    mut swap_chain: ResMut<SwapChainRes>,
    mut main_window: ResMut<MainWindowRes>,
    mut device: ResMut<DeviceRes>,
    mut cmd_buf: ResMut<CmdBufRes>,
    mut imgui: ResMut<ImGuiRes>) {
        cmd_buf.res.begin_render_pass(swap_chain.res.get_backbuffer_pass_no_clear_mut());
        imgui.res.render(&mut app.res, &mut main_window.res, &mut device.res, &mut cmd_buf.res);
        cmd_buf.res.end_render_pass();
}

#[derive(StageLabel)]
pub struct StageUpdate;

#[derive(StageLabel)]
pub struct StageRender3D;

#[derive(StageLabel)]
pub struct StageRender2D;

#[derive(StageLabel)]
pub struct StageRenderImGui;


fn main() -> Result<(), hotline_rs::Error> {    

    //
    // create context
    //

    let mut ctx = create_hotline_context()?;

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();

    let font_path = asset_path
        .join("..\\..\\..\\examples\\imgui_demo\\Roboto-Medium.ttf")
        .to_str()
        .unwrap()
        .to_string();

    let mut imgui_info = imgui::ImGuiInfo::<gfx_platform::Device, os_platform::App> {
        device: &mut ctx.device,
        swap_chain: &mut ctx.swap_chain,
        main_window: &ctx.main_window,
        fonts: vec![imgui::FontInfo {
            filepath: font_path,
            glyph_ranges: None
        }],
    };
    let mut imgui = imgui::ImGui::create(&mut imgui_info).unwrap();

    //
    // create pipelines
    //

    ctx.pmfx.load(asset_path.join("data/shaders/imdraw").to_str().unwrap())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_2d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_3d", ctx.swap_chain.get_backbuffer_pass())?;

    //
    // main loop
    //

    let mut world = World::new();

    world.spawn((
        Position { pos: Vec3f::zero() },
        Velocity { vel: Vec3f::one() },
    ));
    
    world.spawn((
        Position { pos: Vec3f::new(0.0, 100.0, 0.0) },
        Rotation { 0: Vec3f::new(-45.0, 0.0, 0.0) },
        ViewProjectionMatrix { 0: Mat4f::identity()},
        Camera,
    ));

    let mut mode = 0;
    let mut imgui_open = true;
    let mut call_render_2d = false;
    let mut call_render_3d = true;

    while ctx.app.run() {

        // update window and swap chain for the new frame
        ctx.main_window.update(&mut ctx.app);

        // hotline update
        let keys = ctx.app.get_keys_down();
        if keys['M' as usize] {
            mode = !mode;
        }

        imgui.new_frame(&mut ctx.app, &mut ctx.main_window, &mut ctx.device);
        if imgui.begin("hello world", &mut imgui_open, imgui::WindowFlags::NONE) {
            imgui.checkbox("Render 3D", &mut call_render_3d);
            imgui.checkbox("Render 2D", &mut call_render_2d);
        }
        imgui.end();

        ctx.swap_chain.update::<os_platform::App>(&mut ctx.device, &ctx.main_window, &mut ctx.cmd_buf);
        ctx.cmd_buf.reset(&ctx.swap_chain);

        // clear the swap chain
        ctx.cmd_buf.begin_render_pass(ctx.swap_chain.get_backbuffer_pass_mut());
        ctx.cmd_buf.end_render_pass();
       
        // transition to RT
        ctx.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(ctx.swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        // move hotline resource into world
        world.insert_resource(AppRes {res: ctx.app});
        world.insert_resource(MainWindowRes {res: ctx.main_window});
        world.insert_resource(DeviceRes {res: ctx.device});
        world.insert_resource(CmdBufRes {res: ctx.cmd_buf});
        world.insert_resource(SwapChainRes {res: ctx.swap_chain});
        world.insert_resource(ImDrawRes {res: ctx.imdraw});
        world.insert_resource(PmfxRes {res: ctx.pmfx});
        world.insert_resource(ImGuiRes {res: imgui});

        let mut schedule = Schedule::default();
        schedule.add_stage(StageUpdate, SystemStage::parallel()
            .with_system(update_cameras)
            .with_system(movement)
        );

        if call_render_3d {
            schedule.add_stage(StageRender3D, SystemStage::single_threaded()
                .with_system(render_grid)
            );
        }

        if call_render_2d {
            schedule.add_stage(StageRender2D, SystemStage::single_threaded()
                .with_system(render_2d)
            );
        }

        schedule.add_stage(StageRenderImGui, SystemStage::single_threaded()
            .with_system(render_imgui)
        );

        // run systems
        schedule.run(&mut world);
    
        // move resources back out
        ctx.app = world.remove_resource::<AppRes>().unwrap().res;
        ctx.main_window = world.remove_resource::<MainWindowRes>().unwrap().res;
        ctx.device = world.remove_resource::<DeviceRes>().unwrap().res;
        ctx.cmd_buf = world.remove_resource::<CmdBufRes>().unwrap().res;
        ctx.imdraw = world.remove_resource::<ImDrawRes>().unwrap().res;
        ctx.pmfx = world.remove_resource::<PmfxRes>().unwrap().res;
        ctx.swap_chain = world.remove_resource::<SwapChainRes>().unwrap().res;
        imgui = world.remove_resource::<ImGuiRes>().unwrap().res;

        // transition to present
        ctx.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(ctx.swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        // execute cmdbuffers and swap
        ctx.cmd_buf.close(&ctx.swap_chain).unwrap();
        ctx.device.execute(&ctx.cmd_buf);
        ctx.swap_chain.swap(&ctx.device);
    }

    // exited with code 0
    Ok(())
}