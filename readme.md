# Hotline

## Design Goals
- An easy to use cross platform graphics/compute/os api for rapid development.
- Hot reloadable, live coding environment (shaders, render graphs, code).
- Concise low level graphics api... think somewhere in-between Metal and Direct3D12.
- High level data driven graphics api for ease of and speed.
- A focus on modern rendering examples (gpu-driven, multi-threaded, bindless, ray-tracing).
- Flexibility to easily create and use different rendering strategies (deferred vs forward, gpu-driven vs cpu driven, etc).
- Hardware accellerated video decoding.
- Fibre based, multi-threaded, easily scalable to utilise available cpu and gpu.
- Data-driven and configurable.
- Plugin based and extendible...

## Roadmap

#### In Progress
- API (gfx::, os::) / Backend (d3d12::, win32::)

#### Future Work
- Imgui support w/ Viewports (maybe alternatives, but multi-window 'viewports' is important)
- Multi-threading support (async command buffer generation and job dispatches)
- High level graphics api (render graphs, data driven, Uber shaders)
- Hot reloading
- API (av::) / Windows Media Foundation (HW Video / Audio Decoding)
- Samples and Demos
- Linux
- Vulkan
- macOS
- Metal
- AV Foundation
- WASM
- WebGPU

## Contributing

Contributions of all kinds are welcome, you can make a fork and send a PR if you want to submit small fixes or improvements. Anyone interseted in being more involved in development I am happy to take on people to help with project of all experience levels. I would be happy to mentor a limited number junior devs interested in graphics, or work with experienced Rust devs to help me learn more about Rust. You can contact me if interested via [twitter](twitter.com/polymonster), [discord](https://discord.com/invite/3yjXwJ8wJC).
 


