/// Operating system module.
pub mod os;
use gfx::RenderPass;
use os::Window;

/// Graphics and compute module.
pub mod gfx;
use gfx::SwapChain;
use gfx::CmdBuf;
use gfx::Texture;

/// Hardware accelerated audio and video decoding
pub mod av;

/// Image reading/writing module support for (png, jpg, bmp, tiff, dds)
pub mod image;

/// Imgui rendering and platform implementation using imgui_sys
pub mod imgui;

/// Immediate mode primitive rendering API
pub mod imdraw;

/// High level graphics
pub mod pmfx;

/// Base ecs components, systems and resources
pub mod ecs;

/// Geometry primitives
pub mod primitives;

/// Use bitmask for flags
#[macro_use]
extern crate bitflags;

use serde::{Deserialize, Serialize};

/// Generic errors for modules to define their own
pub struct Error {
    pub msg: String,
}

/// Generic debug for errors
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

// conversion for windows-rs win32 errors
#[cfg(target_os = "windows")]
impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Error {
        Error {
            msg: err.message().to_string_lossy(),
        }
    }
}

// std errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error {
            msg: err.to_string()
        }
    }
}

/// information to create a hotline context which will create an app, window, device
pub struct HotlineInfo {
    /// name for the app and window title
    pub name: String,
    /// window rect {pos_x pos_y, width, height}
    pub window_rect: os::Rect<i32>,
    /// signify if the app is DPI aware or not
    pub dpi_aware: bool,
    /// clear colour of the default swap chain
    pub clear_colour: Option<gfx::ClearColour>,
    /// optional name of gpu adaptor, use None for the default / primary device
    pub adapter_name: Option<String>,
    /// number of buffers in the swap chain (2 for double buffered, 3 for tripple etc)
    pub num_buffers: u32,
    /// size of the default device heap for shader resources (textures, buffers, etc)
    pub shader_heap_size: usize, 
    /// size of the default device heap for render targets
    pub render_target_heap_size: usize,
    /// size of the default device heap for depth stencil targets
    pub depth_stencil_heap_size: usize
}

impl Default for HotlineInfo {
    fn default() -> Self {
        HotlineInfo {
            name: "hotline".to_string(),
            window_rect: os::Rect {
                x: 100,
                y: 100,
                width: 1280,
                height: 720
            },
            dpi_aware: true,
            clear_colour: Some(gfx::ClearColour {
                r: 0.45,
                g: 0.55,
                b: 0.60,
                a: 1.00,
            }),
            num_buffers: 2,
            adapter_name: None,
            shader_heap_size: 1024,
            render_target_heap_size: 128,
            depth_stencil_heap_size: 64
        }
    }
}

pub struct Context<D: gfx::Device, A: os::App> {
    pub app: A,
    pub device: D,
    pub main_window: A::Window,
    pub swap_chain: D::SwapChain,
    pub pmfx: pmfx::Pmfx<D>,
    pub cmd_buf: D::CmdBuf,
    pub imdraw: imdraw::ImDraw<D>,
    pub imgui: imgui::ImGui<D, A>,
    pub unit_quad_mesh: pmfx::Mesh<D>,
    pub user_config: UserConfig
}

#[derive(Serialize, Deserialize)]
pub struct UserConfig {
    // pos xy, size xy
    pub main_window_rect: os::Rect<i32>
}

impl<D, A> Context<D, A> where D: gfx::Device, A: os::App {
    /// Create a hotline context consisting of core resources
    pub fn create(info: HotlineInfo) -> Result<Self, Error> {
        
        // read user config or get defaults
        let user_config_path = get_data_path("user_config.json");
        let user_config = if std::path::Path::new(&user_config_path).exists() {
            let user_data = std::fs::read(user_config_path)?;
            serde_json::from_slice(&user_data).unwrap()
        }
        else {
            UserConfig {
                main_window_rect: info.window_rect
            }
        };
        
        // app
        let mut app = A::create(os::AppInfo {
            name: info.name.to_string(),
            num_buffers: info.num_buffers,
            dpi_aware: info.dpi_aware,
            window: false,
        });
    
        // device
        let mut device = D::create(&gfx::DeviceInfo {
            adapter_name: info.adapter_name,
            shader_heap_size: info.shader_heap_size,
            render_target_heap_size: info.render_target_heap_size,
            depth_stencil_heap_size: info.depth_stencil_heap_size,
        });
    
        // main window
        let main_window = app.create_window(os::WindowInfo {
            title: info.name.to_string(),
            rect: user_config.main_window_rect,
            style: os::WindowStyleFlags::NONE,
            parent_handle: None,
        });
    
        // swap chain
        let swap_chain_info = gfx::SwapChainInfo {
            num_buffers: info.num_buffers,
            format: gfx::Format::RGBA8n,
            clear_colour: info.clear_colour
        };
        let mut swap_chain = device.create_swap_chain::<A>(&swap_chain_info, &main_window)?;

        // imdraw
        let imdraw_info = imdraw::ImDrawInfo {
            initial_buffer_size_2d: 1024,
            initial_buffer_size_3d: 1024
        };
        let imdraw : imdraw::ImDraw<D> = imdraw::ImDraw::create(&imdraw_info).unwrap();

        // imgui    
        let mut imgui_info = imgui::ImGuiInfo::<D, A> {
            device: &mut device,
            swap_chain: &mut swap_chain,
            main_window: &main_window,
            fonts: vec![imgui::FontInfo {
                filepath: get_data_path("data/fonts/roboto_medium.ttf"),
                glyph_ranges: None
            }],
        };
        let imgui = imgui::ImGui::create(&mut imgui_info)?;

        // pmfx
        let mut pmfx = pmfx::Pmfx::<D>::create();
        pmfx.update_window::<A>(&mut device, &main_window, "main_window");

        // blit pmfx
        let unit_quad_mesh = primitives::create_unit_quad_mesh(&mut device);

        // default cmd buf
        let cmd_buf = device.create_cmd_buf(info.num_buffers);
    
        Ok(Context {
            app,
            device,
            main_window,
            swap_chain,
            cmd_buf,
            pmfx,
            imdraw,
            imgui,
            unit_quad_mesh,
            user_config
        })
    }

