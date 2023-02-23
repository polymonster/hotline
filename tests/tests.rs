// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use std::collections::HashMap;

use hotline_rs::*;
use hotline_rs::client::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;

#[cfg(target_os = "windows")]
use gfx::d3d12 as gfx_platform;

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
        usage: gfx::BufferUsage::Vertex,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 3,
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
        descriptor_layout: gfx::DescriptorLayout::default(),
        raster_info: gfx::RasterInfo::default(),
        depth_stencil_info: gfx::DepthStencilInfo::default(),
        blend_info: gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        },
        topology: gfx::Topology::TriangleList,
        patch_index: 0,
        pass: swap_chain.get_backbuffer_pass(),
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
fn boot_empty_client() {
    println!("test: boot_empty_client");
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
    ctx.run_once();
}

/// Load the basic empty plugin, should print and close gracefully
/// plugin ui
/// plugin unload
/// plugin setup
/// plugin update
fn boot_client_empty_plugin() {
    println!("test: boot_client_empty_plugin");
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new())
    };

    // empty plugin
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("empty".to_string(), PluginInfo {
            path: get_data_path("../plugins")
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_empty_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once();
}

/// Boots the client with a plugin that does not exist, should load gracefully and notify the missing plugin
fn boot_client_missing_plugin() {
    println!("test: boot_client_missing_plugin");
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new())
    };

    // empty plugin
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("missing".to_string(), PluginInfo {
            path: get_data_path("../missing")
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_missing_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once();
}

/// Boots the client with the ecs plugin and ecs_demos plugin but with no `PluginData`
fn boot_client_ecs_plugin() {
    println!("test: boot_client_ecs_plugin");
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new()),
    };

    // ecs plugin with no demo active
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("ecs".to_string(), PluginInfo {
            path: get_data_path("../plugins")
        });
        plugins.insert("ecs_demos".to_string(), PluginInfo {
            path: get_data_path("../plugins")
        });
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "boot_client_ecs_plugin".to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once();
}

/// Boots the client with the ecs plugin and ecs_demos with the primitives sample active
fn boot_client_ecs_plugin_demo(demo_name: &str) {
    println!("test: boot_client_ecs_plugin");
    let mut config = client::UserConfig {
        main_window_rect: HotlineInfo::default().window_rect,
        console_window_rect: None,
        plugins: Some(HashMap::new()),
        plugin_data: Some(HashMap::new()),
    };

    // ecs plugin with no demo active
    if let Some(plugins) = &mut config.plugins {
        plugins.insert("ecs".to_string(), PluginInfo {
            path: get_data_path("../plugins")
        });
        plugins.insert("ecs_demos".to_string(), PluginInfo {
            path: get_data_path("../plugins")
        });
    }

    if let Some(plugin_data) = &mut config.plugin_data {
        plugin_data.insert("ecs".to_string(), format!("{{\"active_demo\": \"{}\"}}", demo_name));
    }

    // create client
    let ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: demo_name.to_string(),
        user_config: Some(config),
        ..Default::default()
    }).unwrap();
    
    // run
    ctx.run_once();
}

#[test]
fn client_tests() {
    // build the plugins
    //hotline_rs::plugin::build_all();

    // run the tests.. they need to run 1 by 1 as we cant have 2 clients running concurrently
    //boot_empty_client();

    /*
    boot_client_empty_plugin();
    boot_client_ecs_plugin();
    boot_client_missing_plugin();
    boot_client_ecs_plugin_demo("test_missing_demo");
    boot_client_ecs_plugin_demo("test_missing_systems");
    boot_client_ecs_plugin_demo("primitives");
    boot_client_ecs_plugin_demo("cube");
    boot_client_ecs_plugin_demo("test_missing_systems");
    boot_client_ecs_plugin_demo("test_missing_render_graph");
    */
}