use hotline::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

use std::fs;

use maths_rs::*;
use maths_rs::Vec2f;
use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::num::*;
use maths_rs::vec::*;
use maths_rs::mat::*;
use maths_rs::Mat4f;

// use pmfx;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

fn main() -> Result<(), hotline::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("imdraw"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    let num_buffers = 2;

    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers,
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

    // 2d
    let vsc_filepath = asset_path.join("data/shaders/imdraw_2d/default.vsc");
    let psc_filepath = asset_path.join("data/shaders/imdraw_2d/default.psc");
    let info_filepath = asset_path.join("data/shaders/imdraw_2d/info.json");

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device>= pmfx::Pmfx::create();
    pmfx.load_shader(info_filepath.to_str().unwrap());

    let vsc_data = fs::read(vsc_filepath)?;
    let psc_data = fs::read(psc_filepath)?;

    let vsc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: None
    };
    let vs = device.create_shader(&vsc_info, &vsc_data)?;
    
    let psc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: None
    };
    let fs = device.create_shader(&psc_info, &psc_data)?;

    let pso_2d = device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: Some(vs),
        fs: Some(fs),
        input_layout: vec![
            gfx::InputElementInfo {
                semantic: String::from("POSITION"),
                index: 0,
                format: gfx::Format::RG32f,
                input_slot: 0,
                aligned_byte_offset: 0,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
            gfx::InputElementInfo {
                semantic: String::from("COLOR"),
                index: 0,
                format: gfx::Format::RGBA32f,
                input_slot: 0,
                aligned_byte_offset: 8,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
        ],
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
    let vsc_filepath = asset_path.join("data/shaders/imdraw_3d/default.vsc");
    let psc_filepath = asset_path.join("data/shaders/imdraw_3d/default.psc");
    let info_filepath = asset_path.join("data/shaders/imdraw_3d/info.json");

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device>= pmfx::Pmfx::create();
    pmfx.load_shader(info_filepath.to_str().unwrap());

    let vsc_data = fs::read(vsc_filepath)?;
    let psc_data = fs::read(psc_filepath)?;

    let vsc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: None
    };
    let vs = device.create_shader(&vsc_info, &vsc_data)?;
    
    let psc_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: None
    };
    let fs = device.create_shader(&psc_info, &psc_data)?;

    let pso_3d = device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: Some(vs),
        fs: Some(fs),
        input_layout: vec![
            gfx::InputElementInfo {
                semantic: String::from("POSITION"),
                index: 0,
                format: gfx::Format::RGB32f,
                input_slot: 0,
                aligned_byte_offset: 0,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
            gfx::InputElementInfo {
                semantic: String::from("COLOR"),
                index: 0,
                format: gfx::Format::RGBA32f,
                input_slot: 0,
                aligned_byte_offset: 12,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
        ],
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
        if false {
            let ortho = camera::create_ortho_matrix(0.0, window_rect.width as f32, window_rect.height as f32, 0.0, 0.0, 1.0);
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
            let proj = camera::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 0.1, 100000.0);

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

            //
            let l1 = Vec3f::new(10.0, 0.0, 100.0);
            let l2 = Vec3f::new(-10.0, 0.0, 10.0);
            let ll = length(l2 - l1);
            let lv = normalize(l2 - l1);
            let lp = Vec3f::new(lv.z, 0.0, lv.x);
            let r = 10.0;

            imdraw.add_line_3d(l1, l2, Vec4f::white());

            let sp = l1 + (lv * ll * 0.8) + (lp * r * 0.7);
            
            imdraw.add_point_3d(sp, 1.0, Vec4f::red());
            imdraw.add_circle_3d_xz(sp, r, Vec4f::red());

            let cp = closest_point_on_line_segment(sp, l1, l2);

            let lp = sp - l1;
            let t = dot(lv, lp);
            let cp2 = l1 + lv * t;

            imdraw.add_point_3d(cp2, 1.0, Vec4f::red());

            if length(sp - cp) < r {
                let cv = normalize(sp - cp);
                let rp = cp + cv * r;
                imdraw.add_point_3d(rp, 1.0, Vec4f::green());
                imdraw.add_circle_3d_xz(rp, r, Vec4f::green());
            }

            let view_proj = proj * view;

            cmd.set_render_pipeline(&pso_3d);
            cmd.push_constants(0, 16, 0, &view_proj);
            imdraw.submit(&mut device, bb)?;
            imdraw.draw_3d(&mut cmd, bb);
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
}