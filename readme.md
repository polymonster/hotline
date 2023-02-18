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

## Using the Client

You can run the binary `client` which allows code to be reloaded through `plugins`. There are some [plugins](https://github.com/polymonster/hotline/tree/master/plugins) already provided with the repository.

```text
// build the client and data
cargo build

// then build plugins
cargo build --manifest-path plugins/Cargo.toml

// run the client
cargo run client
```

Any code changes made to the plugin libs will cause a rebuild and reload to happen with the client still running. You can also edit the [shaders](https://github.com/polymonster/hotline/tree/master/src/shaders) where `hlsl` files make up the shader code and `pmfx` files allow you to sepcify pipeline state objects in config files. Any changes detected to `pmfx` shaders will be rebuilt and all modified pipelines or views will be rebuilt.

### Adding Plugins

Plugins are loaded by passing a directory to `hotline_rs::client::Client::add_plugin_lib()` which contains a `Cargo.toml` and is a dynamic library. They can be opened in the client using the `File > Open` from the main menu bar by selecting the `Cargo.toml`.

The basic `Cargo.toml` setup looks like this:

```toml
[package]
name = "ecs_basic"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["rlib", "dylib"]

[dependencies]
hotline-rs = { path = "../.." }
ecs_base = { path = "../ecs_base" }
maths-rs = "0.1.4"
bevy_ecs = "0.9.1"
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

There is a core `ecs` plugin which builds ontop of `bevy_ecs`. It allows you to supply you own systems and build schedules dynamically. It is possible to load and find new ecs systems in different dynamic libraries, you can register and instantiate `demos` which are collections of setup, update and render systems.

#### Initialisation Functions

You can setup a new ecs demo by providing an initialisation function named after the demo this returns a `SheduleInfo` for which systems to run:

```rust
/// Supply an in intialise function which returns a `SheduleInfo` for a demo
#[no_mangle]
pub fn cube(client: &mut Client<gfx_platform::Device, os_platform::App>) -> SheduleInfo {
    // pmfx
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/basic").as_str()).unwrap();
    client.pmfx.create_render_graph(&mut client.device, "checkerboard").unwrap();

    SheduleInfo {
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render: client.pmfx.get_render_function_names("checkerboard"),
        setup: vec!["setup_cube".to_string()]
    }
}
```

#### Setup Systems

You can supply setup systems to add entities into a scene, when a dynamic code reload happens the world will be cleared the setup systems will be re-executed. This allows changes to setup systems to appear in the live `client`. You can add multiple setup systems and the will be executed concurrently.

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

You can specify render graphs in `pmfx` which setup `views` which get dispatched into render functions. All rneder systems run concurrently on the CPU, the command buffers they generate are executed in an order specified by the `pmfx` render graph and it's dependencies.

```rust
#[no_mangle]
pub fn render_checkerboard_basic(
    pmfx: bevy_ecs::prelude::Res<PmfxRes>,
    view_name: String,
    view_proj_query: bevy_ecs::prelude::Query<&ViewProjectionMatrix>,
    mesh_draw_query: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) {
        
    // unpack
    let pmfx = &pmfx.0;
    let arc_view = pmfx.get_view(&view_name).unwrap();
    let view = arc_view.lock().unwrap();
    let fmt = view.pass.get_format_hash();

    let checkerboard = pmfx.get_render_pipeline_for_format("checkerboard_mesh", fmt);
    if checkerboard.is_none() {
        return;
    }

    // setup pass
    view.cmd_buf.begin_render_pass(&view.pass);
    view.cmd_buf.set_viewport(&view.viewport);
    view.cmd_buf.set_scissor_rect(&view.scissor_rect);

    view.cmd_buf.set_render_pipeline(&checkerboard.unwrap());

    for view_proj in &view_proj_query {
        view.cmd_buf.push_constants(0, 16, 0, &view_proj.0);
        for (world_matrix, mesh) in &mesh_draw_query {
            // draw
            view.cmd_buf.push_constants(1, 16, 0, &world_matrix.0);
            view.cmd_buf.set_index_buffer(&mesh.0.ib);
            view.cmd_buf.set_vertex_buffer(&mesh.0.vb, 0);
            view.cmd_buf.draw_indexed_instanced(mesh.0.num_indices, 1, 0, 0, 0);
        }
    }

    // end / transition / execute
    view.cmd_buf.end_render_pass();
}
```

#### Update Systems

You can also supply your own update systems to animate and move your entities, these too are all executed concurrently.

```rust
fn update_cameras(
    app: Res<AppRes>, 
    main_window: Res<MainWindowRes>, 
    mut query: Query<(&mut Position, &mut Rotation, &mut ViewProjectionMatrix), With<Camera>>) {    
    let app = &app.0;
    for (mut position, mut rotation, mut view_proj) in &mut query {
    
    // logic
    }
}
```

#### Registering Systems

Systems can be imported dynamically from different plugins, in order to do so they need to be hooked into a function which can be located dynamically by the `ecs` plugin. In time I hope to be able to remove this baggage and be able to `#[derive()]` them.  

You can implement a function called `get_demos_<lib_name>` which returns a list of available demos inside a `plugin` and `get_system_<lib_name>` to return `bevy_ecs::SystemDescriptor` of systems which can then be looked up by name, the ecs plugin will search for systems by name within all other loaded plugins, so you can build and share functionality.

```rust
#[no_mangle]
pub fn get_demos_ecs_basic() -> Vec<String> {
    vec![
        "primitives".to_string(),
        "billboard".to_string(),
        "cube".to_string(),
        "multiple".to_string(),
        "heightmap".to_string(),
    ]
}

#[no_mangle]
pub fn get_system_ecs_basic(name: String) -> Option<SystemDescriptor> {
    match name.as_str() {
        // setup functions
        "setup_cube" => ecs_base::system_func![crate::primitives::setup_cube],
        // ..
        "render_checkerboard_basic" => ecs_base::view_func![crate::primitives::render_checkerboard_basic, "render_checkerboard_basic"],
        _ => std::hint::black_box(None)
    }
}
```

## Using as a library

You can use hotline as a library inside the plugin system or on it's own to use the low level abstractions and modules to create windowed applications with a graphics api backend. Here is a small example:

### Basic Application

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

### In Progress

- Debug / Primitve Rendering API
- High level graphics api (render graphs, data driven, Uber shaders)
- Multi-threading support (async command buffer generation and job dispatches)
- Samples and Demos
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

Contributions of all kinds are welcome, you can make a fork and send a PR if you want to submit small fixes or improvements. Anyone interseted in being more involved in development I am happy to take on people to help with project of all experience levels, especially people with more experience in Rust. You can contact me if interested via [Twitter](twitter.com/polymonster) or [Discord](https://discord.com/invite/3yjXwJ8wJC).  
