use os::Instance;
use gfx::Device;

#[cfg(target_os = "windows")]
use win32 as platform;

fn main() {
    let inst = platform::Instance::create();
    let win = inst.create_window(os::WindowInfo { 
        title : String::from("hello world!"),
        rect : os::Rect {
            x : 0,
            y : 0,
            width : 1280,
            height : 720
        }
    });

    let dev = d3d12::Device::create();

    while inst.run() {
        // println!("I am Running!");
    }
}