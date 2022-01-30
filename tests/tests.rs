use hotline::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::ReadBackRequest;
use gfx::SwapChain;

use os::App;
use os::Window;

use image::*;

use std::env;
use std::fs;

use gfx::d3d12 as gfx_platform;
#[cfg(target_os = "windows")]
use os::win32 as os_platform;

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
    });
}

#[test]
fn create_d3d12_device() {
    let _ = os_platform::App::create(os::AppInfo {
        name: String::from("create_d3d12_device"),
        window: false,
        num_buffers: 0,
    });
    let _dev = gfx_platform::Device::create();
}

#[test]
fn create_window() {
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("create_window"),
        window: false,
        num_buffers: 0,
    });
    let win = app.create_window(os::WindowInfo {
        title: String::from("hello world!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    win.bring_to_front();
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 0);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn window_set_rect() {
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("window_set_rect"),
        window: false,
        num_buffers: 0,
    });
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("window_set_rect!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    win.set_rect(os::Rect {
        x: 200,
        y: 0,
        width: 1280,
        height: 720,
    });
    win.bring_to_front();
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 200);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn swap_chain_buffer() {
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("swap_chain_buffer"),
        window: false,
        num_buffers: 0,
    });
    let dev = gfx_platform::Device::create();
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("swap chain buffering"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });
    win.bring_to_front();

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

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
        win.update();
        swap_chain.update(&dev, &win, &mut cmdbuffer);

        cmdbuffer.reset(&swap_chain);

        let col = &clears_colours[i];

        let mut pass = dev.create_render_pass(&gfx::RenderPassInfo {
            render_targets: vec![swap_chain.get_backbuffer_texture().clone()],
            rt_clear: Some(gfx::ClearColour {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }),
            depth_stencil_target: None,
            ds_clear: None,
            resolve: false,
            discard: false,
        });

        cmdbuffer.begin_render_pass(&mut pass);
        cmdbuffer.end_render_pass();

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        i = (i + 1) % clears_colours.len();
        count = count + 1;

        if count > 3 {
            break;
        }
    }
}

#[test]
fn draw_triangle() {
    let app = os_platform::App::create(os::AppInfo {
        name: String::from("draw_triangle"),
        window: false,
        num_buffers: 0,
    });

    let mut dev = gfx_platform::Device::create();

    let mut win = app.create_window(os::WindowInfo {
        title: String::from("triangle!"),
        rect: os::Rect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
        },
    });

    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

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
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 3
    };

    let vertex_buffer = dev.create_buffer(&info, gfx::as_u8_slice(&vertices));

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
        }\0";

    let vs_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Vertex,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("VSMain\0"),
            target: String::from("vs_5_0\0"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let ps_info = gfx::ShaderInfo {
        shader_type: gfx::ShaderType::Fragment,
        compile_info: Some(gfx::ShaderCompileInfo {
            entry_point: String::from("PSMain\0"),
            target: String::from("ps_5_0\0"),
            flags: gfx::ShaderCompileFlags::NONE,
        }),
    };

    let vs = dev.create_shader(&vs_info, src.as_bytes());
    let ps = dev.create_shader(&ps_info, src.as_bytes());

    let pso = dev.create_pipeline(&gfx::PipelineInfo {
        vs: Some(vs),
        fs: Some(ps),
        cs: None,
        input_layout: vec![
            gfx::InputElementInfo {
                semantic: String::from("POSITION\0"),
                index: 0,
                format: gfx::Format::RGB32f,
                input_slot: 0,
                aligned_byte_offset: 0,
                input_slot_class: gfx::InputSlotClass::PerVertex,
                step_rate: 0,
            },
            gfx::InputElementInfo {
                semantic: String::from("COLOR\0"),
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
                visibility: gfx::ShaderVisibility::Fragment,
                num_values: 4,
                shader_register: 0,
                register_space: 0,
            }]),
            tables: Some(vec![gfx::DescriptorTableInfo {
                visibility: gfx::ShaderVisibility::Fragment,
                table_type: gfx::DescriptorTableType::ShaderResource,
                num_descriptors: Some(1),
                shader_register: 0,
                register_space: 0,
            }]),
            static_samplers: Some(vec![gfx::SamplerInfo {
                visibility: gfx::ShaderVisibility::Fragment,
                filter: gfx::SamplerFilter::Linear,
                address_u: gfx::SamplerAddressMode::Wrap,
                address_v: gfx::SamplerAddressMode::Wrap,
                address_w: gfx::SamplerAddressMode::Wrap,
                comparison: None,
                border_colour: None,
                mip_lod_bias: 0.0,
                max_aniso: 0,
                min_lod: -1.0,
                max_lod: -1.0,
                shader_register: 0,
                register_space: 0,
            }]),
        },
    });

    let mut rbr = gfx_platform::ReadBackRequest {
        fence_value: u64::MAX,
        resource: None,
        size: 0,
        row_pitch: 0,
        slice_pitch: 0,
    };

    let mut written = false;
    let mut ci = 0;
    let mut count = 0;
    while app.run() {
        win.update();
        swap_chain.update(&dev, &win, &mut cmdbuffer);

        let window_rect = win.get_rect();

        let viewport = gfx::Viewport::from(window_rect);
        let scissor = gfx::ScissorRect::from(window_rect);

        cmdbuffer.reset(&swap_chain);

        let mut pass = dev.create_render_pass(&gfx::RenderPassInfo {
            render_targets: vec![swap_chain.get_backbuffer_texture().clone()],
            rt_clear: Some(gfx::ClearColour {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }),
            depth_stencil_target: None,
            ds_clear: None,
            resolve: false,
            discard: false,
        });

        cmdbuffer.begin_render_pass(&mut pass);
        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);
        cmdbuffer.set_pipeline_state(&pso);

        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
        cmdbuffer.draw_instanced(3, 1, 0, 0);

        /*
        if !rbr.resource.is_some() && !written {
            rbr = cmdbuffer.read_back_backbuffer(&swap_chain);
        } else {
            if rbr.is_complete(&swap_chain) && rbr.resource.is_some() {
                let data = rbr.get_data().unwrap();
                image::write_to_file(String::from("my_triangle"), 1280, 720, 4, &data.data)
                    .unwrap();

                rbr.resource = None;
                written = true;
            }
        }
        */

        cmdbuffer.end_render_pass();
        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(128);
        ci = (ci + 1) % 4;
        count = count + 1;
        break;
    }

    cmdbuffer.reset(&swap_chain);
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
