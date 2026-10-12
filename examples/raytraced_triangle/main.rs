use gfx::AccelerationStructureBuildFlags;
use gfx::BufferUsage;
use gfx::RaytracingBLASInfo;
use gfx::RaytracingInstanceInfo;
use gfx::RaytracingTLASInfo;
use hotline_rs::gfx::RaytracingTLAS;
use hotline_rs::gfx::Texture;
use hotline_rs::gfx::RenderPass;
use hotline_rs::*;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use os::App;
use os::Window;

/// Trace rays inline with RayQuery from a compute shader, instead of with a raytracing pipeline and `dispatch_rays`.
/// Metal only supports inline raytracing, d3d12 supports both and defaults to the pipeline, set this to true to
/// take the inline path on d3d12
const INLINE_RAYTRACING: bool = cfg!(target_os = "macos");

#[repr(C)]
struct Vertex {
    position: [f32; 2],
    texcoord: [f32; 2],
}

/// Create an rw texture output for raytracing to write into
fn create_raytracing_output(device: &mut gfx_platform::Device, window_rect: &os::Rect<i32>) -> gfx_platform::Texture {
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
    device.create_texture::<u8>(&rw_info, None).unwrap()
}

fn main() -> Result<(), hotline_rs::Error> {
    // setup app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("raytraced_triangle"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // setup gpu device
    let num_buffers : u32 = 2;
    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers as usize,
        shader_heap_size: 32,
        ..Default::default()
    });
    println!("{}", device.get_adapter_info());
    println!("features: {:?}", device.get_feature_flags());
    println!("raytracing: {}", if INLINE_RAYTRACING { "inline" } else { "pipeline" });

    // setup window and attach swapchain
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

    // setup swap chain
    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    let mut cmd = device.create_cmd_buf(num_buffers);

    // pmfx for easier piepline loading, pipelines from raytracing_example.pmfx
    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut device, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/raytracing_example"))?;
    if INLINE_RAYTRACING {
        pmfx.create_compute_pipeline(&device, "raytracing_inline")?;
    }
    else {
        pmfx.create_raytracing_pipeline(&device, "raytracing")?;
    }
    pmfx.create_render_pipeline(&device, "blit", swap_chain.get_backbuffer_pass())?;
    let blit_fmt = swap_chain.get_backbuffer_pass().get_format_hash();

    // fullscreen quad (NDC) to blit the output, texcoords flipped so (0,0) is top-left of the image
    let quad_vertices = [
        Vertex { position: [-1.0, -1.0], texcoord: [0.0, 1.0] },
        Vertex { position: [-1.0,  1.0], texcoord: [0.0, 0.0] },
        Vertex { position: [ 1.0,  1.0], texcoord: [1.0, 0.0] },
        Vertex { position: [ 1.0, -1.0], texcoord: [1.0, 1.0] },
    ];
    let quad_vertex_buffer = device.create_buffer(&gfx::BufferInfo {
        usage: BufferUsage::VERTEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<Vertex>(),
        num_elements: 4,
        initial_state: gfx::ResourceState::VertexConstantBuffer
    }, Some(gfx::as_u8_slice(&quad_vertices)))?;

    let quad_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
    let quad_index_buffer = device.create_buffer(&gfx::BufferInfo {
        usage: BufferUsage::INDEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::R16u,
        stride: std::mem::size_of::<u16>(),
        num_elements: 6,
        initial_state: gfx::ResourceState::IndexBuffer
    }, Some(gfx::as_u8_slice(&quad_indices)))?;

    // create geometry for the BLAS
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

    // create the BLAS itself
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
                vertex_stride: 12
            }),
        geometry_flags: gfx::RaytracingGeometryFlags::OPAQUE,
        build_flags: AccelerationStructureBuildFlags::PREFER_FAST_TRACE
    })?;

    // create a TLAS with a single instance of BLAS
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

    // unordered access rw texture
    let mut raytracing_output = create_raytracing_output(&mut device, &window.get_viewport_rect());

    while app.run() {
        // update window and swap chain
        window.update(&mut app);

        // update viewport from window size
        let window_rect = window.get_viewport_rect();

        // update and or resize swap chain
        let reized = swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // resize the rw texture output
        if reized {
            raytracing_output = create_raytracing_output(&mut device, &window.get_viewport_rect());
        }

        // build command buffer and make draw calls
        cmd.reset(&swap_chain);

        // viewport and stencil in screen space, shared by both paths
        let border = 0.1;
        let aspect = window_rect.width as f32 / window_rect.height as f32;
        let raygen_constants = [
            // viewport
            -1.0 + border,
            -1.0 + border * aspect,
             1.0 - border,
             1.0 - border * aspect,
            // stencil
            -1.0 + border / aspect,
            -1.0 + border,
             1.0 - border / aspect,
             1.0 - border
        ];

        let uav0 = raytracing_output.get_uav_index().expect("expect raytracing_output to have a uav");
        let srv0 = tlas.get_srv_index().expect("expect tlas to have an srv");

        cmd.begin_event(0xff00ff00, "Raytrace");
        if INLINE_RAYTRACING {
            // trace rays from a compute shader with RayQuery
            let raytracing = pmfx.get_compute_pipeline("raytracing_inline")?;
            cmd.set_compute_pipeline(raytracing);

            // bind rw tex on u0, tlas on t0 and push constants on b0
            cmd.set_binding(raytracing, 0, 0, gfx::DescriptorType::UnorderedAccess, device.get_shader_heap(), uav0);
            cmd.set_binding(raytracing, 0, 0, gfx::DescriptorType::ShaderResource, device.get_shader_heap(), srv0);
            cmd.push_compute_constants(raytracing, 0, 0, 8, 0, gfx::as_u8_slice(&raygen_constants));

            cmd.dispatch(gfx::Size3 {
                x: (window_rect.width as u32 + 7) / 8,
                y: (window_rect.height as u32 + 7) / 8,
                z: 1
            }, gfx::Size3 {
                x: 8,
                y: 8,
                z: 1
            });
        }
        else {
            // trace rays with a raytracing pipeline
            let raytracing_pipeline = pmfx.get_raytracing_pipeline("raytracing")?;
            cmd.set_raytracing_pipeline(&raytracing_pipeline.pipeline);

            // bind rw tex on u0, tlas on t0 and push constants on b0
            cmd.set_binding(&raytracing_pipeline.pipeline, 0, 0, gfx::DescriptorType::UnorderedAccess, device.get_shader_heap(), uav0);
            cmd.set_binding(&raytracing_pipeline.pipeline, 0, 0, gfx::DescriptorType::ShaderResource, device.get_shader_heap(), srv0);
            cmd.push_compute_constants(&raytracing_pipeline.pipeline, 0, 0, 8, 0, gfx::as_u8_slice(&raygen_constants));

            cmd.dispatch_rays(&raytracing_pipeline.sbt, gfx::Size3 {
                x: window_rect.width as u32,
                y: window_rect.height as u32,
                z: 1
            });
        }
        cmd.end_event();

        // blit the output to the back buffer
        cmd.begin_event(0xff0000ff, "Blit");
        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&raytracing_output),
            buffer: None,
            state_before: gfx::ResourceState::UnorderedAccess,
            state_after: gfx::ResourceState::ShaderResource,
        });

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let blit = pmfx.get_render_pipeline_for_format("blit", blit_fmt)?;
        cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmd.set_viewport(&gfx::Viewport::from(window_rect));
        cmd.set_scissor_rect(&gfx::ScissorRect::from(window_rect));
        cmd.set_render_pipeline(blit);
        cmd.set_heap(blit, device.get_shader_heap());
        cmd.set_index_buffer(&quad_index_buffer);
        cmd.set_vertex_buffer(&quad_vertex_buffer, 0);
        let blit_srv = [raytracing_output.get_srv_index().expect("expect raytracing_output to have an srv") as u32, 0, 0, 0];
        cmd.push_render_constants(blit, 0, 0, 4, 0, gfx::as_u8_slice(&blit_srv));
        cmd.draw_indexed_instanced(6, 1, 0, 0, 0);
        cmd.end_render_pass();

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(&raytracing_output),
            buffer: None,
            state_before: gfx::ResourceState::ShaderResource,
            state_after: gfx::ResourceState::UnorderedAccess,
        });
        cmd.end_event();

        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&mut device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    device.cleanup_dropped_resources(&swap_chain);

    Ok(())
}
