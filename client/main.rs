use hotline_rs::*;
use hotline_rs::prelude::*;

#[cfg(target_os = "macos")]
fn platform_dpi_aware() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn platform_dpi_aware() -> bool {
    true
}

/// Values passed on the command line as `--<name> <value>`, the arg can be passed multiple times
fn arg_values(name: &str) -> Vec<String> {
    let flag = format!("--{}", name);
    let mut values = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            if let Some(value) = args.next() {
                values.push(value);
            }
            else {
                println!("hotline_rs::client:: {} expects a value", flag);
            }
        }
    }
    values
}

/// Plugins passed on the command line with `--plugin <path>`, where path is the plugin directory or its Cargo.toml.
/// Returns (name, parent directory) pairs for `Client::add_plugin_lib`, relative paths are from the working directory
fn plugin_args() -> Vec<(String, String)> {
    let mut plugins = Vec::new();
    for path in arg_values("plugin") {
        let path = match std::fs::canonicalize(&path) {
            Ok(path) => path,
            Err(e) => {
                println!("hotline_rs::client:: --plugin {}: {}", path, e);
                continue;
            }
        };
        let dir = if path.is_file() { path.parent().unwrap().to_path_buf() } else { path };
        if let (Some(name), Some(parent)) = (dir.file_name(), dir.parent()) {
            plugins.push((name.to_string_lossy().to_string(), parent.to_string_lossy().to_string()));
        }
    }
    plugins
}

/// Set the active demo in the "ecs" plugin data, restoring the demo's saved camera like selecting it in the ecs ui
fn set_active_demo(ctx: &mut Client<gfx_platform::Device, os_platform::App>, demo: &str) {
    let plugin_data = ctx.user_config.plugin_data.get_or_insert_with(Default::default);
    let ecs = plugin_data.entry("ecs".to_string()).or_insert_with(|| serde_json::json!({}));
    if let Some(ecs) = ecs.as_object_mut() {
        if ecs.get("active_demo").and_then(|d| d.as_str()) != Some(demo) {
            let camera = ["demo_cameras", "default_cameras"].iter()
                .find_map(|cameras| ecs.get(*cameras).and_then(|c| c.get(demo)).cloned())
                .unwrap_or(serde_json::Value::Null);
            ecs.insert("main_camera".to_string(), camera);
            ecs.insert("active_demo".to_string(), serde_json::Value::String(demo.to_string()));
        }
    }
}

fn main() -> Result<(), hotline_rs::Error> {

    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        eprintln!("PANIC: {info}\n{bt}");
        std::process::abort();
    }));

    // plugins passed on the command line replace the previous session's, each ecs plugin hosts its own ecs and they
    // would conflict if loaded together
    let plugins = plugin_args();

    // create client
    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        dpi_aware: platform_dpi_aware(),
        load_session_plugins: plugins.is_empty(),
        fullscreen: std::env::args().any(|arg| arg == "--fullscreen"),
        ..Default::default()
    })?;

    for (name, parent) in plugins {
        ctx.add_plugin_lib(&name, &parent);
    }

    // boot straight into a demo with `--demo <name>`, the ecs reads its active demo from its plugin data on setup
    if let Some(demo) = arg_values("demo").pop() {
        set_active_demo(&mut ctx, &demo);
    }

    // run
    if let Err(e) = ctx.run() {
        println!("error: {}", e.msg);
    };

    Ok(())
}
