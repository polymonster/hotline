// Resolution independent text rendered directly from glyph outlines with the Slug algorithm, see `hotline_rs::slug`.
// Scroll to zoom about the cursor and drag with the left mouse button to pan, text stays sharp at any scale

use hotline_rs::{*, prelude::*};

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain, RenderPass};
use slug::{SlugFont, SlugParams, SlugVertex};

const TEXT_SIZES: [f32; 12] = [10.0, 12.0, 14.0, 16.0, 20.0, 24.0, 32.0, 48.0, 64.0, 96.0, 144.0, 200.0];
const MARGIN: f32 = 32.0;

fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("slug_text"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // device
    let mut dev = gfx_platform::Device::create(&gfx::DeviceInfo {
        adapter_name: None,
        shader_heap_size: 100,
        render_target_heap_size: 100,
        depth_stencil_heap_size: 100,
    });
    print!("{}", dev.get_adapter_info());

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("slug_text"),
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
        num_buffers: 2,
        format: gfx::Format::RGBA8n,
        clear_colour: Some(gfx::ClearColour {
            r: 0.08,
            g: 0.09,
            b: 0.11,
            a: 1.00,
        }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmdbuffer = dev.create_cmd_buf(2);

    // compile printable ascii from the font into curve and band textures
    let font_data = std::fs::read(hotline_rs::get_data_path("fonts/roboto_medium.ttf"))?;
    let font = SlugFont::from_ttf(&font_data, ' '..='~')?;
    let (curve_texture, band_texture) = font.create_textures(&mut dev)?;
    let (curve_rows, band_rows) = font.texture_rows();
    println!("slug_text: {} glyphs, curve texture 4096x{curve_rows}, band texture 4096x{band_rows}", font.glyphs.len());

    // lay the text out down the page in object space (pixels at zoom 1), y up with the top of the page at 0
    let mut vertices: Vec<SlugVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut baseline = 0.0;
    let mut line = |text: &str, size: f32, colour: [f32; 4], baseline: &mut f32| {
        *baseline -= font.ascender * size;
        font.layout(text, [0.0, *baseline], size, colour, &mut vertices, &mut indices);
        *baseline -= (font.line_height() - font.ascender) * size;
    };
    line("Slug: GPU text rendering directly from glyph outlines", 40.0, [1.0, 0.75, 0.2, 1.0], &mut baseline);
    line("Scroll to zoom, drag to pan. No atlas, no signed distance field, just the curves.", 18.0, [0.6, 0.6, 0.6, 0.6], &mut baseline);
    for size in TEXT_SIZES {
        let text = format!("{size}px  The quick brown fox jumps over the lazy dog 0123456789 &@?!");
        line(&text, size, [0.92, 0.92, 0.92, 1.0], &mut baseline);
    }

    let vertex_buffer = dev.create_buffer(&gfx::BufferInfo {
        usage: gfx::BufferUsage::VERTEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::Unknown,
        stride: std::mem::size_of::<SlugVertex>(),
        num_elements: vertices.len(),
        initial_state: gfx::ResourceState::VertexConstantBuffer
    }, Some(vertices.as_slice()))?;

    let index_buffer = dev.create_buffer(&gfx::BufferInfo {
        usage: gfx::BufferUsage::INDEX,
        cpu_access: gfx::CpuAccessFlags::NONE,
        format: gfx::Format::R32u,
        stride: std::mem::size_of::<u32>(),
        num_elements: indices.len(),
        initial_state: gfx::ResourceState::IndexBuffer
    }, Some(indices.as_slice()))?;

    let mut pmfx : pmfx::Pmfx<gfx_platform::Device> = pmfx::Pmfx::create(&mut dev, 0);
    pmfx.load(&hotline_rs::get_data_path("shaders/slug"))?;
    pmfx.create_render_pipeline(&dev, "slug_text", swap_chain.get_backbuffer_pass())?;
    let fmt = swap_chain.get_backbuffer_pass().get_format_hash();
    let pso = pmfx.get_render_pipeline_for_format("slug_text", fmt)?;

    // view: screen pixels (y up from the bottom of the viewport) = object * zoom + pan
    let mut zoom = 1.0f32;
    let mut pan: Option<[f32; 2]> = None;

    while app.run() {
        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        let vp_rect = win.get_viewport_rect();
        let (w, h) = (vp_rect.width as f32, vp_rect.height as f32);
        let pan = pan.get_or_insert([MARGIN, h - MARGIN]);

        if win.is_focused() && win.is_mouse_hovered() {
            let client = win.get_mouse_client_pos(app.get_mouse_pos());
            let cursor = [client.x as f32, h - client.y as f32];
            let wheel = app.get_mouse_wheel();
            if wheel != 0.0 {
                // zoom about the cursor, keeping the object point under it fixed
                let new_zoom = (zoom * 1.1f32.powf(wheel)).clamp(0.05, 200.0);
                for i in 0..2 {
                    pan[i] = cursor[i] - (cursor[i] - pan[i]) * new_zoom / zoom;
                }
                zoom = new_zoom;
            }
            if app.get_mouse_buttons()[os::MouseButton::Left as usize] {
                let drag = app.get_mouse_pos_delta();
                pan[0] += drag.x as f32;
                pan[1] -= drag.y as f32;
            }
        }

        // rows of the mvp, an orthographic projection of the view
        let params = SlugParams {
            matrix: [
                [2.0 * zoom / w, 0.0, 0.0, 2.0 * pan[0] / w - 1.0],
                [0.0, 2.0 * zoom / h, 0.0, 2.0 * pan[1] / h - 1.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0]
            ],
            viewport: [w, h, 0.0, 0.0]
        };

        cmdbuffer.begin_event(0xff0000ff, "Slug Text");
        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);
        cmdbuffer.set_viewport(&gfx::Viewport::from(vp_rect));
        cmdbuffer.set_scissor_rect(&gfx::ScissorRect::from(vp_rect));

        cmdbuffer.set_render_pipeline(pso);
        cmdbuffer.set_heap(pso, dev.get_shader_heap());
        cmdbuffer.set_binding(pso, 0, 0, gfx::DescriptorType::ShaderResource, dev.get_shader_heap(),
            curve_texture.get_srv_index().unwrap());
        cmdbuffer.set_binding(pso, 1, 0, gfx::DescriptorType::ShaderResource, dev.get_shader_heap(),
            band_texture.get_srv_index().unwrap());
        cmdbuffer.push_render_constants(pso, 0, 0, gfx::num_32bit_constants(&params), 0, gfx::as_u8_slice(&params));

        cmdbuffer.set_index_buffer(&index_buffer);
        cmdbuffer.set_vertex_buffer(&vertex_buffer, 0);
        cmdbuffer.draw_indexed_instanced(indices.len() as u32, 1, 0, 0, 0);

        cmdbuffer.end_render_pass();
        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        cmdbuffer.end_event();
        cmdbuffer.close()?;

        dev.execute(&cmdbuffer);
        swap_chain.swap(&mut dev);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    dev.cleanup_dropped_resources(&swap_chain);

    Ok(())
}
