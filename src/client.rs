use crate::gfx;
use crate::os;
use crate::imgui;
use crate::plugin::PluginInstance;
use crate::pmfx;
use crate::imdraw;
use crate::primitives;
use crate::plugin;
use crate::reloader;

use gfx::SwapChain;
use gfx::CmdBuf;
use gfx::Texture;
use gfx::RenderPass;

use os::Window;

use plugin::PluginLib;
use plugin::PluginCollection;
use plugin::PluginState;

use reloader::ReloadResponder;
use reloader::Reloader;

use serde::{Deserialize, Serialize};

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;

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

    lib: Vec<hot_lib_reloader::LibReloader>,
    plugins: Vec<PluginCollection>,
}

/// Serialisable camera info
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct CameraInfo {
    pub pos: (f32, f32, f32),
    pub rot: (f32, f32, f32),
    pub aspect: f32,
    pub fov: f32,
}

/// Serialisable plugin
#[derive(Serialize, Deserialize, Clone)]
pub struct PluginInfo {
    pub path: String
}

/// Serialisable user configration settings and saved state
#[derive(Serialize, Deserialize, Clone)]
pub struct UserConfig {
    // pos xy, size xy
    pub main_window_rect: os::Rect<i32>,
    pub console_window_rect: Option<os::Rect<i32>>,
    pub main_camera: Option<CameraInfo>,
    pub plugins: Option<HashMap<String, PluginInfo>>
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
                main_window_rect: info.window_rect,
                console_window_rect: None,
                main_camera: None,
                plugins: None
            }
        };
        
        // app
        let mut app = A::create(os::AppInfo {
            name: info.name.to_string(),
            num_buffers: info.num_buffers,
            dpi_aware: info.dpi_aware,
            window: false,
        });
        if let Some(console_rect) = user_config.console_window_rect {
            app.set_console_window_rect(console_rect);
        }
    
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

        // create a client
        let mut client = Client {
            app,
            device,
            main_window,
            swap_chain,
            cmd_buf,
            pmfx,
            imdraw,
            imgui,
            unit_quad_mesh,
            user_config: user_config.clone(),
            plugins: Vec::new(),
            lib: Vec::new()
        };

        // automatically load plugins from prev session
        if let Some(plugin_info) = &user_config.plugins {
            for (name, info) in plugin_info {
                client.add_plugin_lib(name, &info.path)
            }
        }
   
        Ok(client)
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
        self.update_user_config_windows();
    }

    /// internal function to manage tracking user config values and changes, writes to disk if change are detected
    fn save_user_config(&mut self) {
        let user_config_file_text = serde_json::to_string(&self.user_config).unwrap();
        let user_config_path = super::get_data_path("user_config.json");
        std::fs::File::create(&user_config_path).unwrap();
        std::fs::write(&user_config_path, user_config_file_text).unwrap();
    }
    
    /// internal function to manage tracking user config values and changes, writes to disk if change are detected
    fn update_user_config_windows(&mut self) {
        // track any changes and write once
        let mut invalidated = false;
        
        // main window pos / size
        if self.user_config.main_window_rect != self.main_window.get_window_rect() {
            self.user_config.main_window_rect = self.main_window.get_window_rect();
            invalidated = true;
        }

        // console window pos / size
        if let Some(console_window_rect) = self.user_config.console_window_rect {
            if console_window_rect != self.app.get_console_window_rect() {
                self.user_config.console_window_rect = Some(self.app.get_console_window_rect());
                invalidated = true;
            }
        }
        else {
            self.user_config.console_window_rect = Some(self.app.get_console_window_rect());
            invalidated = true;
        }

        // write to file
        if invalidated {
            self.save_user_config();
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
            name: name.to_string(),
            path: path.to_string(),
            output_filepath: lib_path.to_string(),
            files: vec![
                src_path
            ],
        };
        let lib = hot_lib_reloader::LibReloader::new(lib_path.to_string(), name.to_string(), None).unwrap();
        unsafe {
            // create instance if it is a Plugin trait
            let create = lib.get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
            let instance = if create.is_ok() {
                // create function returns pointer to instance
                create.unwrap()()
            }
            else {
                // allow null instances, in plugins which only export function calls and not plugin traits
                std::ptr::null_mut()
            };

            // box it up
            let plugin_box = Box::new(plugin);
            let plugin_ref : Arc<Mutex<Box<dyn ReloadResponder>>> = Arc::new(Mutex::new(plugin_box));
            let reloader = Reloader::create(plugin_ref.clone());
        
            // start watching for reloads
            reloader.start();

            // keep hold of everything gor updating
            self.plugins.push( plugin::PluginCollection {
                name: name.to_string(),
                lib: plugin_ref.clone(), 
                instance, 
                reloader,
                state: PluginState::Setup
            });
            self.lib.push(lib);
        }

        // Track the plugin for auto re-loading
        if self.user_config.plugins.is_none() {
            self.user_config.plugins = Some(HashMap::new());
        }

        if let Some(plugin_info) = &mut self.user_config.plugins {
            if plugin_info.contains_key(name) {
                plugin_info.remove(name);
            }
            plugin_info.insert(name.to_string(), PluginInfo { path: path.to_string() });
        }
    }

    pub fn call_lib_function_with_string<T>(&self, function: &str, arg: &str) -> Option<T> {
        for lib in &self.lib {
            unsafe {
                let hook = lib.get_symbol::<unsafe extern fn(String) -> Option<T>>(function.as_bytes());
                if hook.is_ok() {
                    let hook_fn = hook.unwrap();
                    println!("arg: {}", arg);
                    return hook_fn(arg.to_string());
                }
            }
        }
        None
    }
    
    pub fn run(mut self) {
        while self.app.run() {

            self.new_frame();

            // main menu bar
            if self.imgui.begin_main_menu_bar() {
                if self.imgui.begin_menu("File") {
                    if self.imgui.menu_item("Open", false, true) {
                        let file = A::open_file_dialog(os::OpenFileDialogFlags::FILES, vec![".toml"]);
                        if file.is_ok() {
                            let file = file.unwrap();
                            if !file.is_empty() {
                                // add plugin from dll
                                let plugin_path = PathBuf::from(file[0].to_string());
                                let plugin_name = plugin_path.parent().unwrap().file_name().unwrap();
                                let plugin_path = plugin_path.parent().unwrap().parent().unwrap();
                                self.add_plugin_lib(plugin_name.to_str().unwrap(), plugin_path.to_str().unwrap());
                            }
                        }
                    }
                    self.imgui.end_menu();
                }
                if self.imgui.begin_menu("View") {
                
                }
                if self.imgui.begin_menu("Plugin") {

                    for plugin in &self.plugins {
                        if self.imgui.menu_item(&plugin.name, false, true) {
                            if self.imgui.menu_item("Reload", false, true) {
                            
                            }
                        }
                    }

                    self.imgui.end_menu();
                }
                self.imgui.end_main_menu_bar();
            }

            let mut plugins = std::mem::take(&mut self.plugins);

            // check for reloads
            let mut reload = false;
            for plugin in &mut plugins {
                if plugin.reloader.check_for_reload() == reloader::ReloadResult::Reload 
                    || plugin.state == PluginState::Reload {
                        self.swap_chain.wait_for_last_frame();
                        reload = true;
                        plugin.state = PluginState::Reload;
                        break;
                }
            }

            // reload all... currently we need to reload all plugins
            if reload {
                let mut i = 0;
                for plugin in &plugins {
                    unsafe {
                        let reload = self.lib[i].get_symbol::<unsafe extern fn(Self, PluginInstance) -> Self>("reload".as_bytes());
                        if reload.is_ok() {
                            let reload_fn = reload.unwrap();
                            self = reload_fn(self, plugin.instance);
                        }
                    }
                    i = i+1;
                }
    
                // finalise reload (clean up)
                let mut i = 0;
                for plugin in &mut plugins {
                    if plugin.state == PluginState::Reload {
                        println!("complete reload {}", plugin.name);
                        
                        // wait
                        loop {
                            if self.lib[i].update().unwrap() {
                                break;
                            }
                            println!("lib waiting");
                            std::thread::sleep(std::time::Duration::from_millis(16));
                        }
                        plugin.reloader.complete_reload();

                        unsafe {
                            let create = self.lib[i].get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
                            if create.is_ok() {
                                let create_fn = create.unwrap();
                                plugin.instance = create_fn();
                            }
                        }
                    }
                    plugin.state = PluginState::Setup;
                    i = i+1;
                }
            }

            // setup
            let mut i = 0;
            for responder in &plugins {
                unsafe {
                    if responder.state == PluginState::Setup {
                        let setup = self.lib[i].get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("setup".as_bytes());
                        if setup.is_ok() {
                            let setup_fn = setup.unwrap();
                            self = setup_fn(self, responder.instance);
                        }
                    }
                }
                i = i + 1;
            }
            
            // update
            let mut i = 0;
            for responder in &mut plugins {
                unsafe {
                    responder.state = PluginState::None;
                    let update = self.lib[i].get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("update".as_bytes());
                    if update.is_ok() {
                        let update_fn = update.unwrap();
                        self = update_fn(self, responder.instance);
                    }
                }
                i = i + 1;
            }
            self.plugins = plugins;

            self.present("main_colour");
        }

        // save out values for next time
        self.save_user_config();

        self.wait_for_last_frame();
    }
}