# Hotline

[![tests](https://github.com/polymonster/hotline/actions/workflows/tests.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/tests.yaml)
[![samples](https://github.com/polymonster/hotline/actions/workflows/samples.yaml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/samples.yaml)
[![publish](https://github.com/polymonster/hotline/actions/workflows/publish.yml/badge.svg)](https://github.com/polymonster/hotline/actions/workflows/publish.yml)
[![docs](https://img.shields.io/docsrs/hotline-rs/latest)](https://docs.rs/hotline_rs/latest/hotline_rs/index.html)
[![crates](https://img.shields.io/crates/v/hotline-rs)](https://crates.io/crates/hotline-rs)

Hotline is a live coding tool that allows you to edit code, shaders, render pipelines, render graphs and more without restarting the application. It provides a `client` application which remains running for the duration of a session. Code can be reloaded that is inside the dynamic `plugins` and render pipelines can be edited and hot reloaded through `pmfx` files.

[<img src="https://raw.githubusercontent.com/polymonster/polymonster.github.io/master/images/hotline/geom_final.gif" width="100%"/>]  

There is a demo [video](https://www.youtube.com/watch?v=jkD78gXfIe0&) showcasing the features in their early stages.

## Prerequisites

Currently Windows with Direct3D12 is the only supported platform, there are plans for macOS, Metal, Linux Vulkan and more over time.

## Building Data

The [hotline-data](https://github.com/polymonster/hotline-data) repository is required but it is kept separate to keep the size of the main hotline repository down when running `cargo build` the `hotline-data` repository will be cloned automatically for you.

The [config.jsn](https://github.com/polymonster/hotline/blob/master/config.jsn) is used to configure [pmbuild](https://github.com/polymonster/pmbuild) build jobs and tools, if you wanted to manually configure the setup or add new steps.

```text
// fetch the data repository and build the library and client
cargo build

// build the data to target/data
.\hotline-data\pmbuild.cmd win32-data
```

## Using the Client

You can run the binary `client` which allows code to be reloaded through `plugins`. There are some [plugins](https://github.com/polymonster/hotline/tree/master/plugins) already provided with the repository:

```text
// build the hotline library, and the client, fetch the hotline-data repository
cargo build

// build the data
.\hotline-data\pmbuild.cmd win32-data

// then build plugins
cargo build --manifest-path plugins/Cargo.toml

// run the client
cargo run client
```

Any code changes made to the plugin libs will cause a rebuild and reload to happen with the client still running. You can also edit the [shaders](https://github.com/polymonster/hotline/tree/master/src/shaders) where `hlsl` files make up the shader code and `pmfx` files allow you to specify pipeline state objects in config files. Any changes detected to `pmfx` shaders will be rebuilt and all modified pipelines or views will be rebuilt.

### Building One-Liners

To make things more convenient during development and keep the `plugins`, `client` and `lib` all in sync and make switching configurations easily, you can use the bundled `pmbuild` in the `hotline-data` repository and use the following commands which bundle together build steps:

```text
// build release
.\hotline-data\pmbuild.cmd win32-release

// build debug
.\hotline-data\pmbuild.cmd win32-debug

// run the client 
.\hotline-data\pmbuild.cmd win32-debug -run

// build and run the client 
.\hotline-data\pmbuild.cmd win32-release -all -run
```

### Building from VSCode

There are included `tasks` and `launch` files for vscode including configurations for the client and the examples. Launching the `client` from VSCode in debug or release will build the core hotline `lib`, `client`, `data` and `plugins`.  

## Adding Plugins

Plugins are loaded by passing a directory to [add_plugin_lib](https://docs.rs/hotline-rs/latest/hotline_rs/client/struct.Client.html#method.add_plugin_lib) which contains a `Cargo.toml` and is a dynamic library. They can be opened interactively in the client using the `File > Open` from the main menu bar by selecting the `Cargo.toml`.

The basic `Cargo.toml` setup looks like this:

```toml
[package]
name = "ecs_demos"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["rlib", "dylib"]

[dependencies]
hotline-rs = { path = "../.." }
```

You can provide your own plugins implementations using the [Plugin](https://docs.rs/hotline-rs/latest/hotline_rs/plugin/trait.Plugin.html) trait. A basic plugin can hook itself by implementing a few functions:

```rust
impl Plugin<gfx_platform::Device, os_platform::App> for EmptyPlugin {
    fn create() -> Self {
        EmptyPlugin {
        }
    }

    fn setup(&mut self, client: Client<gfx_platform::Device, os_platform::App>) 
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin setup");
        client
    }

    fn update(&mut self, client: client::Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin update");
        client
    }

    fn unload(&mut self, client: Client<gfx_platform::Device, os_platform::App>)
        -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin unload");
        client
    }

    fn ui(&mut self, client: Client<gfx_platform::Device, os_platform::App>)
    -> Client<gfx_platform::Device, os_platform::App> {
        println!("plugin ui");
        client
    }
}

// the macro instantiates the plugin with a c-abi so it can be loaded dynamically.
hotline_plugin![EmptyPlugin];
```

### Ecs Plugin

There is a core `ecs` plugin which builds on top of [bevy_ecs](https://docs.rs/bevy_ecs/latest/bevy_ecs/). It allows you to supply your own systems and build schedules dynamically. It is possible to load and find new `ecs` systems in different dynamic libraries. You can register and instantiate `demos` which are collections of `setup`, `update` and `render` systems.

#### Initialisation Functions

You can set up a new ecs demo by providing an initialisation function named after the demo this returns a `ScheduleInfo` for which systems to run:

```rust
/// Init function for primitives demo
#[no_mangle]
pub fn primitives(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {
    // load resources we may need
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/debug").as_str()).unwrap();
    
    // fill out info
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        update: systems![
            "update_cameras",
            "update_main_camera_config"
        ],
        render_graph: "mesh_debug".to_string()
    }
}
```

#### Setup Systems

You can supply `setup` systems to add entities into a scene, when a dynamic code reload happens the world will be cleared and the setup systems will be re-executed. This allows changes to setup systems to appear in the live `client`. You can add multiple `setup` systems and they will be executed concurrently.

```rust
#[no_mangle]
pub fn setup_cube(
    mut device: bevy_ecs::change_detection::ResMut<DeviceRes>,
    mut commands: bevy_ecs::system::Commands) {

    let pos = Mat4f::from_translation(Vec3f::unit_y() * 10.0);
    let scale = Mat4f::from_scale(splat3f(10.0));

    let cube_mesh = hotline_rs::primitives::create_cube_mesh(&mut device.0);
    commands.spawn((
        Position(Vec3f::zero()),
        Velocity(Vec3f::one()),
        MeshComponent(cube_mesh.clone()),
        WorldMatrix(pos * scale)
    ));
}
```

#### Render Systems

You can specify render graphs in `pmfx` which set up `views` which get dispatched into `render` functions. All render systems run concurrently on the CPU, the command buffers they generate are executed in an order determined by the `pmfx` render graph and it's dependencies.

```rust
#[no_mangle]
pub fn render_meshes(
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    mesh_draw_query: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let fmt = view.pass.get_format_hash();
    let mesh_debug = pmfx.get_render_pipeline_for_format(&view.view_pipeline, fmt)?;
    let camera = pmfx.get_camera_constants(&view.camera)?;

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);
    view.cmd_buf.set_render_pipeline(&mesh_debug);
    view.cmd_buf.push_constants(0, 16 * 3, 0, gfx::as_u8_slice(camera));

    // make draw calls
    for (world_matrix, mesh) in &mesh_draw_query {
        view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
        view.cmd_buf.set_index_buffer(&mesh.0.ib);
        view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
        view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();
    Ok(())
}
```

#### Update Systems

You can also supply your own `update` systems to animate and move your entities, these too are all executed concurrently.

```rust
fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>, 
    mut query: Query<(&mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.0;
    for (mut position, mut rotation, mut view_proj) in &mut query {
        // ..
    }
}
```

#### Registering Systems

Systems can be imported dynamically from different plugins, in order to do so they need to be hooked into a function which can be located dynamically by the `ecs` plugin. In time I hope to be able to remove this baggage and be able to `#[derive()]` them.  

You can implement a function called `get_demos_<lib_name>` which returns a list of available demos inside a `plugin` named `<lib_name>` and `get_system_<lib_name>` to return `bevy_ecs::SystemDescriptor` of systems which can then be looked up by name, the ecs plugin will search for systems by name within all other loaded plugins, so you can build and share functionality.

```rust
/// Register demo names
#[no_mangle]
pub fn get_demos_ecs_demos() -> Vec<String> {
    demos![
        "primitives",
        "draw_indexed",
        "draw_indexed_push_constants",

        // ..
    ]
}

/// Register plugin system functions
#[no_mangle]
pub fn get_system_ecs_demos(name: String, view_name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        // setup functions
        "setup_draw_indexed" => system_func![setup_draw_indexed],
        "setup_primitives" => system_func![setup_primitives],
        "setup_draw_indexed_push_constants" => system_func![setup_draw_indexed_push_constants],
        // render functions
        "render_meshes" => render_func![render_meshes, view_name],
        "render_wireframe" => render_func![render_wireframe, view_name],
        _ => std::hint::black_box(None)
    }
}
```
### Serialising Plugin Data

You can supply your own serialisable plugin data which will be serialised with the rest of the `user_config` and can be grouped with your plugin and reloaded between sessions.

```rust
/// Seriablisable user info for maintaining state between reloads and sessions
#[derive(Serialize, Deserialize, Default, Resource, Clone)]
pub struct SessionInfo {
    pub active_demo: String,
    pub main_camera: Option<CameraInfo>
}

// the client provides functions which can serialise and deserialise this data for you
fn update_user_config(&mut self) {
    // find plugin data for the "ecs" plugin
    self.session_info = client.deserialise_plugin_data("ecs");

    //.. make updates to your data here

    // write back session info which will be serialised to disk and reloaded between sessions
    client.serialise_plugin_data("ecs", &self.session_info);
}
```

## Using as a library

You can use hotline as a library inside the plugin system or on its own to use the low level abstractions and modules to create windowed applications with a graphics api backend. Here is a small example:

### Basic Application

```rust
// include prelude for convenience
use hotline_rs::prelude::*;

pub fn main() -> Result<(), hotline_rs::Error> { 
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

    // Run main loop
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

    /// must reset command buffers on shutdown otherwise they will leak COM objects (win32)
    cmd.reset(&swap_chain);

    Ok(());
}
```

### Gfx

The [gfx](https://docs.rs/hotline-rs/latest/hotline_rs/gfx/index.html) module provides a modern graphics API loosely following Direct3D12 with Vulkan and Metal compatibility in mind. If you are familiar with those API's it should be straight forward, but here is a quick example of how to do some render commands:

```rust
// create a buffer
let info = gfx::BufferInfo {
    usage: gfx::BufferUsage::Vertex,
    cpu_access: gfx::CpuAccessFlags::NONE,
    format: gfx::Format::Unknown,
    stride: std::mem::size_of::<Vertex>(),
    num_elements: 3,
};

let vertex_buffer = device.create_buffer(&info, Some(gfx::as_u8_slice(&vertices)))?;

// create shaders and a pipeline
let vsc_filepath = hotline_rs::get_data_path("data/shaders/triangle/vs_main.vsc");
let psc_filepath = hotline_rs::get_data_path("data/shaders/triangle/ps_main.psc");

let vsc_data = fs::read(vsc_filepath)?;
let psc_data = fs::read(psc_filepath)?;

let vsc_info = gfx::ShaderInfo {
    shader_type: gfx::ShaderType::Vertex,
    compile_info: None
};
let vs = device.create_shader(&vsc_info, &vsc_data)?;

let psc_info = gfx::ShaderInfo {
    shader_type: gfx::ShaderType::Vertex,
    compile_info: None
};
let fs = device.create_shader(&psc_info, &psc_data)?;

// create the pipeline itself with the vs and fs
let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
    vs: Some(&vs),
    fs: Some(&fs),
    input_layout: vec![
        gfx::InputElementInfo {
            semantic: String::from("POSITION"),
            index: 0,
            format: gfx::Format::RGB32f,
            input_slot: 0,
            aligned_byte_offset: 0,
            input_slot_class: gfx::InputSlotClass::PerVertex,
            step_rate: 0,
        },
        gfx::InputElementInfo {
            semantic: String::from("COLOR"),
            index: 0,
            format: gfx::Format::RGBA32f,
            input_slot: 0,
            aligned_byte_offset: 12,
            input_slot_class: gfx::InputSlotClass::PerVertex,
            step_rate: 0,
        },
    ],
    descriptor_layout: gfx::DescriptorLayout::default(),
    raster_info: gfx::RasterInfo::default(),
    depth_stencil_info: gfx::DepthStencilInfo::default(),
    blend_info: gfx::BlendInfo {
        alpha_to_coverage_enabled: false,
        independent_blend_enabled: false,
        render_target: vec![gfx::RenderTargetBlendInfo::default()],
    },
    topology: gfx::Topology::TriangleList,
    patch_index: 0,
    pass: swap_chain.get_backbuffer_pass(),
})?;

// build command buffer and make draw calls
cmd.reset(&swap_chain);

// manual transition handling
cmd.transition_barrier(&gfx::TransitionBarrier {
    texture: Some(swap_chain.get_backbuffer_texture()),
    buffer: None,
    state_before: gfx::ResourceState::Present,
    state_after: gfx::ResourceState::RenderTarget,
});

// render pass approach is used, swap chain automatically creates some for us
cmd.begin_render_pass(swap_chain.get_backbuffer_pass_mut());
cmd.set_viewport(&viewport);
cmd.set_scissor_rect(&scissor);

// set state for the draw
cmd.set_render_pipeline(&pso);
cmd.set_vertex_buffer(&vertex_buffer, 0);
cmd.draw_instanced(3, 1, 0, 0);
cmd.end_render_pass();

// manually transition
cmd.transition_barrier(&gfx::TransitionBarrier {
    texture: Some(swap_chain.get_backbuffer_texture()),
    buffer: None,
    state_before: gfx::ResourceState::RenderTarget,
    state_after: gfx::ResourceState::Present,
});

// execute command buffer
cmd.close()?;
device.execute(&cmd);

// swap for the next frame
swap_chain.swap(&device);
```

### Pmfx

Pmfx builds on top of the `gfx` module to make render configuration more ergonomic, data driven and quicker to develop with. You can use the [pmfx](https://docs.rs/hotline-rs/latest/hotline_rs/pmfx/index.html) module and `pmfx` data to configure render pipelines in a data driven way. The [pmfx-shader](https://github.com/polymonster/pmfx-shader) repository has more detailed information and is currently undergoing changes and improvements but it now supports a decent range of features.

You can supply [jsn](https://github.com/polymonster/jsnr) config files to specify render pipelines, textures (render targets), views (render pass with cameras) and render graphs. Useful defaults are supplied for all fields and combined with jsn inheritance it can aid creating many different render strategies with minimal repetition.

```pmfx
textures: {
    main_colour: {
        ratio: {
            window: "main_window",
            scale: 1.0
        }
        format: "RGBA8n"
        usage: ["ShaderResource", "RenderTarget"]
        samples: 8
    }
    main_depth(main_colour): {
        format: "D24nS8u"
        usage: ["ShaderResource", "DepthStencil"]
        samples: 8
    }
}
views: {
    main_view: {
        render_target: [
            "main_colour"
        ]
        clear_colour: [0.45, 0.55, 0.60, 1.0]
        depth_stencil: [
            "main_depth"
        ]
        clear_depth: 1.0
        viewport: [0.0, 0.0, 1.0, 1.0, 0.0, 1.0]
        camera: "main_camera"
    }
    main_view_no_clear(main_view): {
        clear_colour: null
        clear_depth: null
    }
}
pipelines: {
    mesh_debug: {
        vs: vs_mesh
        ps: ps_checkerboard
        push_constants: [
            "view_push_constants"
            "draw_push_constants"
        ]
        depth_stencil_state: depth_test_less
        raster_state: cull_back
        topology: "TriangleList"
    }
}
render_graphs: {
    mesh_debug: {
        grid: {
            view: "main_view"
            pipelines: ["imdraw_3d"]
            function: "render_grid"
        }
        meshes: {
            view: "main_view_no_clear"
            pipelines: ["mesh_debug"]
            function: "render_meshes"
            depends_on: ["grid"]
        }
        wireframe: {
            view: "main_view_no_clear"
            pipelines: ["wireframe_overlay"]
            function: "render_meshes"
            depends_on: ["meshes", "grid"]
        }
    }
}
```

When pmfx is built, shader source is generated along with an [info file](https://github.com/polymonster/pmfx-shader/blob/master/examples/outputs/v2_info.json) which contains useful reflection information to be used at runtime. Based on shader inputs and usage, descriptor layouts can automatically be generated.

## Examples

There are a few standalone examples of how to use the lower level components of hotline (`gfx, app, av`). You can build and run these as follows:

```text
// build examples
cargo build --examples

// make sure to build data
.\hotline-data\pmbuild.cmd win32-data

// run a single sample
cargo run --example triangle
```

## Design Goals

- An easy to use cross platform graphics/compute/os api for rapid development.
- Hot reloadable, live coding environment (shaders, render graphs, code).
- Concise low level graphics api... think somewhere in-between Metal and Direct3D12.
- High level data driven graphics api (`pmfx`) for ease of use and speed.
- A focus on modern rendering examples (gpu-driven, multi-threaded, bindless, ray-tracing).
- Flexibility to easily create and use different rendering strategies (deferred vs forward, gpu-driven vs cpu driven, etc).
- Hardware accelerated video decoding.
- Fibre based, multi-threaded, easily scalable to utilise available cpu and gpu.
- Data-driven and configurable.
- Plugin based and extensible...

## Roadmap

### In Progress

- Samples and Demos
- Debug / Primitive Rendering API
- High level graphics api (render graphs, data driven, Uber shaders)  
- ~~Multi-threading support (async command buffer generation and job dispatches)~~
- ~~Hot reloading~~
- ~~API (gfx::, os::) / Backend (d3d12::, win32::)~~
- ~~API (av::) / Windows Media Foundation (HW Video / Audio Decoding)~~
- ~~Imgui support w/ Viewports~~

### Future Plans

- Linux
- Vulkan
- macOS
- Metal
- AV Foundation
- WASM
- WebGPU

## Contributing

Contributions of all kinds are welcome, you can make a fork and send a PR if you want to submit small fixes or improvements. Anyone interested in being more involved in development I am happy to take on people to help with the project of all experience levels, especially people with more experience in Rust. You can contact me if interested via [Twitter](twitter.com/polymonster) or [Discord](https://discord.com/invite/3yjXwJ8wJC).  
