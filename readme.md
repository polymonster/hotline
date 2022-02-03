# Hotline

## Design Goals
- An easy to use cross platform graphics/compute/os api for rapid development.
- Hot reloadable, live coding environment (shaders, render graphs, code).
- Concise graphics api frontend with low level feature access think somewhere in-between Metal and Direct3D12.
- A focus on modern rendering (gpu-driven, multi-threaded, bindless, ray-tracing).
- With flexibility to easily create and use different rendering strategies (deferred vs forward, gpu-driven vs cpu driven, etc).
- Hardware accellerated video decoding.
- Fibre based, multi-threaded, easily scalable to utilise available cpu and gpu.
- Data-driven and configurable.
- Plugin based an extendible.

### Roadmap
- API Frontend (gfx::, os::) / Backend (d3d12::, win32::)  <- In Progress!
- Imgui support w/ Viewports (maybe alternatives, but multi-window 'viewports' is important)
- API Frontend (av::) / Windows Media Foundation (HW Video / Audio Decoding)
- Samples and Demos
- Linux
- Vulkan
- macOS
- Metal
 


