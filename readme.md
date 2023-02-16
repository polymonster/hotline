# Hotline
[![tests](https://github.com/polymonster/hotline/actions/workflows/tests.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/tests.yaml)
[![samples](https://github.com/polymonster/hotline/actions/workflows/samples.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/samples.yaml)
[![publish](https://github.com/polymonster/hotline/actions/workflows/publish.yml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/publish.yml)
[![docs](https://img.shields.io/docsrs/hotline-rs/latest)](https://docs.rs/hotline_rs/latest/hotline_rs/index.html)
[![crates](https://img.shields.io/crates/v/hotline-rs)](https://crates.io/crates/hotline-rs)

Hotline is a live coding tool that allows you to edit code, shaders, render pipelines, render graphs and more without restarting the application. It provides a `client` application which remains running for the duration of a session. Code can be reloaded that is inside the dynamic `plugins` and render pipelines can be edited and hot reloaded through `pmfx` files.

## Prequisites

Currently Windows with Direct3D12 is the only suppported platform, there are plans for macOS, Metal, Linux Vulkan and more over time.

## Building Data

The [hotline-data](https://github.com/polymonster/hotline-data) repository is required but it is kept separate to keep the size of the main hotline repository down when running `cargo build` the `hotline-data` repository will be cloned automatically for you.

The [config.jsn](https://github.com/polymonster/hotline/blob/master/config.jsn) is used to configure `pmbuild` build jobs and tools, if you wanted to manually configure the setup or add new steps.

`cargo build` will automatically build data into `target/data` this is where the client and the examples will look for data files.

## Using as a library

You can use hotline as a library to use the low level abstractions and modules to create windowed applications with a graphics api backend. Here is a small example:

```rust
    // Create an Application
    let mut app = os_platform::App::create(os::AppInfo {
        name: String::from("triangle"),
        window: false,
        num_buffers: 0,
        dpi_aware: true,
    });

    // Double buffered
    let num_buffers = 2;

    // Create an a GPU Device
    let mut device = gfx_platform::Device::create(&gfx::DeviceInfo {
        render_target_heap_size: num_buffers,
        ..Default::default()
    });

    // Create main window
    let mut window = app.create_window(os::WindowInfo {
        title: String::from("triangle!"),
        ..Default::default()
    });

    /// Create swap chain
    let swap_chain_info = gfx::SwapChainInfo {
        num_buffers: num_buffers as u32,
        format: gfx::Format::RGBA8n,
        ..Default::default()
    };

    let mut swap_chain = device.create_swap_chain::<os_platform::App>(&swap_chain_info, &window)?;
    
    /// Create a command buffer
    let mut cmd = device.create_cmd_buf(num_buffers);

    while app.run() {
        // update window and swap chain
        window.update(&mut app);
        swap_chain.update::<os_platform::App>(&mut device, &window, &mut cmd);

        // build command buffer and make draw calls
        cmd.reset(&swap_chain);

        // Render command can go here
        // ..

        cmd.close()?;

        // execute command buffer
        device.execute(&cmd);

        // swap for the next frame
        swap_chain.swap(&device);
    }

    // must wait for the final frame to be completed
    swap_chain.wait_for_last_frame();
    cmd.reset(&swap_chain);

    Ok(());
}
```

## Using Hotreload Client

You can run the binary `client` which allows code to be reloaded through `Plugins`. There are some [plugins](https://github.com/polymonster/hotline/tree/master/plugins) already provided with the repository.


```text
// build the client and data
cargo build

// then build plugins
cargo build --manifest-path plugins/Cargo.toml

// run the client
cargo run client
```

You can then use the visual client to locate `Cargo.toml` files inside the `plugins` directory. The `ecs` plugin provides a basic wrapper around `bevy_ecs` and `scheduler`.

Any code changes made to the plugin libs will cause a rebuild and reload to happen with the client still running. You can also edit the [shaders](https://github.com/polymonster/hotline/tree/master/src/shaders) where `hlsl` files make up the shader code and `pmfx` files allow you to sepcify pipeline state objects in config files. Any changes detected to `pmfx` shaders will be rebuilt and all modified pipelines or views will be rebuilt.

## Examples

There are a few standalone examples of how to use the lower level components of hotline (`gfx, app, av`). You can build and run these as follows:

```text
cargo build --examples
cargo run --example triangle
```

## VSCode

There are included `tasks` and `launch` files for vscode including configurations for the client and the examples.

## Design Goals
- An easy to use cross platform graphics/compute/os api for rapid development.
- Hot reloadable, live coding environment (shaders, render graphs, code).
- Concise low level graphics api... think somewhere in-between Metal and Direct3D12.
- High level data driven graphics api for ease of use and speed.
- A focus on modern rendering examples (gpu-driven, multi-threaded, bindless, ray-tracing).
- Flexibility to easily create and use different rendering strategies (deferred vs forward, gpu-driven vs cpu driven, etc).
- Hardware accellerated video decoding.
- Fibre based, multi-threaded, easily scalable to utilise available cpu and gpu.
- Data-driven and configurable.
- Plugin based and extendible...

## Roadmap

#### In Progress
- Debug / Primitve Rendering API
- High level graphics api (render graphs, data driven, Uber shaders)
- Multi-threading support (async command buffer generation and job dispatches)
- Samples and Demos
- ~~Hot reloading~~
- ~~API (gfx::, os::) / Backend (d3d12::, win32::)~~
- ~~API (av::) / Windows Media Foundation (HW Video / Audio Decoding)~~
- ~~Imgui support w/ Viewports~~

#### Future Plans
- Linux
- Vulkan
- macOS
- Metal
- AV Foundation
- WASM
- WebGPU

## Contributing

Contributions of all kinds are welcome, you can make a fork and send a PR if you want to submit small fixes or improvements. Anyone interseted in being more involved in development I am happy to take on people to help with project of all experience levels, especially people with more experience in Rust. You can contact me if interested via [Twitter](twitter.com/polymonster) or [Discord](https://discord.com/invite/3yjXwJ8wJC).
 
