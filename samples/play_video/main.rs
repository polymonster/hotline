use hotline::*;

use os::App;
use os::Window;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use av::VideoPlayer;

#[cfg(target_os = "windows")]
use hotline::os::win32 as os_platform;
use hotline::gfx::d3d12 as gfx_platform;
use hotline::av::wmf as av_platform;

fn main() -> Result<(), hotline::Error> {
    // app
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("window_set_rect"),
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
        title: String::from("play video!"),
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
    let asset_path = exe_path.parent().unwrap();

    let mut imgui_info = imgui::ImGuiInfo {
        device: &mut dev,
        swap_chain: &mut swap_chain,
        main_window: &win,
        fonts: vec![asset_path
            .join("..\\..\\samples\\imgui_demo\\Roboto-Medium.ttf")
            .to_str()
            .unwrap()
            .to_string()],
    };
    let mut imgui = imgui::ImGui::create(&mut imgui_info).unwrap();

    let mut player = av_platform::VideoPlayer::create(&dev).unwrap();

    // ..
    let mut ci = 0;
    let mut player_open = true;
    while app.run() {

        win.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut dev, &win, &mut cmdbuffer);
        cmdbuffer.reset(&swap_chain);

        // main pass
        cmdbuffer.begin_event(0xff0000ff, "Main Pass");

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let mut pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(&mut pass);

        // imgui
        imgui.new_frame(&mut app, &mut win, &mut dev);

        player.update(&mut dev)?;

        if player.is_ended() {
            println!("ended!");
        }

        if imgui.begin("Video Player", &mut player_open, imgui::WindowFlags::NONE) {
            if imgui.button("Open") {
                if let Ok(files) = os_platform::App::open_file_dialog(os::OpenFileDialogFlags::FILES, &vec![".mp4"]) {
                    if files.len() > 0 {
                        player.set_source(files[0].to_string())?;
                    }
                }
            }
            if player.is_loaded() {
                imgui.same_line();
                if imgui.button("Play") {
                    player.play()?;
                }
                imgui.same_line();
                if imgui.button("Pause") {
                    player.pause()?;
                }
            }
        }

        if let Some(video_tex) = &player.get_texture() {
            imgui.image(video_tex, 1280.0, 720.0);
        }

        imgui.end();

        imgui.render(&mut app, &mut win, &mut dev, &mut cmdbuffer);

        cmdbuffer.end_render_pass();

        cmdbuffer.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(swap_chain.get_backbuffer_texture().clone()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });
        cmdbuffer.end_event();

        cmdbuffer.close(&swap_chain);

        dev.execute(&cmdbuffer);

        swap_chain.swap(&dev);
        ci = (ci + 1) % 4;
    }

    swap_chain.wait_for_last_frame();

    // must wait for the final frame to be completed
    cmdbuffer.reset(&swap_chain);

    Ok(())
}