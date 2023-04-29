// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use std::collections::HashMap;

use hotline_rs::prelude::*;

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

#[test]
fn create_app() {
    let _app = os_platform::App::create(os::AppInfo {
        name: String::from("create_app"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });
}

#[test]
fn create_d3d12_device() {
    let _ = os_platform::App::create(os::AppInfo {
        name: String::from("create_d3d12_device"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });
    let _dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 1,
        render_target_heap_size: 1,
        depth_stencil_heap_size: 1,
    });
}

#[test]
fn create_window() {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("create_window"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });
    let win = app.create_window(os::WindowInfo {
        title: String::from("hello world!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });
    win.bring_to_front();
    let size = win.get_size();
    let pos = win.get_pos();
    assert_eq!(pos.x, 0);
    assert_eq!(pos.y, 0);
    assert_eq!(size.x, 1280);
    assert_eq!(size.y, 720);
}

#[test]
fn swap_chain_buffer() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("swap_chain_buffer"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });
    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 0,
        render_target_heap_size: 2,
        depth_stencil_heap_size: 0,
    });
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("swap chain buffering"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });
    win.bring_to_front();

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: None,
    };

    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmd = dev.create_cmd_buf(2);

    let clears_colours: [gfx::ClearColour; 4] = [
        gfx::ClearColour {
            r: 1.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        },
        gfx::ClearColour {
            r: 1.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
        gfx::ClearColour {
            r: 0.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        gfx::ClearColour {
            r: 0.0,
            g: 1.0,
            b: 0.0,
            a: 1.0,
        },
    ];

    let mut i = 0;
    let mut count = 0;
    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmd);

        cmd.reset(&swap_chain);

        let mut pass = swap_chain.get_backbuffer_pass_mut();
        cmd.begin_render_pass(&mut pass);
        cmd.end_render_pass();

        cmd.close()?;

        dev.execute(&cmd);
        swap_chain.swap(&dev);

        std::thread::sleep(std::time::Duration::from_millis(60));
        i = (i + 1) % clears_colours.len();
        count = count + 1;

        if count > 3 {
            break;
        }
    }

    swap_chain.wait_for_last_frame();
    cmd.reset(&swap_chain);

    Ok(())
}

#[test]
fn draw_triangle() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("draw_triangle"),
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
        title: String::from("triangle!"),
        rect: os::Rect {
            x: 0,
            y: 0,
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

    let vertices = [
        Vertex {
            position: [0.0, 0.25, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.25, -0.25, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.25, -0.25, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];

    let info = gfx::BufferInfo {
        usage: gfx::BufferUsage::VERTEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 3,
        initial_state: gfx::ResourceState::VertexConstantBuffer
    };

    let vertex_buffer = device.create_buffer(&info, Some(gfx::as_u8_slice(&vertices)))?;

    let src = "
        struct PSInput
        {
            float4 position : SV_POSITION;
            float4 color : COLOR;
        };

        PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
        {
            PSInput result;

            result.position = position;
            result.color = color;

            return result;
        }

        float4 PSMain(PSInput input) : SV_TARGET
        {
            return input.color;
        }";

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain"),
            target: String::from("vs_5_0"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let fs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain"),
            target: String::from("ps_5_0"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let vs = device.create_shader(&vs_info, src.as_bytes())?;
    let fs = device.create_shader(&fs_info, src.as_bytes())?;

    let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
        vs: Some(&vs),
        fs: Some(&fs),
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
        blend_info: gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        },
        topology: gfx::Topology::TriangleList,
        pass: Some(swap_chain.get_backbuffer_pass()),
        ..Default::default()
    })?;

    while app.run() {
        // update window and swap chain
        window.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // update viewport from window size
        let window_rect = window.get_viewport_rect();
        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        // build command buffer and make draw calls
        cmd.reset(&swap_chain);
        cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmd.set_viewport(&viewport);
        cmd.set_scissor_rect(&scissor);
        cmd.set_render_pipeline(&pso);
        cmd.set_vertex_buffer(&vertex_buffer, 0);
        cmd.draw_instanced(3, 1, 0, 0);
        cmd.end_render_pass();
        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&device);

        break;
    }

    swap_chain.wait_for_last_frame();
    cmd.reset(&swap_chain);

    Ok(())
}

