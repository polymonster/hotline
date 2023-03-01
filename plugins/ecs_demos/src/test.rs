// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use hotline_rs::prelude::*;

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

/// Tests missing view specified in the render graph
#[no_mangle]
pub fn test_missing_view(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: vec![
            "setup_primitives".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing_view".to_string()
    }
}

/// Tests case where render graph fails, in this case it is missing a pipeline, but the pipeline can also fail to build depending on the src data
#[no_mangle]
pub fn test_failing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: vec![
            "setup_primitives".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing_pipeline".to_string()
    }
}

/// Tests missing pipeline specified in the render graph
#[no_mangle]
pub fn test_missing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: vec![
            "setup_primitives".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing_pipeline".to_string()
    }
}

/// Tests missing camera specified in the render graph
#[no_mangle]
pub fn test_missing_camera(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: vec![
            "setup_primitives".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing_camera".to_string()
    }
}

/// Tests missing view_function (system) specified in the render graph
#[no_mangle]
pub fn test_missing_view_function(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("data/shaders/tests").as_str()).unwrap();
    ScheduleInfo {
        setup: vec![
            "setup_primitives".to_string()
        ],
        update: vec![
            "update_cameras".to_string(),
            "update_main_camera_config".to_string()
        ],
        render_graph: "missing_function".to_string()
    }
}

#[no_mangle]
pub fn render_missing_camera(
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    _: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx.0;
    pmfx.get_camera_constants("missing")?;

    Ok(())
}

#[no_mangle]
pub fn render_missing_pipeline(
    pmfx: &bevy_ecs::prelude::Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>,
    _: bevy_ecs::prelude::Query<(&WorldMatrix, &MeshComponent)>) -> Result<(), hotline_rs::Error> {
        
    let pmfx = &pmfx.0;
    let fmt = view.pass.get_format_hash();
    pmfx.get_render_pipeline_for_format("missing", fmt)?;

    Ok(())
}