use crate::gfx;
use crate::os;
use crate::imgui;
use crate::pmfx;
use crate::imdraw;
use crate::primitives;

use gfx::SwapChain;
use gfx::CmdBuf;
use gfx::Texture;
use gfx::RenderPass;

use os::Window;

use libloading::Symbol;
use serde::{Deserialize, Serialize};

use std::time::Duration;
use std::process::Command;
use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::any::Any;

/// Information to create a hotline context which will create an app, window, device
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

/// Useful defaults for quick HotlineInfo initialisation
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

type PluginLibRef = Arc<Mutex<Box<dyn ReloadResponder>>>;
type PluginInstance = *mut core::ffi::c_void;


/// Hotline client 
pub struct Client<D: gfx::Device, A: os::App> {
    pub app: A,
    pub device: D,
    pub main_window: A::Window,
    pub swap_chain: D::SwapChain,
    pub pmfx: pmfx::Pmfx<D>,
    pub cmd_buf: D::CmdBuf,
    pub imdraw: imdraw::ImDraw<D>,
    pub imgui: imgui::ImGui<D, A>,
    pub unit_quad_mesh: pmfx::Mesh<D>,
    pub user_config: UserConfig,

    new_responders: Vec<(PluginLibRef, PluginInstance, Reloader)>,
    run_setup: bool
}

/// Serialisable user configration settings and saved state
#[derive(Serialize, Deserialize)]
pub struct UserConfig {
    // pos xy, size xy
    pub main_window_rect: os::Rect<i32>
}

