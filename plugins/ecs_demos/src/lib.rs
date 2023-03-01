// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::SystemDescriptor;

mod primitives;
mod test;
mod dev;
mod draw;

use crate::draw::*;
use crate::primitives::*;
use crate::test::*;

/// Register demo names
#[no_mangle]
pub fn get_demos_ecs_demos() -> Vec<String> {
    demos![
        "primitives",
        "draw_indexed",
        "draw_indexed_push_constants",

        // tests
        "test_missing_demo",
        "test_missing_systems",
        "test_missing_render_graph",
        "test_missing_view",
        "test_missing_pipeline",
        "test_failing_pipeline",
        "test_missing_camera"
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

        // test functions
        "render_missing_camera" => render_func![render_missing_camera, view_name],
        "render_missing_pipeline" => render_func![render_missing_pipeline, view_name],
        _ => std::hint::black_box(None)
    }
}