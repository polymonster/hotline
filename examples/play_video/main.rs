use hotline_rs::*;

use os::App;
use os::Window;

use gfx::CmdBuf;
use gfx::Device;
use gfx::SwapChain;

use av::VideoPlayer;

#[cfg(target_os = "windows")]
use hotline_rs::os::win32 as os_platform;
use hotline_rs::gfx::d3d12 as gfx_platform;
use hotline_rs::av::wmf as av_platform;

fn main() -> Result<(), hotline_rs::Error> {
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

    let roboto = asset_path
        .join("..\\..\\..\\examples\\imgui_demo\\Roboto-Medium.ttf")
        .to_str()
        .unwrap()
        .to_string();

    let font_awesome = asset_path
        .join("..\\..\\..\\examples\\imgui_demo\\FontAwesome.ttf")
        .to_str()
        .unwrap()
        .to_string(); 

    let mut imgui_info = imgui::ImGuiInfo {
        device: &mut dev,
        swap_chain: &mut swap_chain,
        main_window: &win,
        fonts: vec![
            imgui::FontInfo{
                filepath: roboto,
                glyph_ranges: None 
            },
            imgui::FontInfo{
                filepath: font_awesome,
                glyph_ranges: Some(vec![
                    [font_awesome::MINIMUM_CODEPOINT as u32, font_awesome::MAXIMUM_CODEPOINT as u32]
                ])
            }
        ],
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
            texture: Some(swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        let pass = swap_chain.get_backbuffer_pass_mut();
        cmdbuffer.begin_render_pass(pass);

        // imgui
        imgui.new_frame(&mut app, &mut win, &mut dev);

        player.update(&mut dev)?;

        if player.is_ended() {
            println!("ended!");
        }

        if imgui.begin("Video Player", &mut player_open, imgui::WindowFlags::ALWAYS_AUTO_RESIZE) {
            if imgui.button("Open") {
                if let Ok(files) = os_platform::App::open_file_dialog(os::OpenFileDialogFlags::FILES, vec![".mp4"]) {
                    if !files.is_empty() {
                        player.set_source(files[0].to_string())?;
                    }
                }
            }
            if player.is_loaded() {
                imgui.same_line();
                if imgui.button(font_awesome::strs::PLAY) {
                    player.play()?;
                }
                imgui.same_line();
                if imgui.button(font_awesome::strs::PAUSE) {
                    player.pause()?;
                }
            }
        }

        if let Some(video_tex) = &player.get_texture() {
            let size = player.get_size();
            imgui.image(video_tex, size.x as f32, size.y as f32);
        }

        imgui.end();

        imgui.render(&mut app, &mut win, &mut dev, &mut cmdbuffer);

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

    // must wait for the final frame to be completed
    cmdbuffer.reset(&swap_chain);

    Ok(())
}