impl<D, A> Client<D, A> where D: gfx::Device, A: os::App {
    /// Create a hotline context consisting of core resources
    pub fn create(info: HotlineInfo) -> Result<Self, super::Error> {
        // read user config or get defaults
        let user_config_path = super::get_data_path("user_config.json");
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
                filepath: super::get_data_path("data/fonts/roboto_medium.ttf"),
                glyph_ranges: None
            }],
        };
        let imgui = imgui::ImGui::create(&mut imgui_info)?;

        // pmfx
        let mut pmfx = pmfx::Pmfx::<D>::create();

        // core pipelines
        pmfx.load(&super::get_data_path("data/shaders/imdraw").as_str())?;
        pmfx.create_pipeline(&mut device, "imdraw_blit", &swap_chain.get_backbuffer_pass())?;

        pmfx.update_window::<A>(&mut device, &main_window, "main_window");

        // blit pmfx
        let unit_quad_mesh = primitives::create_unit_quad_mesh(&mut device);

        // default cmd buf
        let cmd_buf = device.create_cmd_buf(info.num_buffers);
    
        Ok(Client {
            app,
            device,
            main_window,
            swap_chain,
            cmd_buf,
            pmfx,
            imdraw,
            imgui,
            unit_quad_mesh,
            user_config,
            new_responders: Vec::new(),
            run_setup: false
        })
    }

    /// Start a new frame syncronised to the swap chain
    pub fn new_frame(&mut self) {
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
            let user_config_path = super::get_data_path("user_config.json");

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
       
        // blit to main window
        let vp_rect = self.main_window.get_viewport_rect();
        self.cmd_buf.begin_event(0xff0000ff, "Blit Pmfx");
        self.cmd_buf.set_viewport(&gfx::Viewport::from(vp_rect));
        self.cmd_buf.set_scissor_rect(&gfx::ScissorRect::from(vp_rect));
        
        // get srv index of the pmfx target to blit to the window, if the target exists
        if let Some(tex) = self.pmfx.get_texture(blit_view_name) {
            let srv = tex.get_srv_index().unwrap();
            let fmt = self.swap_chain.get_backbuffer_pass_mut().get_format_hash();
            self.cmd_buf.set_render_pipeline(self.pmfx.get_render_pipeline_for_format("imdraw_blit", fmt).unwrap());
            self.cmd_buf.push_constants(0, 2, 0, &[vp_rect.width as f32, vp_rect.height as f32]);
            self.cmd_buf.set_render_heap(1, self.device.get_shader_heap(), srv);
            self.cmd_buf.set_index_buffer(&self.unit_quad_mesh.ib);
            self.cmd_buf.set_vertex_buffer(&self.unit_quad_mesh.vb, 0);
            self.cmd_buf.draw_indexed_instanced(6, 1, 0, 0, 0);
        }

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

    /// Wait for the last submitted frame to complete to ensure safe shutdown once all in-flight resources are no longer needed
    pub fn wait_for_last_frame(&mut self) {
        self.swap_chain.wait_for_last_frame();
        self.cmd_buf.reset(&self.swap_chain);
        self.pmfx.reset(&self.swap_chain);
    }

    pub fn add_plugin_lib(&mut self, name: &str, path: &str) {
        let lib_path = path.to_string() + "/target/" + crate::get_config_name();
        let src_path = path.to_string() + "/" + name + "/src/lib.rs";

        let plugin = PluginLib {
            lib: hot_lib_reloader::LibReloader::new(lib_path.to_string(), name.to_string(), None).unwrap(),
            files: vec![
                src_path
            ],
        };
        unsafe {
            let create = plugin.lib.get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
            if create.is_ok() {
                // create an instance of the plugin
                let create_fn = create.unwrap();
                let instance = create_fn();

                // box it up
                let plugin_box = Box::new(plugin);
                let plugin_ref : Arc<Mutex<Box<dyn ReloadResponder>>> = Arc::new(Mutex::new(plugin_box));
                let reloader = Reloader::create(plugin_ref.clone());
            
                // start watching for reloads
                reloader.start();

                // keep hold of everything gor updating
                self.new_responders.push((plugin_ref.clone(), instance, reloader));
            }
        }

        self.run_setup = true;
    }

    pub fn run(mut self) {
        while self.app.run() {

            self.new_frame();

            let mut new_responders = std::mem::take(&mut self.new_responders);

            // check for reloads
            for responder in &mut new_responders {
                if responder.2.check_for_reload() == ReloadResult::Reload {
                    self.swap_chain.wait_for_last_frame();

                    let plugin = responder.0.lock().unwrap();
                    if let Some(plugin) = plugin.as_any().downcast_ref::<PluginLib>() {
                        unsafe {
                            let reload = plugin.lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("reload".as_bytes());
                            if reload.is_ok() {
                                let reload_fn = reload.unwrap();
                                self = reload_fn(self, responder.1);
                            }
                        }
                    }
                    drop(plugin);

                    responder.2.complete_reload();

                    // create a new instance of the plugin
                    let plugin = responder.0.lock().unwrap();
                    if let Some(plugin) = plugin.as_any().downcast_ref::<PluginLib>() {
                        unsafe {
                            let create = plugin.lib.get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
                            if create.is_ok() {
                                let create_fn = create.unwrap();
                                responder.1 = create_fn();
                            }
                            self.run_setup = true;
                        }
                    }
                    break;
                }
            }

            // setup / update
            for responder in &new_responders {
                let plugin = responder.0.lock().unwrap();
                if let Some(plugin) = plugin.as_any().downcast_ref::<PluginLib>() {
                    unsafe {
                        if self.run_setup {
                            let setup = plugin.lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("setup".as_bytes());
                            if setup.is_ok() {
                                let setup_fn = setup.unwrap();
                                self = setup_fn(self, responder.1);
                            }
                        }
    
                        let update = plugin.lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("update".as_bytes());
                        if update.is_ok() {
                            let update_fn = update.unwrap();
                            self = update_fn(self, responder.1);
                        }
                    }
                }
            }
            self.run_setup = false;
            self.new_responders = new_responders;

            self.present("main_colour");
        }

        self.wait_for_last_frame();
    }
}

/// Basic Reloader which can check timestamps on files and then callback functions supplied by the reload responder
pub struct Reloader {
    /// Hash map storing files grouped by type (pmfx, code) and then keep a vector of files
    /// and timestamps for quick checking at run time.
    lock: Arc<Mutex<ReloadState>>,
    responder: Arc<Mutex<Box<dyn ReloadResponder>>>
}

/// Internal private enum to track reload states
#[derive(PartialEq)]
enum ReloadState {
    None,
    Requested,
    Confirmed,
}