    /// Start a new frame syncronised to the swap chain
    pub fn new_frame(&mut self) {
        // hotline update
        // update window and swap chain for the new frame
        self.main_window.update(&mut self.app);
        self.swap_chain.update::<A>(&mut self.device, &self.main_window, &mut self.cmd_buf);
        self.pmfx.update_window::<A>(&mut self.device, &self.main_window, "main_window");

        // reset main command buffer
        self.cmd_buf.reset(&self.swap_chain);

        // start imgui new frame
        self.imgui.new_frame(&mut self.app, &mut self.main_window, &mut self.device);

        // start new pmfx frame
        self.pmfx.reload(&mut self.device);
        self.pmfx.new_frame(&self.swap_chain);

        // user config changes
        self.update_user_config_cache();
    }

    /// internal function to manage tracking user config values and changes, writes to disk if change are detected
    fn update_user_config_cache(&mut self) {
        // track any changes and write once
        let mut invalidated = false;
        
        // main window pos / size
        if self.user_config.main_window_rect != self.main_window.get_window_rect() {
            self.user_config.main_window_rect = self.main_window.get_window_rect();
            invalidated = true;
        }

        // write to file
        if invalidated {
            let user_config_file_text = serde_json::to_string(&self.user_config).unwrap();
            let user_config_path = get_data_path("user_config.json");

            std::fs::File::create(&user_config_path).unwrap();
            std::fs::write(&user_config_path, user_config_file_text).unwrap();
        }
    }

    /// Render and display a pmfx target 'blit_view_name' to the main window, draw imgui and swap buffers
    pub fn present(&mut self, blit_view_name: &str) {
        // execute pmfx command buffers first
        self.pmfx.execute(&mut self.device);

        // main pass
        self.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(self.swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::Present,
            state_after: gfx::ResourceState::RenderTarget,
        });

        // clear window
        self.cmd_buf.begin_render_pass(self.swap_chain.get_backbuffer_pass_mut());
        self.cmd_buf.end_render_pass();

        // blit
        self.cmd_buf.begin_render_pass(self.swap_chain.get_backbuffer_pass_no_clear());

        // get serv index of the pmfx target to blit to the window
        let srv = self.pmfx.get_texture(blit_view_name).unwrap().get_srv_index().unwrap();
        let fmt = self.swap_chain.get_backbuffer_pass_mut().get_format_hash();
       
        // blit to main window
        let vp_rect = self.main_window.get_viewport_rect();
        self.cmd_buf.begin_event(0xff0000ff, "Blit Pmfx");
        self.cmd_buf.set_viewport(&gfx::Viewport::from(vp_rect));
        self.cmd_buf.set_scissor_rect(&gfx::ScissorRect::from(vp_rect));
        self.cmd_buf.set_render_pipeline(self.pmfx.get_render_pipeline_for_format("imdraw_blit", fmt).unwrap());
        self.cmd_buf.push_constants(0, 2, 0, &[vp_rect.width as f32, vp_rect.height as f32]);
        self.cmd_buf.set_render_heap(1, self.device.get_shader_heap(), srv);
        self.cmd_buf.set_index_buffer(&self.unit_quad_mesh.ib);
        self.cmd_buf.set_vertex_buffer(&self.unit_quad_mesh.vb, 0);
        self.cmd_buf.draw_indexed_instanced(6, 1, 0, 0, 0);
        self.cmd_buf.end_event();

        // render imgui
        self.cmd_buf.begin_event(0xff0000ff, "ImGui");
        self.imgui.render(&mut self.app, &mut self.main_window, &mut self.device, &mut self.cmd_buf);
        self.cmd_buf.end_event();

        self.cmd_buf.end_render_pass();
        
        // transition to present
        self.cmd_buf.transition_barrier(&gfx::TransitionBarrier {
            texture: Some(self.swap_chain.get_backbuffer_texture()),
            buffer: None,
            state_before: gfx::ResourceState::RenderTarget,
            state_after: gfx::ResourceState::Present,
        });

        self.cmd_buf.close().unwrap();

        // execute the main window command buffer + swap
        self.device.execute(&self.cmd_buf);
        self.swap_chain.swap(&self.device);
        self.device.clean_up_resources(&self.swap_chain);
    }

    /// Wait for the last submitted frame to complete rendering to ensure safe shutdown once all in-flight resources
    /// are no longer needed
    pub fn wait_for_last_frame(&mut self) {
        self.swap_chain.wait_for_last_frame();
        self.cmd_buf.reset(&self.swap_chain);
        self.pmfx.reset(&self.swap_chain);
    }
}

/// return an absolute path for a resource given the relative resource name from the /data dir
pub fn get_data_path(asset: &str) -> String {
    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap().join("..");
    String::from(asset_path.join(asset).to_str().unwrap())
}

/// return an absolute path for a resource given the relative path from the /executable dir
pub fn get_exe_path(asset: &str) -> String {
    let exe_path = std::env::current_exe().ok().unwrap();
    println!("{}", String::from(exe_path.join(asset).to_str().unwrap()));
    String::from(exe_path.join(asset).to_str().unwrap())
}

#[cfg(target_os = "windows")]
pub use os::win32 as os_platform;

#[cfg(target_os = "windows")]
pub use gfx::d3d12 as gfx_platform;