#[test]
fn align_tests() {
    // pow2
    let val = gfx::align_pow2(101, 256);
    assert_eq!(val % 256, 0);
    let val = gfx::align_pow2(8861, 64);
    assert_eq!(val % 64, 0);
    let val = gfx::align_pow2(1280, 128);
    assert_eq!(val % 128, 0);
    let val = gfx::align_pow2(5, 4);
    assert_eq!(val % 4, 0);
    let val = gfx::align_pow2(19, 2);
    assert_eq!(val % 2, 0);
    // non pow2
    let val = gfx::align(92, 133);
    assert_eq!(val % 133, 0);
    let val = gfx::align(172, 201);
    assert_eq!(val % 201, 0);
    let val = gfx::align(288, 1177);
    assert_eq!(val % 1177, 0);
    let val = gfx::align(1092, 52);
    assert_eq!(val % 52, 0);
    let val = gfx::align(5568, 21);
    assert_eq!(val % 21, 0);
}

#[test]
fn image_size_tests() {
    assert_eq!(gfx::mip_levels_for_dimension(1024, 1024), 11);
    assert_eq!(gfx::mip_levels_for_dimension(512, 512), 10);
    assert_eq!(gfx::mip_levels_for_dimension(256, 256), 9);
    assert_eq!(gfx::mip_levels_for_dimension(128, 128), 8);
    assert_eq!(gfx::mip_levels_for_dimension(64, 64), 7);
    assert_eq!(gfx::mip_levels_for_dimension(32, 32), 6);
    assert_eq!(gfx::mip_levels_for_dimension(16, 16), 5);
    assert_eq!(gfx::mip_levels_for_dimension(8, 8), 4);
    assert_eq!(gfx::mip_levels_for_dimension(4, 4), 3);
    assert_eq!(gfx::mip_levels_for_dimension(2, 2), 2);
    assert_eq!(gfx::mip_levels_for_dimension(1, 1), 1);

    assert_eq!(gfx::mip_levels_for_dimension(1024, 2048), 12);
    assert_eq!(gfx::mip_levels_for_dimension(1024 + 33, 1024 + 513), 11);
    assert_eq!(gfx::mip_levels_for_dimension(512 + 33, 512 + 263), 10);
}


// client tests must run 1 at a time, this boots the client with empty user info
#[test]
fn pmfx() -> Result<(), hotline_rs::Error> {
    // create a client
    let config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: None,
        plugin_data: Some(HashMap::new())
    };

    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "pmfx_test_client".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();

    // loads the test shaders
    ctx.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples"))?;

    // create pipelines
    ctx.pmfx.create_render_pipeline(&ctx.device, "texture2d_array_test", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_render_pipeline(&ctx.device, "blend_additive", ctx.swap_chain.get_backbuffer_pass())?;

    // test getting pass for format
    let fmt = ctx.swap_chain.get_backbuffer_pass().get_format_hash();
    let texture_pipeline = ctx.pmfx.get_render_pipeline_for_format("texture2d_array_test", fmt)?;
    
    // texture array pipeline has 2 sets of push constants, so the resources bind onto 2
    // t0, space9 is Texture2DArray
    let slots = texture_pipeline.get_pipeline_slot(0, 9, gfx::DescriptorType::ShaderResource);
    assert!(slots.is_some());

    if let Some(slots) = slots {
        assert_eq!(slots.index, 2);
    }

    // colour_pipeline has no textures
    let colour_pipeline = ctx.pmfx.get_render_pipeline_for_format("blend_additive", fmt)?;
    let slots = colour_pipeline.get_pipeline_slot(0, 6, gfx::DescriptorType::ShaderResource);
    assert!(slots.is_none());

    let slots = colour_pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::PushConstants);
    assert!(slots.is_some());

    Ok(())
}

#[test]
// client tests must run 1 at a time, this boots the client with empty user info
fn boot_empty_client() -> Result<(), hotline_rs::Error> {
    let config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: None,
        plugin_data: Some(HashMap::new())
    };

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_empty_client".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once()
}