#[derive(PartialEq)]
pub enum ReloadResult {
    Continue,
    Reload
}

pub trait ReloadResponder: Send + Sync {
    fn get_files(&self) -> &Vec<String>;
    fn build(&mut self);
    fn wait_for_completion(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct PluginLib {
    lib: hot_lib_reloader::LibReloader,
    files: Vec<String>
}

impl PluginLib {
    pub fn get_symbol<T>(&self, name: &str) -> Option<Symbol<T>> {
        unsafe {
            let get_function = self.lib.get_symbol::<T>(name.as_bytes());
            if get_function.is_ok() {
                return Some(get_function.unwrap());
            }
            else {
                None
            }
        }
    }
}

impl ReloadResponder for PluginLib {
    fn get_files(&self) -> &Vec<String> {
        &self.files
    }

    fn build(&mut self) {
        let output = Command::new("cargo")
            .current_dir("C:\\Users\\alex_\\dev\\hotline\\plugins")
            .arg("build")
            .arg("-p")
            .arg("ecs")
            .output()
            .expect("hotline::hot_lib:: hot lib failed to build!");

        if output.stdout.len() > 0 {
            println!("{}", String::from_utf8(output.stdout).unwrap());
        }

        if output.stderr.len() > 0 {
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }
    }

    fn wait_for_completion(&mut self) {
        // wait for lib to reload
        loop {
            if self.lib.update().unwrap() {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Reloader {
    /// Create a new instance of a reload with the designated ReloadResponder
    pub fn create(responder: Arc<Mutex<Box<dyn ReloadResponder>>>) -> Self {
        Self {
            lock: Arc::new(Mutex::new(ReloadState::None)),
            responder
        }
    }

    /// Start watching for and invoking reload changes, this will spawn threads to watch files
    pub fn start(&self) {
        self.file_watcher_thread();
    }

    /// Call this each frame, if ReloadResult::Reload you must then clean up any data in preperation for a reload
    pub fn check_for_reload(&self) -> ReloadResult {
        let lock = self.lock.lock().unwrap();
        if *lock == ReloadState::Requested {
            ReloadResult::Reload
        }
        else {
            ReloadResult::Continue
        }
    }

    /// Once data is cleaned up and it is safe to proceed this functions must be called 
    pub fn complete_reload(&mut self) {
        println!("hotline_rs::reloader:: wait for completion");
        self.responder.lock().unwrap().wait_for_completion();

        let mut lock = self.lock.lock().unwrap();
        // signal it is safe to proceed and reload the new code
        *lock = ReloadState::Confirmed;
        drop(lock);
        println!("hotline_rs::reloader:: confirmed");
    }

    fn file_watcher_thread_check_mtime(responder: &Arc<Mutex<Box<dyn ReloadResponder>>>, cur_mtime: SystemTime) -> SystemTime {
        let responder = responder.lock().unwrap();
        let files = responder.get_files();
        for file in files {
            let filepath = super::get_data_path(file);
            let meta = std::fs::metadata(&filepath);
            if meta.is_ok() {
                let mtime = std::fs::metadata(&filepath).unwrap().modified().unwrap();
                if mtime > cur_mtime {
                    return mtime;
                }
            }
            else {
                print!("hotline_rs::reloader: {filepath} not found!")
            }
        };
        cur_mtime
    }

    /// Background thread will watch for changed filestamps among the registered files from the responder
    fn file_watcher_thread(&self) {
        let mut cur_mtime = SystemTime::now();
        let lock = self.lock.clone();
        let responder = self.responder.clone();
        thread::spawn(move || {
            loop {
                
                let mtime = Self::file_watcher_thread_check_mtime(&responder, cur_mtime);

                if mtime > cur_mtime {
                    println!("hotline_rs::reloader: changes detected, building");

                    let mut responder = responder.lock().unwrap();
                    responder.build();

                    let mut a = lock.lock().unwrap();
                    println!("hotline_rs::reloader: reload requested");
                    *a = ReloadState::Requested;
                    drop(a);
        
                    cur_mtime = mtime;
                }

                std::thread::sleep(Duration::from_millis(16));
            }
        });
    }
}