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
use hotline::av::winmf as av_platform;

fn main() {
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
        title: String::from("play_video!"),
        rect: os::Rect {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
        },
        style: os::WindowStyleFlags::NONE,
        parent_handle: None,
    });

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let video_path = asset_path.join("..\\..\\samples\\play_video\\touch_video_logo.mp4");

    // video player
    let player = av_platform::VideoPlayer::create(&dev).unwrap();
    player.set_source(String::from(video_path.to_str().unwrap()));
    player.play();

    while app.run() {
        win.update(&mut app);

        // wait until player is ready to play
        if player.is_loaded() && !player.is_playing() {
            player.play();
        }

        if player.is_playing() {
            player.transfer_frame();
        }

        if player.is_ended() {
            println!("ended!");
        }
    }
}