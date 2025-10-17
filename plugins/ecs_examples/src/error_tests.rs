// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "macos")]

use crate::prelude::*;

/// Tests missing setup and updates are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_systems(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {   
    ScheduleInfo {
        setup: systems![
            "missing"
        ],
        update: systems![
            "missing"
        ],
        render_graph: "mesh_debug"
    }
}

/// Tests missing render graphs are handled gracefully and notified to the user
#[no_mangle]
pub fn test_missing_render_graph(_: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    ScheduleInfo {
        setup: systems![
            "setup_cube"
        ],
        render_graph: "missing",
        ..Default::default()
    }
}

/// Tests missing view specified in the render graph
#[no_mangle]
pub fn test_missing_view(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_view",
        ..Default::default()
    }
}

/// Tests case where render graph fails, in this case it is missing a pipeline, but the pipeline can also fail to build depending on the src data
#[no_mangle]
pub fn test_failing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_pipeline",
        ..Default::default()
    }
}

/// Tests missing pipeline specified in the render graph
#[no_mangle]
pub fn test_missing_pipeline(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_pipeline",
        ..Default::default()
    }
}

/// Tests missing camera specified in the render graph
#[no_mangle]
pub fn test_missing_camera(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_camera",
        ..Default::default()
    }
}

/// Tests missing view_function (system) specified in the render graph
#[no_mangle]
pub fn test_missing_view_function(client: &mut Client<gfx_platform::Device, os_platform::App>) -> ScheduleInfo {    
    client.pmfx.load(&hotline_rs::get_data_path("shaders/ecs_examples").as_str()).unwrap();
    ScheduleInfo {
        setup: systems![
            "setup_primitives"
        ],
        render_graph: "missing_function",
        ..Default::default()
    }
}

#[no_mangle]
#[export_render_fn]
pub fn render_missing_camera(
    pmfx: &Res<PmfxRes>) -> Result<(), hotline_rs::Error> {
    pmfx.get_camera_constants("missing")?;
    Ok(())
}

#[no_mangle]
#[export_render_fn]
pub fn render_missing_pipeline(
    pmfx: &Res<PmfxRes>,
    view: &pmfx::View<gfx_platform::Device>) -> Result<(), hotline_rs::Error> {
    let fmt = view.pass.get_format_hash();
    pmfx.get_render_pipeline_for_format("missing", fmt)?;
    Ok(())
}