use hotline::*;

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

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

fn main() -> Result<(), hotline::Error> {    
    let num_buffers = 2;

    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("imdraw"),
        window: false,
        num_buffers: num_buffers,
        dpi_aware: true,
    });

    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
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

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(2);

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create();

    //
    let pmfx_imdraw = asset_path.join("data/shaders/imdraw");
    pmfx.load2(&device, pmfx_imdraw.to_str().unwrap())?;

    // 2d
    let pmfx_imdraw_2d = asset_path.join("data/shaders/imdraw_2d");
    pmfx.load(&device, pmfx_imdraw_2d.to_str().unwrap())?;

    let technique_2d = pmfx.get_technique("imdraw_2d", "default").unwrap();

    let pso_2d = device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: technique_2d.vs.as_ref(),
        fs: technique_2d.fs.as_ref(),
        input_layout: technique_2d.input_layout.to_vec(),
        descriptor_layout: gfx::DescriptorLayout {
            push_constants: Some(vec![gfx::PushConstantInfo {
                visibility: gfx::ShaderVisibility::Vertex,
                num_values: 16,
                shader_register: 0,
                register_space: 0,
            }]),
            static_samplers: None,
            bindings: None,
        },
        raster_info: gfx::RasterInfo::default(),
        depth_stencil_info: gfx::DepthStencilInfo::default(),
        blend_info: gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        },
        topology: gfx::Topology::LineList,
        patch_index: 0,
        pass: swap_chain.get_backbuffer_pass(),
    })?;

    // 3d
    let pmfx_imdraw_3d = asset_path.join("data/shaders/imdraw_3d");
    pmfx.load(&device, pmfx_imdraw_3d.to_str().unwrap())?;

    let technique_3d = pmfx.get_technique("imdraw_3d", "default").unwrap();

    let pso_3d = device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: technique_3d.vs.as_ref(),
        fs: technique_3d.fs.as_ref(),
        input_layout: technique_3d.input_layout.to_vec(),
        descriptor_layout: gfx::DescriptorLayout {
            push_constants: Some(vec![gfx::PushConstantInfo {
                visibility: gfx::ShaderVisibility::Vertex,
                num_values: 16, 
                shader_register: 0,
                register_space: 0,
            }]),
            static_samplers: None,
            bindings: None,
        },
        raster_info: gfx::RasterInfo::default(),
        depth_stencil_info: gfx::DepthStencilInfo::default(),
        blend_info: gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        },
        topology: gfx::Topology::LineList,
        patch_index: 0,
        pass: swap_chain.get_backbuffer_pass(),
    })?;

    let imdraw_info = imdraw::ImDrawInfo {
        initial_buffer_size_2d: 1024,
        initial_buffer_size_3d: 1024
    };
    let mut imdraw : imdraw::ImDraw<gfx_platform::Device> = imdraw::ImDraw::create(&imdraw_info).unwrap();

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

        //
        /*
        if write_debug(&input_data, &result_data) {
            break;
        }
        */
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();
    cmd.reset(&swap_chain);

    Ok(())
}