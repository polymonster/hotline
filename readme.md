# Hotline
[![tests](https://github.com/polymonster/hotline/actions/workflows/tests.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/tests.yaml)
[![samples](https://github.com/polymonster/hotline/actions/workflows/samples.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/samples.yaml)
[![docs](https://img.shields.io/docsrs/hotline-rs/latest)](https://docs.rs/hotline_rs/latest/hotline_rs/index.html)
[![crates](https://img.shields.io/crates/v/hotline-rs)](https://crates.io/crates/hotline-rs)

Hotline is a live coding tool where you can editor code, shaders, render pipelines, render graphs and more without restarting the application. It provides a `host` application which remains running for the duration of a session. Code can be reloaded that is inside the dynamic `lib` and render pipelines and shaders can be edited and hot reloaded through `pmfx` files.

## Building

Currently Windows with Direct3D12 is the only suppported platform, there are plans for macOS, Metal, Linux Vulkan and more over time.

First build the live `lib` and then build the `host` executable and data (data will be built automatically when using `cargo build`). Then run the `host`:

```text
cargo build -p lib
cargo build
cargo run host
```

Once the `host` is running, any changes made to the source code in [lib.rs](https://github.com/polymonster/hotline/blob/master/lib/src/lib.rs) or the [pipelines](https://github.com/polymonster/hotline/blob/master/src/shaders) will be automatically reloaded into the running application. 

### Examples

There are a few standalong examples of how to use the lowr level components of hotline (`gfx, app, av`). You can build and run these as follows:

```text
cargo build --examples
cargo run --example triangle
```

### VScode

There are include `tasks` and `launch` files for vscode including aunc configurations for the host and the examples.


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
- ~~API (gfx::, os::) / Backend (d3d12::, win32::)~~
- ~~API (av::) / Windows Media Foundation (HW Video / Audio Decoding)~~
- ~~Imgui support w/ Viewports~~

#### Future Plans
- Hot reloading
- Samples and Demos
- Linux
- Vulkan
- macOS
- Metal
- AV Foundation
- WASM
- WebGPU

## Contributing

Contributions of all kinds are welcome, you can make a fork and send a PR if you want to submit small fixes or improvements. Anyone interseted in being more involved in development I am happy to take on people to help with project of all experience levels, especially people with more experience in Rust. You can contact me if interested via [Twitter](twitter.com/polymonster) or [Discord](https://discord.com/invite/3yjXwJ8wJC).
 
