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
    ctx.pmfx.load(&hotline_rs::get_data_path("shaders/tests"))?;

    // create pipelines
    ctx.pmfx.create_render_pipeline(&ctx.device, "texture2d_array_test", ctx.swap_chain.get_backbuffer_pass())?;
    ctx.pmfx.create_render_pipeline(&ctx.device, "blend_additive", ctx.swap_chain.get_backbuffer_pass())?;

    // test getting pass for format
    let fmt = ctx.swap_chain.get_backbuffer_pass().get_format_hash();
    let texture_pipeline = ctx.pmfx.get_render_pipeline_for_format("texture2d_array_test", fmt)?;
    let colour_pipeline = ctx.pmfx.get_render_pipeline_for_format("blend_additive", fmt)?;

    // textures pipeline has 2 sets of push constants, so the resources bind onto 2
    let slots = texture_pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    assert!(slots.is_some());
    if let Some(slots) = slots {
        assert_eq!(slots.slot, 2);
    }

    let slots = colour_pipeline.get_descriptor_slot(0, gfx::DescriptorType::ShaderResource);
    assert!(slots.is_none());

    let slots = colour_pipeline.get_descriptor_slot(0, gfx::DescriptorType::PushConstants);
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
        plugins.insert("ecs_demos".to_string(), PluginInfo {
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
        plugins.insert("ecs_demos".to_string(), PluginInfo {
            path: hotline_rs::get_data_path("../../plugins")
        });
    }

    let cameras = "\"default_cameras\": {\n    \"test_raster_states\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -98.194916,\n        139.37158,\n        -63.3391\n      ],\n      \"rot\": [\n        -48.0,\n        235.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 176.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"spot_lights\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        689.35834,\n        4019.8125,\n        858.7116\n      ],\n      \"rot\": [\n        -77.0,\n        44.0,\n        0.0\n      ],\n      \"focus\": [\n        28.986124,\n        -97.87555,\n        174.87614\n      ],\n      \"zoom\": 4226.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"point_lights\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        182.38878,\n        2662.2185,\n        1634.4049\n      ],\n      \"rot\": [\n        -62.0,\n        6.0,\n        0.0\n      ],\n      \"focus\": [\n        28.986124,\n        -97.87555,\n        174.87614\n      ],\n      \"zoom\": 3126.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_material\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -114.654564,\n        186.1614,\n        -117.1013\n      ],\n      \"rot\": [\n        -45.0,\n        222.0,\n        0.0\n      ],\n      \"focus\": [\n        15.933952,\n        -9.000065,\n        27.931932\n      ],\n      \"zoom\": 276.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"test_texture3d\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -66.92452,\n        96.105034,\n        67.170876\n      ],\n      \"rot\": [\n        -44.0,\n        314.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 126.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indexed_vertex_buffer_instanced\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -219.33995,\n        342.10468,\n        -189.63248\n      ],\n      \"rot\": [\n        -47.0,\n        226.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 476.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"test_texture2d_array\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -226.33401,\n        102.97563,\n        -125.46829\n      ],\n      \"rot\": [\n        -20.0,\n        240.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 276.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_push_constants_texture\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -67.33988,\n        115.45073,\n        -64.553024\n      ],\n      \"rot\": [\n        -45.0,\n        222.0,\n        0.0\n      ],\n      \"focus\": [\n        15.933952,\n        -9.000065,\n        27.931932\n      ],\n      \"zoom\": 176.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"tangent_space_normal_maps\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        137.51843,\n        228.4851,\n        225.79953\n      ],\n      \"rot\": [\n        -46.0,\n        33.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 326.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indexed_cbuffer_instanced\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -72.163376,\n        122.69857,\n        -47.505707\n      ],\n      \"rot\": [\n        -47.0,\n        226.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 176.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"test_cubemap\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -3.079699,\n        107.86743,\n        81.7708\n      ],\n      \"rot\": [\n        -52.0,\n        359.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 126.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indirect_gpu_frustum_culling\": {\n      \"camera_type\": \"Fly\",\n      \"pos\": [\n        36.28253,\n        1094.353,\n        -56.87494\n      ],\n      \"rot\": [\n        -17.0,\n        242.0,\n        0.0\n      ],\n      \"focus\": [\n        0.14157295,\n        6.260146,\n        -2.9459028\n      ],\n      \"zoom\": 1676.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indexed\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -55.17009,\n        123.608604,\n        -135.77267\n      ],\n      \"rot\": [\n        -35.0,\n        202.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 226.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"geometry_primitives\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        148.3664,\n        224.52704,\n        178.43329\n      ],\n      \"rot\": [\n        -45.0,\n        41.0,\n        0.0\n      ],\n      \"focus\": [\n        -2.8662376,\n        -5.9897594,\n        4.4600344\n      ],\n      \"zoom\": 326.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"test_blend_states\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -98.194916,\n        139.37158,\n        -63.3391\n      ],\n      \"rot\": [\n        -48.0,\n        235.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 176.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"directional_lights\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        507.42844,\n        486.19464,\n        509.88507\n      ],\n      \"rot\": [\n        -45.0,\n        55.0,\n        0.0\n      ],\n      \"focus\": [\n        28.986124,\n        -97.87555,\n        174.87614\n      ],\n      \"zoom\": 826.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indexed_push_constants\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -99.10036,\n        195.83395,\n        -114.45306\n      ],\n      \"rot\": [\n        -47.0,\n        217.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 276.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw_indirect\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -25.002493,\n        56.120632,\n        48.60706\n      ],\n      \"rot\": [\n        -41.0,\n        334.0,\n        0.0\n      ],\n      \"focus\": [\n        0.14157295,\n        6.260146,\n        -2.9459028\n      ],\n      \"zoom\": 76.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"draw\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -85.85607,\n        180.96625,\n        -211.72314\n      ],\n      \"rot\": [\n        -35.0,\n        202.0,\n        0.0\n      ],\n      \"focus\": [\n        14.180221,\n        -6.019674,\n        35.875328\n      ],\n      \"zoom\": 326.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    },\n    \"test_compute\": {\n      \"camera_type\": \"Orbit\",\n      \"pos\": [\n        -118.6695,\n        165.57088,\n        117.14041\n      ],\n      \"rot\": [\n        -44.0,\n        314.0,\n        0.0\n      ],\n      \"focus\": [\n        -1.7258416,\n        8.578082,\n        4.209255\n      ],\n      \"zoom\": 226.0,\n      \"aspect\": 0.93421054,\n      \"fov\": 60.0\n    }\n  }";

    if let Some(plugin_data) = &mut config.plugin_data {
        plugin_data.insert("ecs".to_string(), format!("{{\"active_demo\": \"{}\", {}}}", demo_name, cameras));
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
fn draw_indirect_gpu_frustum_culling() -> Result<(), hotline_rs::Error> {
    boot_client_ecs_plugin_demo("draw_indirect_gpu_frustum_culling")
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