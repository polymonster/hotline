use gfx::AccelerationStructureBuildFlags;
use gfx::BufferUsage;
use gfx::RaytracingBLASInfo;
use gfx::RaytracingInstanceInfo;
use gfx::RaytracingTLASInfo;
use hotline_rs::gfx::RaytracingTLAS;
use hotline_rs::gfx::Texture;
use hotline_rs::gfx::Pipeline;
use hotline_rs::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

#[cfg(target_os = "windows")]
use os::win32 as os_platform;
use gfx::d3d12 as gfx_platform;

struct RaytracingViewport {
    viewport: [f32; 4],
    scissor: [f32; 4]
}

fn main() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("raytraced_triangle"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    let num_buffers : u32 = 2;
    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        shader_heap_size: 4,
        ..Default::default()
    });
    println!("{}", device.get_adapter_info());
    println!("features: {:?}", device.get_feature_flags());

    let mut window = app.create_window(os::WindowInfo {
        title: String::from("raytraced_triangle!"),
        ..Default::default()
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(num_buffers);

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut device, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/raytracing_example"))?;
   
    pmfx.create_raytracing_pipeline(&device, "raytracing")?;

    let index_buffer = device.create_buffer(&gfx::BufferInfo {
        usage: BufferUsage::UPLOAD,
        cpu_access: gfx::CpuAccessFlags::WRITE,
        format: gfx::Format::R16u,
        stride: 2,
        num_elements: 3,
        initial_state: gfx::ResourceState::GenericRead
    }, Some(&vec![0 as u16, 1 as u16, 2 as u16]))?;

    let vertices: Vec<f32> = vec![
        0.0, -0.25, 1.0,
        -0.25, 0.25, 1.0,
        0.25, 0.25, 1.0
    ];
    
    let vertex_buffer = device.create_buffer(&gfx::BufferInfo {
        usage: BufferUsage::UPLOAD,
        cpu_access: gfx::CpuAccessFlags::WRITE,
        format: gfx::Format::RGB32f,
        stride: 12,
        num_elements: 3,
        initial_state: gfx::ResourceState::GenericRead
    }, Some(&vertices))?;

    let blas = device.create_raytracing_blas(&RaytracingBLASInfo {
        geometry: gfx::RaytracingGeometryInfo::Triangles(
            gfx::RaytracingTrianglesInfo {
                index_buffer: &index_buffer,
                vertex_buffer: &vertex_buffer,
                transform3x4: None,
                index_count: 3,
                index_format: gfx::Format::R16u,
                vertex_count: 3,
                vertex_format: gfx::Format::RGB32f,
            }),
        geometry_flags: gfx::RaytracingGeometryFlags::OPAQUE,
        build_flags: AccelerationStructureBuildFlags::PREFER_FAST_TRACE
    })?;

    let tlas = device.create_raytracing_tlas(&RaytracingTLASInfo {
        instances: &vec![RaytracingInstanceInfo {
            transform: [
                1.0, 0.0, 0.0, 0.0, 
                0.0, 1.0, 0.0, 0.0, 
                0.0, 0.0, 1.0, 0.0
            ],
            instance_id: 0,
            instance_mask: 0xff,
            hit_group_index: 0,
            instance_flags: 0,
            blas: &blas
        }],
        build_flags: AccelerationStructureBuildFlags::PREFER_FAST_TRACE
    })?;

    let window_rect = window.get_viewport_rect();

    // unordered access rw texture
    let rw_info = gfx::TextureInfo {
        format: gfx::Format::RGBA8n,
        tex_type: gfx::TextureType::Texture2D,
        width: window_rect.width as u64,
        height: window_rect.height as u64,
        depth: 1,
        array_layers: 1,
        mip_levels: 1,
        samples: 1,
        usage: gfx::TextureUsage::SHADER_RESOURCE | gfx::TextureUsage::UNORDERED_ACCESS,
        initial_state: gfx::ResourceState::UnorderedAccess,
    };
    let raytracing_output = device.create_texture::<u8>(&rw_info, None).unwrap();

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

        let raytracing_pipeline = pmfx.get_raytracing_pipeline("raytracing")?;
        cmd.set_raytracing_pipeline(&raytracing_pipeline.pipeline);
        
        // bind rw tex on u0
        let uav0 =  raytracing_output.get_uav_index().expect("expect raytracing_output to have a uav");
        if let Some(u0) = raytracing_pipeline.pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::UnorderedAccess) {
            cmd.set_binding(&raytracing_pipeline.pipeline, device.get_shader_heap(), u0.index, uav0);
        }

        // set push constants on b0
        let border = 0.1;
        let aspect = window_rect.width as f32 / window_rect.height as f32;
        cmd.push_compute_constants(0, 8, 0, gfx::as_u8_slice(&RaytracingViewport {
            viewport: [
                -1.0 + border, 
                -1.0 + border * aspect,
                 1.0 - border, 
                 1.0 - border * aspect
            ],
            scissor: [
                -1.0 + border / aspect, 
                -1.0 + border,
                 1.0 - border / aspect, 
                 1.0 - border
            ]
        }));

        // bind tlas on t0
        let srv0 =  tlas.get_srv_index().expect("expect tlas to have an srv");
        if let Some(t0) = raytracing_pipeline.pipeline.get_pipeline_slot(0, 0, gfx::DescriptorType::ShaderResource) {
            cmd.set_binding(&raytracing_pipeline.pipeline, device.get_shader_heap(), t0.index, srv0);
        }

        cmd.dispatch_rays(&raytracing_pipeline.sbt, gfx::Size3 {
            x: window_rect.width as u32,
            y: window_rect.height as u32,
            z: 1
        });

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::CopyDst,
        });

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&raytracing_output),
            buffer: None,
            state_before: gfx::ResourceState::UnorderedAccess,
            state_after: gfx::ResourceState::CopySrc,
        });

        cmd.copy_texture_region(&swap_chain.get_backbuffer_texture(), 0, 0, 0, 0, &raytracing_output, None);

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::CopyDst,
            state_after: gfx::ResourceState::Present,
        });

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&raytracing_output),
            buffer: None,
            state_before: gfx::ResourceState::CopySrc,
            state_after: gfx::ResourceState::UnorderedAccess,
        });

        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    device.cleanup_dropped_resources(&swap_chain);

    Ok(())
}