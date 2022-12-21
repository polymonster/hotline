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

#[derive(Resource)]
struct Context<D: Device, A: App> {
    app: A,
    device: D,
    main_window: A::Window,
    swap_chain: D::SwapChain,
    pmfx: pmfx::Pmfx<D>,
    cmd_buf: D::CmdBuf,
    imdraw: imdraw::ImDraw<D>
}

impl<D, A> Context<D, A> where D: Device, A:App {
    pub fn update(&mut self) {
        self.main_window.update(&mut self.app);
        self.swap_chain.update::<A>(&mut self.device, &self.main_window, &mut self.cmd_buf);
        self.cmd_buf.reset(&self.swap_chain);
        self.cmd_buf.close(&self.swap_chain);
        self.device.execute(&self.cmd_buf);
        self.swap_chain.swap(&self.device);
    }
}

fn create_hotline_context<D: Device, A: App>() -> Result<Context<D, A>, hotline_rs::Error> {
    let exe_path = std::env::current_exe().ok().unwrap();
    let num_buffers = 2;

    let mut app = A::create(os::AppInfo {
        name: String::from("imdraw"),
        window: false,
        num_buffers: num_buffers,
        dpi_aware: true,
    });

    let mut device = D::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        ..Default::default()
    });

    let mut window = app.create_window(os::WindowInfo {
        title: String::from("imdraw!"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let mut swap_chain_info = gfx::SwapChainInfo {
        num_buffers: num_buffers as u32,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };

    let mut swap_chain = device.create_swap_chain::<A>(&swap_chain_info, &window)?;
    let mut cmd_buf = device.create_cmd_buf(num_buffers);

    let imdraw_info = imdraw::ImDrawInfo {
        initial_buffer_size_2d: 1024,
        initial_buffer_size_3d: 1024
    };
    let imdraw : imdraw::ImDraw<D> = imdraw::ImDraw::create(&imdraw_info).unwrap();

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



// update
fn movement(mut query: Query<(&mut Position, &Velocity)>) {
    for (mut position, velocity) in &mut query {
        position.pos += velocity.vel;
    }
}


// 
fn render<D: Device, A: App>(mut ctx: ResMut<Context<D, A>>, mut query: Query<()>) {
    ctx.update();
}

// Define a unique public name for a new Stage.
#[derive(StageLabel)]
pub struct UpdateMovement;

fn main() -> Result<(), hotline_rs::Error> {    

    //
    // create context
    //

    let mut ctx = create_hotline_context::<gfx_platform::Device, os_platform::App>()?;

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();

    //
    // create pipelines
    //

    ctx.pmfx.load(asset_path.join("data/shaders/imdraw").to_str().unwrap())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_2d", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_pipeline(&ctx.device, "imdraw_3d", ctx.swap_chain.get_backbuffer_pass())?;

    let pso_3d = ctx.pmfx.get_render_pipeline("imdraw_3d").unwrap();
    let pso_2d = ctx.pmfx.get_render_pipeline("imdraw_2d").unwrap();

    //
    // main loop
    //

    let mut world = World::new();
    let mut schedule = Schedule::default();

    world.spawn((
        Position { pos: Vec3f::zero() },
        Velocity { vel: Vec3f::one() },
    ));

    let mut app = ctx.app.clone();
    world.insert_resource(ctx);

    schedule.add_stage(UpdateMovement, SystemStage::parallel()
        .with_system(movement)
        .with_system(render::<gfx_platform::Device, os_platform::App>)
    );

    while app.run() {
        schedule.run(&mut world);
    }

    // exited with code 0
    Ok(())
    
    /*

    let mut cam_rot = Vec2f::new(-45.0, 0.0);
    let mut cam_pos = Vec3f::new(0.0, 100.0, 0.0);

    let mut regen = true;
    let draw_bb = 0;

    let mut debounce = false;

    while app.run() {
        // update window and swap chain
        window.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

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

        if keys['R' as usize] && !debounce {
            regen = true;
            debounce = true;
        }
        else if !keys['R' as usize] {
            debounce = false;
        }

        if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
            let mouse_delta = app.get_mouse_pos_delta();
            cam_rot.x -= mouse_delta.y as f32;
            cam_rot.y -= mouse_delta.x as f32;
        }

        let mat_rot_x = Mat4f::from_x_rotation(f32::deg_to_rad(cam_rot.x));
        let mat_rot_y = Mat4f::from_y_rotation(f32::deg_to_rad(cam_rot.y));
        let mat_rot = mat_rot_y * mat_rot_x;

        cam_pos += mat_rot * cam_move_delta; 

        let translate = Mat4f::from_translation(cam_pos);
        let view = translate * mat_rot;
        let view = view.inverse();

        // update viewport from window size
        let window_rect = window.get_viewport_rect();
        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        // build command buffer and make draw calls
        cmd.reset(&swap_chain);

        // transition to RT
        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmd.set_viewport(&viewport);
        cmd.set_scissor_rect(&scissor);
        
        let bb = cmd.get_backbuffer_index() as usize;

        // 2d pass
        if true {
            let ortho = Mat4f::create_ortho_matrix(0.0, window_rect.width as f32, window_rect.height as f32, 0.0, 0.0, 1.0);
            imdraw.add_line_2d(Vec2f::new(600.0, 100.0), Vec2f::new(600.0, 500.0), Vec4f::new(1.0, 0.0, 1.0, 1.0));
            imdraw.add_tri_2d(Vec2f::new(100.0, 100.0), Vec2f::new(100.0, 400.0), Vec2f::new(400.0, 400.0), Vec4f::new(0.0, 1.0, 1.0, 1.0));
            imdraw.add_rect_2d(Vec2f::new(800.0, 100.0), Vec2f::new(200.0, 400.0), Vec4f::new(1.0, 0.5, 0.0, 1.0));
    
            cmd.set_render_pipeline(&pso_2d);
            cmd.push_constants(0, 16, 0, &ortho);
            imdraw.submit(&mut device, bb)?;
            imdraw.draw_2d(&mut cmd, bb);
        }

        // 3d pass
        if true {
            let aspect = window_rect.width as f32 / window_rect.height as f32;
            let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 0.1, 100000.0);

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

            if regen {
                imdraw.submit(&mut device, draw_bb)?;
                regen = false;
            }
            
            let view_proj = proj * view;

            cmd.set_render_pipeline(&pso_3d);
            cmd.push_constants(0, 16, 0, &view_proj);
            
            imdraw.draw_3d(&mut cmd, draw_bb);
        }

        cmd.end_render_pass();

        // transition to present
        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });

        cmd.close(&swap_chain)?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();
    cmd.reset(&swap_chain);

    Ok(())
    */
}