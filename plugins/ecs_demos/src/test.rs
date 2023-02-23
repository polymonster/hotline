// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::client::Client;
use hotline_rs::gfx_platform;
use hotline_rs::os_platform;

use hotline_rs::ecs_base::ScheduleInfo;

/// Tests missing setup and updates are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_systems(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {   
    ScheduleInfo {
        setup: vec![
            "missing".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string(),
            "missing".to_string()
        ],
        render_graph: "mesh_debug".to_string()
    }
}

/// Tests missing render graphs are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_render_graph(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    ScheduleInfo {
        setup: vec![
            "setup_cube".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing".to_string()
    }
}