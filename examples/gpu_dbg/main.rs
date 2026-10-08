// gpu driven debug rendering, shaders append debug draw commands which are drawn with a single `execute_indirect`
// of a mesh shader pipeline. see shaders/gpu_dbg.hlsl for the api and `GpuDbg_demo`

use hotline_rs::{*, prelude::*};

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain, RenderPass, Buffer, Texture, Pipeline};

use maths_rs::prelude::*;

// buffer sizes, the commands and args are 1:1
const MAX_COMMANDS: usize = 4096;
const MAX_VERTICES: usize = 1 << 16;
const MAX_CHARS: usize = 1 << 16;
const PRINT_RING_SIZE: u32 = 1 << 16;

// matches `GpuDbg_DrawIndirectArgs` in shaders/gpu_dbg.hlsl (root constant draw id + DispatchMesh args)
// these are written by the gpu, only the size is used on the cpu
#[allow(dead_code)]
#[repr(C)]
struct DrawIndirectArgs {
    draw_id: u32,
    dispatch: gfx::DispatchArguments
}

// matches `gpu_dbg_view` in shaders/gpu_dbg.hlsl
#[repr(C)]
struct ViewConstants {
    view_projection_matrix: Mat4f,
    screen_size: Vec2f
}

fn create_uav_buffer(dev: &mut gfx_platform::Device, stride: usize, num_elements: usize, usage: gfx::BufferUsage)
    -> Result<gfx_platform::Buffer, hotline_rs::Error> {
    dev.create_buffer::<u8>(&gfx::BufferInfo {
        usage: gfx::BufferUsage::UNORDERED_ACCESS | usage,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride,
        num_elements,
        initial_state: gfx::ResourceState::UnorderedAccess
    }, None)
}

fn transition_buffer(cmd: &mut gfx_platform::CmdBuf, buffer: &gfx_platform::Buffer, before: gfx::ResourceState, after: gfx::ResourceState) {
    cmd.transition_barrier(&gfx::TransitionBarrier {
        texture: None,
        buffer: Some(buffer),
        state_before: before,
        state_after: after,
    });
}

// reads chars written by `GpuDbg_printf` from the persistently mapped print ring, without syncing with the gpu.
// each slot is (lap << 8) | char, so a slot holding the lap we expect for `pos` is a new char and anything else has
// not been written yet, unless it is from a later lap, in which case the gpu has overrun the ring and we skip ahead
struct GpuPrinter {
    ring: *const u32,
    pos: u32,
    line: String
}

impl GpuPrinter {
    fn read(&mut self) {
        loop {
            let slot = self.pos % PRINT_RING_SIZE;
            let lap = (self.pos / PRINT_RING_SIZE).wrapping_add(1) & 0xffffff;
            let value = unsafe { std::ptr::read_volatile(self.ring.add(slot as usize)) };
            let value_lap = value >> 8;
            if value_lap != lap {
                if value_lap > lap {
                    println!("gpu_dbg: print ring overrun, skipping ahead");
                    self.line.clear();
                    self.pos = (value_lap - 1).wrapping_mul(PRINT_RING_SIZE) + slot;
                    continue;
                }
                break;
            }
            match (value & 0xff) as u8 {
                b'\n' => println!("{}", std::mem::take(&mut self.line)),
                c => self.line.push(c as char)
            }
            self.pos = self.pos.wrapping_add(1);
        }
    }
}

// binds the gpu_dbg buffers to u0-u5, pipelines that do not use a buffer will skip it
fn bind_buffers<P: Pipeline>(cmd: &mut gfx_platform::CmdBuf, pipeline: &P, heap: &gfx_platform::Heap, buffers: &[&gfx_platform::Buffer]) {
    for (i, buffer) in buffers.iter().enumerate() {
        cmd.set_binding(pipeline, i as u32, 0, gfx::DescriptorType::UnorderedAccess, heap, buffer.get_uav_index().unwrap());
    }
}

