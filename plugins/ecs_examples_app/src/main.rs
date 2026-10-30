/// ecs_examples as a standalone app: the plugin crate is linked statically instead of loaded and hot reloaded from
/// its dll. The client finds its functions by name in the link time `PLUGIN_SYMBOLS` table, see `PluginLib`.
/// usage: cargo run -p ecs_examples_app -- --demo <name>
use hotline_rs::prelude::*;

// link the plugin, its systems and demos register themselves into `PLUGIN_SYMBOLS`
use ecs_examples as _;

fn main() -> Result<(), hotline_rs::Error> {
    let mut ctx : Client<gfx_platform::Device, os_platform::App> = Client::create(HotlineInfo {
        name: "ecs_examples_app".to_string(),
        load_session_plugins: false,
        // its own session, so it doesn't change the dev client's active demo and camera
        user_config_path: Some("ecs_examples_app_user_config.json".to_string()),
        ..Default::default()
    })?;

    ctx.add_plugin_static("ecs_examples");

    if let Some(demo) = client::arg_values("demo").pop() {
        ctx.set_active_demo(&demo);
    }

    ctx.run()
}
