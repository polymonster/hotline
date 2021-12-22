use os::Instance;
use os::Window;

use gfx::Device;

use std::sync::Arc;

#[cfg(target_os = "windows")]
use win32 as platform;

pub struct Test {
    pub boobs : i32
}

fn main() {
    let instarc = Arc::new(platform::Instance::create());

    let mut win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });

    win.set_rect(os::Rect {
        x : 200,
        y : 0,
        width : 1280,
        height : 720
    });

    win.set_size(1280, 400);

    let winrect = win.get_rect();
    println!("winrect = x = {}, y = {}, width = {}, height = {}", 
        winrect.x, 
        winrect.y, 
        winrect.width, 
        winrect.height
    );

    let winsize = win.get_size();
    println!("winsize = width = {}, height = {}", 
        winsize.0, 
        winsize.1
    );

    while instarc.run() {
        // println!("I am Running!");
    }
}

#[test]
fn create_device() {
    let instarc = Arc::new(platform::Instance::create());
    let dev = d3d12::Device::create();
}

/*
#[test]
fn window_spawn() {
    let instarc = Arc::new(platform::Instance::create());
    let win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 0);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}

#[test]
fn window_set_rect() {
    let instarc = Arc::new(platform::Instance::create());
    let mut win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });
    win.set_rect(os::Rect {
        x : 200,
        y : 0,
        width : 1280,
        height : 720
    });
    let winrect = win.get_rect();
    assert_eq!(winrect.x, 200);
    assert_eq!(winrect.y, 0);
    assert_eq!(winrect.width, 1280);
    assert_eq!(winrect.height, 720);
}
*/