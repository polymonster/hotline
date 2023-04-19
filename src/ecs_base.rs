// currently windows only because here we need a concrete gfx and os implementation
#![cfg(target_os = "windows")]

use crate::{client, pmfx, imdraw, gfx_platform, os_platform};

use bevy_ecs::prelude::*;
use maths_rs::prelude::*;

use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::ops::DerefMut;

/// Schedule info can be filled out and passed to the `ecs` plugin to build a schedulre for a running demo
pub struct ScheduleInfo {
    /// List of setup functions by their name, the function name must be registered in a `get_system_function` 
    /// all setup systems will run concurrently
    pub setup: Vec<String>,
    /// List of update functions by their name, the function name must be registered in a `get_system_function` 
    /// all update systems will run concurrently
    pub update: Vec<String>,
    /// Name of the render graph to load, buld and make active from pmfx
    pub render_graph: &'static str
}

/// Empty schedule info
impl Default for ScheduleInfo {
    fn default() -> ScheduleInfo {
        ScheduleInfo {
            setup: Vec::new(),
            update: Vec::new(),
            render_graph: "",
        }
    }
}

/// Serialisable camera info
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct CameraInfo {
    pub camera_type: CameraType,
    pub pos: (f32, f32, f32),
    pub rot: (f32, f32, f32),
    pub focus: (f32, f32, f32),
    pub zoom: f32,
    pub aspect: f32,
    pub fov: f32,
}

/// Sensible default values for a 3D perspective camera looking downward in the y-axis to an xz-plane
impl Default for CameraInfo {
    fn default() -> CameraInfo {
        CameraInfo {
            camera_type: CameraType::Fly,
            zoom: 500.0,
            pos: (0.0, 150.0, 150.0),
            rot: (-45.0, 0.0, 0.0),
            focus: (0.0, 0.0, 0.0),
            aspect: 16.0/9.0,
            fov: 60.0
        }
    }
}

/// This macro allows you to create a newtype which will automatically deref and deref_mut
/// you can use it to create resources or compnents and avoid having to use .0 to access the inner data
#[macro_export]
macro_rules! hotline_ecs {
    ($derive:ty, $name:ident, $inner:ty) => {
        #[derive($derive)]
        pub struct $name(pub $inner);
        impl Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &$inner {
                &self.0
            }
        }
        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut $inner {
                &mut self.0
            }
        }
    }
}

//
// Resources
//

hotline_ecs!(Resource, TimeRes, client::Time);
hotline_ecs!(Resource, PmfxRes, pmfx::Pmfx<gfx_platform::Device>);
hotline_ecs!(Resource, DeviceRes, gfx_platform::Device);
hotline_ecs!(Resource, AppRes, os_platform::App);
hotline_ecs!(Resource, MainWindowRes,os_platform::Window);
hotline_ecs!(Resource, ImDrawRes, imdraw::ImDraw<gfx_platform::Device>);
hotline_ecs!(Resource, UserConfigRes, client::UserConfig);

//
// Components
//

hotline_ecs!(Component, Name, String);
hotline_ecs!(Component, Parent, Entity);
hotline_ecs!(Component, Velocity, Vec3f);
hotline_ecs!(Component, Position, Vec3f);
hotline_ecs!(Component, Rotation, Quatf);
hotline_ecs!(Component, Scale, Vec3f);
hotline_ecs!(Component, Colour, Vec4f);
hotline_ecs!(Component, LocalMatrix, Mat34f);
hotline_ecs!(Component, WorldMatrix, Mat34f);
hotline_ecs!(Component, ViewProjectionMatrix, Mat4f);
hotline_ecs!(Component, MeshComponent, pmfx::Mesh<gfx_platform::Device>);
hotline_ecs!(Component, PipelineComponent, String);
hotline_ecs!(Component, TextureComponent, gfx_platform::Texture);
hotline_ecs!(Component, BufferComponent, gfx_platform::Buffer);
hotline_ecs!(Component, TextureInstance, u32);
hotline_ecs!(Component, TimeComponent, f32);
hotline_ecs!(Component, CommandSignatureComponent, gfx_platform::CommandSignature);

#[derive(Component)]
pub struct InstanceBuffer {
    pub heap: Option<gfx_platform::Heap>,
    pub buffer: gfx_platform::Buffer,
    pub instance_count: u32
}

#[derive(Bundle)]
pub struct InstanceBatch {
    pub mesh: MeshComponent,
    pub pipeline: PipelineComponent,
    pub instance_buffer: InstanceBuffer
}

#[derive(Bundle)]
pub struct Instance {
    pub pos: Position,
    pub rot: Rotation,
    pub scale: Scale,
    pub world_matrix: WorldMatrix,
    pub parent: Parent
}

#[derive(Component)]
pub struct InstanceIds {
    pub entity_id: u32,
    pub material_id: u32
}

#[derive(Component)]
pub struct MaterialResources {
    pub albedo: gfx_platform::Texture,
    pub normal: gfx_platform::Texture,
    pub roughness: gfx_platform::Texture
}

#[derive(Component)]
pub struct Camera {
    pub rot: Vec3f,
    pub focus: Vec3f,
    pub zoom: f32,
    pub camera_type: CameraType
}

