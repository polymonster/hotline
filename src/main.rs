use os::Instance;
use os::Window;

use gfx::Device;
use gfx::SwapChain;
use gfx::CmdBuf;

use std::sync::Arc;
use std::thread;
use std::time;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
use win32 as platform;

pub struct Toot {
    value: Mutex<i32>
}

fn main() {
    let instarc = platform::Instance::create();
    let dev = d3d12::Device::create();

    let win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });


    let mut swap_chain = dev.create_swap_chain(&win);
    let mut cmdbuffer = dev.create_cmd_buf();

    while instarc.run() {
        swap_chain.new_frame();
        
        cmdbuffer.reset(&swap_chain);
        cmdbuffer.clear_debug(&swap_chain);

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);
    }

}

#[test]
fn aync_mut_device_test() {
    let instarc = platform::Instance::create();
    let dev = Arc::new(Mutex::new(d3d12::Device::create()));
    let ten_millis = time::Duration::from_millis(10);
    let d2 = dev.clone();
    thread::spawn(move || {
        {
            let dd = d2.lock().unwrap();
            //dd.create_queue();
        }
        loop {
            {
                let mut dd = d2.lock().unwrap();
                dd.test_mutate();
            }
                       
            thread::sleep(ten_millis);
        }
    });

    while instarc.run() {
        dev.lock().unwrap().print_mutate();
        thread::sleep(ten_millis);
    }
}

#[test]
fn aync_test() {
    let instarc = platform::Instance::create();
    let ttt = Arc::new(Toot {
        value: Mutex::new(69)
    });

    let t3 = ttt.clone();
    thread::spawn(move || {
        loop {
            let mut t4 = t3.value.lock().unwrap();
            *t4 += 1;
        }
    });

    while instarc.run() {
        let t5 = ttt.value.lock().unwrap();
        println!("ttt = {}", t5);
    }
}

#[test]
fn create_queue() {
    let instarc = platform::Instance::create();
    let dev = d3d12::Device::create();

    let win = instarc.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });

    //let mut queue = dev.create_queue();
    //queue.create_swap_chain(dev, win);
}

#[test]
fn create_device() {
    let instarc = Arc::new(platform::Instance::create());
    let dev = d3d12::Device::create();
}

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