#[test]
/// Load the basic empty plugin, should print and close gracefully
fn boot_client_empty_plugin() -> Result<(), hotline_rs::Error> {
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new())
    };

    // empty plugin
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("empty".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_empty_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once()
}

#[test]
/// Boots the client with a plugin that does not exist, should load gracefully and notify the missing plugin
fn boot_client_missing_plugin() -> Result<(), hotline_rs::Error> {
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new())
    };

    // empty plugin
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("missing".to_string(), PluginInfo {
            path: "missing".to_string()
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_missing_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once()
}

#[test]
/// Boots the client with the ecs plugin and ecs_demos plugin but with no `PluginData`
fn boot_client_ecs_plugin() -> Result<(), hotline_rs::Error> {
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new()),
    };

    // ecs plugin with no demo active
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("ecs".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
        plugins.insert("ecs_examples".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_ecs_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once()
}

/// default values for examples
const fn examples_config_defaults() -> &'static str {
r#"
{
    "debug_draw_flags": {
        "bits": 1
    },
    "default_cameras": {
        "directional_lights": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            249.39102172851562,
            365.6768493652344,
            232.56088256835938
        ],
        "rot": [
            -47.0,
            47.0,
            0.0
        ],
        "zoom": 500.0
        },
        "draw": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            0.3299350440502167,
            1.5006731748580933,
            -2.6871063709259033
        ],
        "rot": [
            -29.0,
            173.0,
            0.0
        ],
        "zoom": 3.0953867435455322
        },
        "draw_indexed": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            4.720995903015137,
            6.772276401519775,
            -7.269696235656738
        ],
        "rot": [
            -38.0,
            147.0,
            0.0
        ],
        "zoom": 11.0
        },
        "draw_indexed_cbuffer_instanced": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            101.81123352050781,
            123.74796295166016,
            121.33390808105469
        ],
        "rot": [
            -38.0,
            40.0,
            0.0
        ],
        "zoom": 201.0
        },
        "draw_indexed_push_constants": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            101.81123352050781,
            123.74796295166016,
            121.33390808105469
        ],
        "rot": [
            -38.0,
            40.0,
            0.0
        ],
        "zoom": 201.0
        },
        "draw_indirect": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -36.38866424560547,
            67.58219146728516,
            -65.6468734741211
        ],
        "rot": [
            -42.0,
            209.0,
            0.0
        ],
        "zoom": 101.0
        },
        "gpu_frustum_culling": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -135.22999572753906,
            2188.942626953125,
            -186.128173828125
        ],
        "rot": [
            -84.0,
            216.0,
            0.0
        ],
        "zoom": 2201.0
        },
        "draw_material": {
        "aspect": 0.9342105388641357,
        "camera_type": "Fly",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -184.70013427734375,
            346.8671875,
            205.2322235107422
        ],
        "rot": [
            -50.0,
            1401.0,
            0.0
        ],
        "zoom": 401.0
        },
        "draw_push_constants_texture": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            51.158878326416016,
            62.18181228637695,
            60.968780517578125
        ],
        "rot": [
            -38.0,
            40.0,
            0.0
        ],
        "zoom": 101.0
        },
        "geometry_primitives": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            129.87283325195312,
            208.39752197265625,
            172.3470916748047
        ],
        "rot": [
            -44.0,
            37.0,
            0.0
        ],
        "zoom": 300.0
        },
        "point_lights": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            315.1270446777344,
            4168.69384765625,
            403.34423828125
        ],
        "rot": [
            -83.0,
            38.0,
            0.0
        ],
        "zoom": 4200.0
        },
        "spot_lights": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            315.1270446777344,
            4168.69384765625,
            403.34423828125
        ],
        "rot": [
            -83.0,
            38.0,
            0.0
        ],
        "zoom": 4200.0
        },
        "tangent_space_normal_maps": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            152.46359252929688,
            185.31411743164062,
            181.69903564453125
        ],
        "rot": [
            -38.0,
            40.0,
            0.0
        ],
        "zoom": 301.0
        },
        "test_blend_states": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            75.20877838134766,
            149.3721160888672,
            111.50161743164062
        ],
        "rot": [
            -48.0,
            34.0,
            0.0
        ],
        "zoom": 201.0
        },
        "test_compute": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -77.8858642578125,
            167.95074462890625,
            129.62384033203125
        ],
        "rot": [
            -48.0,
            -31.0,
            0.0
        ],
        "zoom": 226.0
        },
        "test_cubemap": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            75.20877838134766,
            149.3721160888672,
            111.50161743164062
        ],
        "rot": [
            -48.0,
            34.0,
            0.0
        ],
        "zoom": 201.0
        },
        "test_multiple_render_targets": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            1012.0797119140625,
            1288.74560546875,
            1012.0797119140625
        ],
        "rot": [
            -42.0,
            45.0,
            0.0
        ],
        "zoom": 1926.0
        },
        "test_raster_states": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -28.536039352416992,
            80.66219329833984,
            53.66848373413086
        ],
        "rot": [
            -53.0,
            -28.0,
            0.0
        ],
        "zoom": 101.0
        },
        "test_texture2d_array": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            61.213871002197266,
            62.5814208984375,
            287.9886169433594
        ],
        "rot": [
            -12.0,
            12.0,
            0.0
        ],
        "zoom": 301.0
        },
        "test_texture3d": {
        "aspect": 0.9342105388641357,
        "camera_type": "Orbit",
        "focus": [
            0.0,
            0.0,
            0.0
        ],
        "fov": 60.0,
        "pos": [
            -68.53109741210938,
            70.45830535888672,
            78.83601379394531
        ],
        "rot": [
            -34.0,
            -41.0,
            0.0
        ],
        "zoom": 126.0
        }
    }
}
"#
}