#[derive(Component)]
pub struct AnimatedTexture {
    pub frame : u32,
    pub frame_count: u32
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
pub struct Billboard;

#[derive(Component)]
pub struct CylindricalBillboard;

#[derive(Debug, Clone, Copy)]
pub enum LightType {
    Point,
    Spot,
    Directional
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CameraType {
    None,
    Fly,
    Orbit,
}

#[derive(Component)]
pub struct LightComponent {
    pub light_type: LightType,
    pub direction: Vec3f,
    pub cutoff: f32,
    pub falloff: f32,
    pub radius: f32
}

impl Default for LightComponent {
    fn default() -> Self {
        Self {
            light_type: LightType::Directional,
            direction: vec3f(0.0, -1.0, 0.0),
            cutoff: f32::pi() / 8.0,
            falloff: 0.5,
            radius: 64.0
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
#[system_set(base)]
pub enum SystemSets {
    Setup,
    Update,
    Batch,
    Render,
}

#[macro_export]
macro_rules! system_func {
    ($func:expr) => {
        Some($func.into_config())
    }
}

#[macro_export]
macro_rules! render_func {
    ($func:expr, $view:expr, $query:ty) => {
        Some(render_func_closure![$func, $view, $query].into_config())
    }
}

#[macro_export]
macro_rules! compute_func {
    ($pass:expr) => {
        Some(compute_func_closure![$pass].into_config())
    }
}

#[macro_export]
macro_rules! compute_func_query {
    ($func:expr, $pass:expr, $query:ty) => {
        Some(compute_func_query_closure![$func, $pass, $query].into_config())
    }
}

/// This macro can be used to export a system render function for bevy ecs. You can pass a compatible 
/// system function with a `view` name which can be looked up when the function is called
/// so that a single render function can have different views
#[macro_export]
macro_rules! render_func_closure {
    ($func:expr, $view_name:expr, $query:ty) => {
        move |
            pmfx: Res<PmfxRes>,
            q: $query | {
                let view = pmfx.get_view(&$view_name);
                let err = match view {
                    Ok(v) => { 
                        let mut view = v.lock().unwrap();
                        
                        let col = view.colour_hash;
                        view.cmd_buf.begin_event(col, &$view_name);
                        view.cmd_buf.begin_render_pass(&view.pass);
                        view.cmd_buf.set_viewport(&view.viewport);
                        view.cmd_buf.set_scissor_rect(&view.scissor_rect);

                        let result = $func(
                            &pmfx,
                            &view,
                            q
                        );

                        view.cmd_buf.end_render_pass();
                        view.cmd_buf.end_event();
                        result
                    }
                    Err(v) => {
                        Err(hotline_rs::Error {
                            msg: v.msg
                        })
                    }
                };

                // record errors
                if let Err(err) = err {
                    pmfx.log_error(&$view_name, &err.msg);
                }
        }
    }
}

/// This macro can be used to export a system render function for bevy ecs. You can pass a compatible 
/// system function with a `view` name which can be looked up when the function is called
/// so that a single render function can have different views
#[macro_export]
macro_rules! compute_func_closure {
    ($pass_name:expr) => {
        move | pmfx: Res<PmfxRes> | {
                let pass = pmfx.get_compute_pass(&$pass_name);
                let err = match pass {
                    Ok(p) => {
                        let mut pass = p.lock().unwrap();
                        let pipeline = pmfx.get_compute_pipeline(&pass.pass_pipline).unwrap();

                        pass.cmd_buf.begin_event(0xffffff, &$pass_name);
                        pass.cmd_buf.set_compute_pipeline(&pipeline);

                        for i in 0..pass.srv_indices.len() {
                            pass.cmd_buf.push_compute_constants(0, 1, i as u32, gfx::as_u8_slice(&pass.srv_indices[i]));
                        }
                        
                        pass.cmd_buf.set_compute_heap(1, &pmfx.shader_heap);

                        pass.cmd_buf.dispatch(
                            pass.group_count,
                            pass.thread_count
                        );
                        pass.cmd_buf.end_event();

                        Ok(())
                    }
                    Err(p) => {
                        Err(hotline_rs::Error {
                            msg: p.msg
                        })
                    }
                };

                // record errors
                if let Err(err) = err {
                    pmfx.log_error(&$pass_name, &err.msg);
                }
            }
    }
}

#[macro_export]
macro_rules! compute_func_query_closure {
    ($func:expr, $pass_name:expr, $query:ty) => {
        move | 
            pmfx: Res<PmfxRes>,
            q: $query  | {
                let pass = pmfx.get_compute_pass(&$pass_name);
                let err = match pass {
                    Ok(p) => {
                        let mut pass = p.lock().unwrap();
                        pass.cmd_buf.begin_event(0xffffff, &$pass_name);

                        let result = $func(
                            &pmfx,
                            &mut pass,
                            q
                        );

                        pass.cmd_buf.end_event();

                        Ok(())
                    }
                    Err(p) => {
                        Err(hotline_rs::Error {
                            msg: p.msg
                        })
                    }
                };

                // record errors
                if let Err(err) = err {
                    pmfx.log_error(&$pass_name, &err.msg);
                }
            }
    }
}

/// You can use this macro to make the exporting of demo names for ecs plugins mor ergonomic, 
/// it will make a `Vec<String>` from a list of `&str`.
/// demos![
///     "primitives,
///     "draw_indexed"
///     "draw_indexed_instance"
/// ]
#[macro_export]
macro_rules! demos {
    ($($entry:expr),*) => {   
        vec![
            $($entry.to_string(),)+
        ]
    }
}

/// You can use this macro to make the exporting of systems names for ecs plugins more ergonomic, 
/// it will make a `Vec<String>` from a list of `&str`.
/// systems![
///     "update_camera,
///     "update_config"
/// ]
#[macro_export]
macro_rules! systems {
    ($($entry:expr),*) => {   
        vec![
            $($entry.to_string(),)+
        ]
    }
}