fn main() -> Result<(), hotline_rs::Error> {
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("gpu_dbg"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });
    print!("{}", dev.get_adapter_info());

    let mut win = app.create_window(os::WindowInfo {
        title: String::from("gpu_dbg"),
        rect: os::Rect { x: 100, y: 100, width: 1280, height: 720 },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour { r: 0.3, g: 0.3, b: 0.3, a: 1.0 }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmd = dev.create_cmd_buf(2);

    // gpu_dbg buffers, in register order u0-u4 (sizes match the structs in shaders/gpu_dbg.hlsl)
    let commands = create_uav_buffer(&mut dev, 72, MAX_COMMANDS, gfx::BufferUsage::NONE)?;
    let vertices = create_uav_buffer(&mut dev, 28, MAX_VERTICES, gfx::BufferUsage::NONE)?;
    let args = create_uav_buffer(&mut dev, std::mem::size_of::<DrawIndirectArgs>(), MAX_COMMANDS, gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER)?;
    let counters = create_uav_buffer(&mut dev, 16, 1, gfx::BufferUsage::INDIRECT_ARGUMENT_BUFFER)?;
    let data = create_uav_buffer(&mut dev, 4, MAX_CHARS, gfx::BufferUsage::NONE)?;

    // print ring in cpu readable memory, persistently mapped so we can read gpu prints as they arrive
    let mut print_data = dev.create_buffer::<u8>(&gfx::BufferInfo {
        usage: gfx::BufferUsage::UNORDERED_ACCESS,
        cpu_access: gfx::CpuAccessFlags::READ | gfx::CpuAccessFlags::PERSISTENTLY_MAPPED,
        format: gfx::Format::Unknown,
        stride: 4,
        num_elements: PRINT_RING_SIZE as usize,
        initial_state: gfx::ResourceState::UnorderedAccess
    }, None)?;
    let mut printer = GpuPrinter {
        ring: print_data.map(&gfx::MapInfo { subresource: 0, read_start: 0, read_end: usize::MAX }) as *const u32,
        pos: 0,
        line: String::new()
    };

    let buffers = [&commands, &vertices, &args, &counters, &data, &print_data];

    // sdf font atlas, an 8x8 grid of ascii 32-95
    let font_atlas = image::load_texture_from_file(&mut dev, &hotline_rs::get_data_path("textures/gpu_dbg_atlas.dds"), None)?;

    // pipelines
    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut dev, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/gpu_dbg"))?;
    pmfx.create_compute_pipeline(&dev, "gpu_dbg_reset")?;
    pmfx.create_compute_pipeline(&dev, "gpu_dbg_demo")?;
    pmfx.create_mesh_pipeline(&dev, "gpu_dbg_draw", swap_chain.get_backbuffer_pass())?;

    let fmt = swap_chain.get_backbuffer_pass().get_format_hash();
    let draw = pmfx.get_mesh_pipeline_for_format("gpu_dbg_draw", fmt)?;

    // each command sets the draw id root constant (b1) and dispatches mesh groups
    let command_signature = dev.create_indirect_mesh_command::<DrawIndirectArgs>(
        vec![
            gfx::IndirectArgument {
                argument_type: gfx::IndirectArgumentType::PushConstants,
                arguments: Some(gfx::IndirectTypeArguments {
                    push_constants: gfx::IndirectPushConstantsArguments {
                        slot: draw.get_pipeline_slot(1, 0, gfx::DescriptorType::PushConstants).unwrap().index,
                        offset: 0,
                        num_values: 1
                    }
                })
            },
            gfx::IndirectArgument {
                argument_type: gfx::IndirectArgumentType::DispatchMesh,
                arguments: None
            }
        ],
        Some(draw)
    )?;

    // orbit camera, the demo is z-up
    let mut rot = vec2f(-30.0, 30.0);
    let mut zoom = 2500.0;

    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmd);
        cmd.reset(&swap_chain);

        // camera
        if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
            let drag = app.get_mouse_pos_delta();
            rot -= vec2f(drag.y as f32, drag.x as f32);
        }
        zoom = max(zoom - app.get_mouse_wheel() * 100.0, 100.0);

        let vp_rect = win.get_viewport_rect();
        let aspect = vp_rect.width as f32 / vp_rect.height as f32;
        let proj = Mat4f::create_perspective_projection_lh_yup(f32::deg_to_rad(60.0), aspect, 1.0, 10000.0);
        let cam = Mat4f::from_y_rotation(f32::deg_to_rad(rot.y)) * Mat4f::from_x_rotation(f32::deg_to_rad(rot.x))
            * Mat4f::from_translation(vec3f(0.0, 0.0, zoom));
        let z_up = Mat4f::from_x_rotation(f32::deg_to_rad(-90.0));
        let view = ViewConstants {
            view_projection_matrix: proj * cam.inverse() * z_up,
            screen_size: vec2f(vp_rect.width as f32, vp_rect.height as f32)
        };

        // reset counters and run the demo to generate commands
        cmd.begin_event(0xff00ff00, "GpuDbg Generate");
        for name in ["gpu_dbg_reset", "gpu_dbg_demo"] {
            let pipeline = pmfx.get_compute_pipeline(name)?;
            cmd.set_compute_pipeline(pipeline);
            bind_buffers(&mut cmd, pipeline, dev.get_shader_heap(), &buffers);
            cmd.dispatch(gfx::Size3 { x: 1, y: 1, z: 1 }, gfx::Size3 { x: 1, y: 1, z: 1 });
            for buffer in buffers {
                cmd.uav_barrier(gfx::UavResource::Buffer(buffer));
            }
        }
        cmd.end_event();

        // draw the commands
        cmd.begin_event(0xff0000ff, "GpuDbg Draw");
        transition_buffer(&mut cmd, &args, gfx::ResourceState::UnorderedAccess, gfx::ResourceState::IndirectArgument);
        transition_buffer(&mut cmd, &counters, gfx::ResourceState::UnorderedAccess, gfx::ResourceState::IndirectArgument);
        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
        cmd.set_viewport(&gfx::Viewport::from(vp_rect));
        cmd.set_scissor_rect(&gfx::ScissorRect::from(vp_rect));
        cmd.set_mesh_pipeline(draw);
        bind_buffers(&mut cmd, draw, dev.get_shader_heap(), &buffers);
        cmd.set_binding(draw, 0, 0, gfx::DescriptorType::ShaderResource, dev.get_shader_heap(), font_atlas.get_srv_index().unwrap());
        cmd.push_render_constants(draw, 0, 0, 18, 0, gfx::as_u8_slice(&view));
        cmd.execute_indirect(&command_signature, MAX_COMMANDS as u32, &args, 0, Some(&counters), 0);
        cmd.end_render_pass();

        cmd.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        transition_buffer(&mut cmd, &args, gfx::ResourceState::IndirectArgument, gfx::ResourceState::UnorderedAccess);
        transition_buffer(&mut cmd, &counters, gfx::ResourceState::IndirectArgument, gfx::ResourceState::UnorderedAccess);
        cmd.end_event();

        cmd.close()?;
        dev.execute(&cmd);
        swap_chain.swap(&mut dev);

        // print anything the gpu has written so far
        printer.read();
    }

    swap_chain.wait_for_last_frame();
    dev.cleanup_dropped_resources(&swap_chain);
    Ok(())
}
