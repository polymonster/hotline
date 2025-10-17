// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::*;

use os::{App, Window};
use gfx::{CmdBuf, Device, SwapChain};


fn main() -> Result<(), hotline_rs::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("imgui"),
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

    // window
    let mut win = app.create_window(os::WindowInfo {
        title: String::from("imgui!"),
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
            r: 0.45,
            g: 0.55,
            b: 0.60,
            a: 1.00,
        }),
    };
    let mut swap_chain = dev.create_swap_chain::<os_platform::App>(&swap_chain_info, &win)?;
    let mut cmdbuffer = dev.create_cmd_buf(2);

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap().join("../..");

    let font_path = asset_path
        .join("data/fonts/roboto_medium.ttf")
        .to_str()
        .unwrap()
        .to_string();

    let mut imgui_info = imgui::ImGuiInfo {
        device: &mut dev,
        swap_chain: &mut swap_chain,
        main_window: &win,
        fonts: vec![imgui::FontInfo {
            filepath: font_path,
            glyph_ranges: None
        }],
    };
    let mut imgui = imgui::ImGui::create(&mut imgui_info).unwrap();

    // ..
    let mut ci = 0;
    while app.run() {

        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // main pass
        cmdbuffer.begin_event(0xff0000ff, "Main Pass");

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);

        // imgui
        imgui.new_frame(&mut app, &mut win, &mut dev);
        imgui.demo();
        imgui.render(&mut app, &mut win, &mut dev, &mut cmdbuffer, &Vec::new());

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

        swap_chain.swap(&dev);
        ci = (ci + 1) % 4;
    }

    swap_chain.wait_for_last_frame();

    // resources now no longer in use they can be properly cleaned up
    dev.cleanup_dropped_resources(&swap_chain);
    
    Ok(())
}
