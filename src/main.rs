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

pub struct ClearCol {
    r: f32,
    g: f32,
    b: f32,
    a: f32
}

fn main() {
    let instarc = platform::Instance::create();
    let dev = d3d12::Device::create();

    let mut win = instarc.create_window(os::WindowInfo { 
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

    let magenta = ClearCol {
        r: 1.0,
        g: 0.0, 
        b: 1.0,
        a: 1.0
    };

    let yellow = ClearCol {
        r: 1.0,
        g: 1.0, 
        b: 0.0,
        a: 1.0
    };

    let cyan = ClearCol {
        r: 0.0,
        g: 1.0, 
        b: 1.0,
        a: 1.0
    };

    let green = ClearCol {
        r: 0.0,
        g: 1.0, 
        b: 0.0,
        a: 1.0
    };

    let mut clears : [ClearCol; 4] = [
        magenta, yellow, cyan, green
    ];

    let mut ci = 0;

    while instarc.run() {
        win.update();
        swap_chain.update(&dev, &win);

        swap_chain.new_frame();
        
        let window_rect = win.get_rect();

        // TODO: utility func?
        let viewport = gfx::Viewport {
            x: window_rect.x as f32,
            y: window_rect.y as f32,
            width: window_rect.width as f32,
            height: window_rect.height as f32,
            min_depth: 0.0,
            max_depth: 1.0
        };

        let scissor = gfx::ScissorRect {
            left: window_rect.x,
            top: window_rect.y,
            right: window_rect.width,
            bottom: window_rect.height
        };
        
        cmdbuffer.reset(&swap_chain);

        let col = &clears[ci];
        cmdbuffer.clear_debug(&swap_chain, col.r, col.g, col.b, col.a); //

        cmdbuffer.set_viewport(&viewport);
        cmdbuffer.set_scissor_rect(&scissor);

        cmdbuffer.set_state_debug(); //
        cmdbuffer.draw_instanced(3, 1, 0, 0);
        cmdbuffer.close_debug(&swap_chain); //

        dev.execute(&cmdbuffer);
        swap_chain.swap(&dev);

        std::thread::sleep_ms(32);
        ci = (ci + 1) % 4;
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