/// Boots the client with the ecs plugin and ecs_demos with the primitives sample active
fn boot_client_ecs_plugin_demo(demo_name: &str) -> Result<(), hotline_rs::Error> {
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new()),
    };

    // ecs plugin with no demo active
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("ecs".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
        plugins.insert("ecs_examples".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
    }

    if let Some(plugin_data) = &mut config.plugin_data {
        let mut plugin_defaults : serde_json::Value = serde_json::from_str(examples_config_defaults())?;
        
        // activate the current demo
        plugin_defaults.as_object_mut().unwrap().insert(
            "active_demo".to_string(), 
            serde_json::Value::String(demo_name.to_string())
        );

        plugin_data.insert("ecs".to_string(), plugin_defaults);
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: demo_name.to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once()
}

#[test]
fn missing_demo() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_missing_demo")
}

#[test]
fn missing_render_graph() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_missing_render_graph")
}

#[test]
fn missing_render_view() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_missing_view")
}

#[test]
fn geometry_primitives() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("geometry_primitives")
}

#[test]
fn test_point_lights() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("point_lights")
}

#[test]
fn test_spot_lights() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("spot_lights")
}

#[test]
fn test_directional_lights() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("directional_lights")
}

#[test]
fn test_tangent_space_normal_maps() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("tangent_space_normal_maps")
}

#[test]
fn draw() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw")
}

#[test]
fn draw_indexed() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indexed")
}

#[test]
fn draw_indexed_push_constants() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indexed_push_constants")
}

#[test]
fn draw_indexed_vertex_buffer_instanced() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indexed_vertex_buffer_instanced")
}

#[test]
fn draw_indexed_cbuffer_instanced() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indexed_cbuffer_instanced")
}

#[test]
fn draw_push_constants_texture() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_push_constants_texture")
}

#[test]
fn draw_material() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_material")
}

#[test]
fn draw_indirect() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indirect")
}

#[test]
fn gpu_frustum_culling() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("gpu_frustum_culling")
}

#[test]
fn test_raster_states() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_raster_states")
}

#[test]
fn test_blend_states() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_blend_states")
}

#[test]
fn test_cubemap() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_cubemap")
}

#[test]
fn test_texture2d_array() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_texture2d_array")
}

#[test]
fn test_texture3d() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_texture3d")
}

#[test]
fn test_compute() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_compute")
}

#[test]
fn test_multiple_render_targets() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("test_multiple_render_targets")
}