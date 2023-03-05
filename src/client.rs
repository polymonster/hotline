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

use imgui::UserInterface;
use plugin::PluginReloadResponder;
use reloader::Reloader;

use serde::{Deserialize, Serialize};

use std::path::PathBuf;
use std::collections::HashMap;
use std::time::SystemTime;

/// Information to create a hotline context which will create an app, window, device.
pub struct HotlineInfo {
    /// Name for the app and window title
    pub name: String,
    /// Window rect {pos_x pos_y, width, height}
    pub window_rect: os::Rect<i32>,
    /// Signify if the app is DPI aware or not
    pub dpi_aware: bool,
    /// Clear colour of the default swap chain
    pub clear_colour: Option<gfx::ClearColour>,
    /// Optional name of gpu adaptor, use None for the default / primary device
    pub adapter_name: Option<String>,
    /// Number of buffers in the swap chain (2 for double buffered, 3 for tripple etc)
    pub num_buffers: u32,
    /// Size of the default device heap for shader resources (textures, buffers, etc)
    pub shader_heap_size: usize, 
    /// Size of the default device heap for render targets
    pub render_target_heap_size: usize,
    /// Size of the default device heap for depth stencil targets
    pub depth_stencil_heap_size: usize,
    /// Optional user config, the default will be automatically located in the file system, this allows to override the launch configuration
    pub user_config: Option<UserConfig>
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
            depth_stencil_heap_size: 64,
            user_config: None
        }
    }
}

/// Hotline client data members
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
    pub libs: HashMap<String, hot_lib_reloader::LibReloader>,
    plugins: Vec<PluginCollection>,
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
    pub plugins: Option<HashMap<String, PluginInfo>>,
    pub plugin_data: Option<HashMap<String, String>>
}

/// Internal enum to track plugin state and syncornise unloads, reloads and setups etc.
#[derive(PartialEq, Eq)]
enum PluginState {
    None,
    Reload,
    Setup,
    Unload,
}

/// Container data describing a plugin 
struct PluginCollection {
    name: String,
    reloader: reloader::Reloader,
    instance: PluginInstance,
    state: PluginState
}

/// Hotline `Client` implementation
impl<D, A> Client<D, A> where D: gfx::Device, A: os::App {
    /// Create a hotline context consisting of core resources
    pub fn create(info: HotlineInfo) -> Result<Self, super::Error> {
        // read user config or get defaults
        let user_config_path = super::get_data_path("user_config.json");
        let saved_user_config = if std::path::Path::new(&user_config_path).exists() {
            let user_data = std::fs::read(user_config_path)?;
            serde_json::from_slice(&user_data).unwrap()
        }
        else {
            UserConfig {
                main_window_rect: info.window_rect,
                console_window_rect: None,
                plugin_data: Some(HashMap::new()),
                plugins: None
            }
        };
        
        // override by the supplied user config
        let user_config = info.user_config.unwrap_or(saved_user_config);
        
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
            fonts: vec![
                imgui::FontInfo {
                    filepath: super::get_data_path("data/fonts/cousine_regular.ttf"),
                    glyph_ranges: None
                },
                imgui::FontInfo {
                    filepath: super::get_data_path("data/fonts/font_awesome.ttf"),
                    glyph_ranges: Some(vec![
                        [font_awesome::MINIMUM_CODEPOINT as u32, font_awesome::MAXIMUM_CODEPOINT as u32]
                    ])
                }
            ]
        };
        let imgui = imgui::ImGui::create(&mut imgui_info)?;

        // pmfx
        let mut pmfx = pmfx::Pmfx::<D>::create();

        // core pipelines
        pmfx.load(super::get_data_path("data/shaders/imdraw").as_str())?;
        pmfx.create_pipeline(&device, "imdraw_blit", swap_chain.get_backbuffer_pass())?;

        let size = main_window.get_size();
        pmfx.update_window(&mut device, (size.x as f32, size.y as f32), "main_window");

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
            libs: HashMap::new()
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

        // reset main command buffer
        self.cmd_buf.reset(&self.swap_chain);

        // start imgui new frame
        self.imgui.new_frame(&mut self.app, &mut self.main_window, &mut self.device);

        let dock_input = self.imgui.main_dock_hovered();
        self.app.set_input_enabled(
            !self.imgui.want_capture_keyboard() || dock_input, 
            !self.imgui.want_capture_mouse() || dock_input);

        let size = self.main_window.get_size();
        self.pmfx.update_window(&mut self.device, (size.x as f32, size.y as f32), "main_window");

        let size = self.imgui.get_main_dock_size();
        self.pmfx.update_window(&mut self.device, size, "main_dock");

        // start new pmfx frame
        self.pmfx.new_frame(&mut self.device, &self.swap_chain);

        // user config changes
        self.update_user_config_windows();
    }

    /// internal function to manage tracking user config values and changes, writes to disk if change are detected
    fn save_user_config(&mut self) {
        let user_config_file_text = serde_json::to_string_pretty(&self.user_config).unwrap();
        let user_config_path = super::get_data_path("user_config.json");
        std::fs::File::create(&user_config_path).unwrap();
        std::fs::write(&user_config_path, user_config_file_text).unwrap();
    }

    /// Intenral function to save both the `user_config.json` and `imgui.ini` to a disk location, for saving re-usable presets
    fn save_configs_to_location(&mut self, path: &str) {
        let user_config_file_text = serde_json::to_string_pretty(&self.user_config).unwrap();
        let user_config_path = format!("{}/user_config.json", path);
        std::fs::File::create(&user_config_path).unwrap();
        std::fs::write(&user_config_path, user_config_file_text).unwrap();
    }
    
    /// internal function to manage tracking user config values and changes, writes to disk if change are detected
    fn update_user_config_windows(&mut self) {
        // track any changes and write once
        let mut invalidated = false;
        
        // main window pos / size
        let current = self.main_window.get_window_rect();
        if current.x > 0 && current.y > 0 && self.user_config.main_window_rect != current {
            self.user_config.main_window_rect = self.main_window.get_window_rect();
            invalidated = true;
        }

        // console window pos / size
        if let Some(console_window_rect) = self.user_config.console_window_rect {
            let current = self.app.get_console_window_rect();
            if current.x > 0 && current.y > 0 && console_window_rect != current {
                self.user_config.console_window_rect = Some(self.app.get_console_window_rect());
                invalidated = true;
            }
        }
        else {
            let current = self.app.get_console_window_rect();
            if current.x > 0 && current.y > 0 {
                self.user_config.console_window_rect = Some(self.app.get_console_window_rect());
                invalidated = true;
            }
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

    /// This assumes you pass the path to a `Cargo.toml` for a `dylib` which you want to load dynamically
    /// The lib can implement the `hotline_plugin!` and `Plugin` trait, but that is not required
    /// You can also just load libs and use `lib.get_symbol` to find custom callable code for other plugins.
    pub fn add_plugin_lib(&mut self, name: &str, path: &str) {
        let abs_path = if path == "/plugins" {
            super::get_data_path("../plugins")
        }
        else {
            String::from(path)
        };

        let lib_path = PathBuf::from(abs_path.to_string())
            .join("target")
            .join(crate::get_config_name())
            .to_str().unwrap().to_string();
        
        let src_path = PathBuf::from(abs_path.to_string())
            .join(name)
            .join("src")
            .join("lib.rs")
            .to_str().unwrap().to_string();

        let plugin = PluginReloadResponder {
            name: name.to_string(),
            path: abs_path.to_string(),
            output_filepath: lib_path.to_string(),
            files: vec![
                src_path
            ],
        };

        if !std::path::Path::new(&lib_path).join(name.to_string() + ".dll").exists() {
            println!("hotline_rs::client:: plugin not found: {}/{}", lib_path, name);
            return;
        }

        println!("hotline_rs::client:: loading plugin: {}/{}", lib_path, name);
        let lib = hot_lib_reloader::LibReloader::new(&lib_path, name, None).unwrap();
        unsafe {
            // create instance if it is a Plugin trait
            let create = lib.get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
            
            let instance = if let Ok(create) = create {
                // create function returns pointer to instance
                create()
            }
            else {
                // allow null instances, in plugins which only export function calls and not plugin traits
                std::ptr::null_mut()
            };
            
            // keep hold of everything for updating
            self.plugins.push( PluginCollection {
                name: name.to_string(),
                instance, 
                reloader: Reloader::create(Box::new(plugin)),
                state: PluginState::Setup
            });
            self.libs.insert(name.to_string(), lib);
        }

        // Track the plugin for auto re-loading
        if self.user_config.plugins.is_none() {
            self.user_config.plugins = Some(HashMap::new());
        }

        // plugins inside the main repro can have the abs path truncated so they are portable
        let hotline_path = super::get_data_path("..");
        let path = abs_path.replace(&hotline_path, "").replace('\\', "/");

        if let Some(plugin_info) = &mut self.user_config.plugins {
            if plugin_info.contains_key(name) {
                plugin_info.remove(name);
            }
            plugin_info.insert(name.to_string(), PluginInfo { path });
        }
    }

    /// Intenral core-ui function, it displays the main menu bar in the main window and
    /// A plugin menu which allows users to reload or unload live plugins.
    fn core_ui(&mut self) {
        // main menu bar 
        if self.imgui.begin_main_menu_bar() {
            
            if self.imgui.begin_menu("File") {
                // allow us to add plugins from files (libs)
                if self.imgui.menu_item("Open") {
                    let file = A::open_file_dialog(os::OpenFileDialogFlags::FILES, vec![".toml"]);
                    if let Ok(file) = file {
                        if !file.is_empty() {
                            // add plugin from dll
                            let plugin_path = PathBuf::from(file[0].to_string());
                            let plugin_name = plugin_path.parent().unwrap().file_name().unwrap();
                            let plugin_path = plugin_path.parent().unwrap().parent().unwrap();
                            self.add_plugin_lib(plugin_name.to_str().unwrap(), plugin_path.to_str().unwrap());
                        }
                    }
                }

                // save configs for presets
                if self.imgui.menu_item("Save User Config") {
                    let folder = A::open_file_dialog(os::OpenFileDialogFlags::FOLDERS, Vec::new());
                    if let Ok(folder) = folder {
                        if !folder.is_empty() {
                            self.save_configs_to_location(&folder[0]);
                            self.imgui.save_ini_settings_to_location(&folder[0]);
                        }
                    }
                }

                self.imgui.separator();
                if self.imgui.menu_item("Exit") {
                    self.app.exit(0);
                }

                self.imgui.end_menu();
            }

            // menu per plugin to allow the user to unload or reload
            if self.imgui.begin_menu("Plugin") {
                for plugin in &mut self.plugins {
                    if self.imgui.begin_menu(&plugin.name) {
                        if self.imgui.menu_item("Reload") {
                            plugin.state = PluginState::Setup;
                        }
                        if self.imgui.menu_item("Unload") {
                            plugin.state = PluginState::Unload;
                        }
                        self.imgui.end_menu();
                    }
                }
                self.imgui.end_menu();
            }
            self.imgui.end_main_menu_bar();
        }
    }

    /// Internal plugin yupdate function process reloads, setups and updates of hooked in plugins
    fn update_plugins(mut self) -> Self {
        // take the plugin mem so we can decouple the shared mutability between client and plugins
        let mut plugins = std::mem::take(&mut self.plugins);

        // call plugin ui functions
        for plugin in &mut plugins {
            let lib = self.libs.get(&plugin.name).expect("hotline::client: lib missing for plugin");
            unsafe {
                let ui = lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void, *mut core::ffi::c_void) -> Self>("ui".as_bytes());
                if let Ok(ui_fn) = ui {
                    let imgui_ctx = self.imgui.get_current_context();
                    self = ui_fn(self, plugin.instance, imgui_ctx);
                }
            }
        }

        // check for reloads
        let mut reload = false;
        for plugin in &mut plugins {
            if plugin.reloader.check_for_reload() == reloader::ReloadState::Available || plugin.state == PluginState::Reload {
                    self.swap_chain.wait_for_last_frame();
                    reload = true;
                    plugin.state = PluginState::Reload;
                    break;
            }
        }

        // if we require a reload, we also re-setup all the other plugins
        // this could be configured to only re-setup necessary plugins that are dependent
        if reload {
            for plugin in &mut plugins {
                if plugin.state == PluginState::None {
                    plugin.state = PluginState::Setup 
                }
            }
        }

        // perfrom unloads this will clean up memory, setup will be called again afterwards
        for plugin in &plugins {
            if plugin.state != PluginState::None {
                unsafe {
                    let lib = self.libs.get(&plugin.name).expect("hotline::client: lib missing for plugin");
                    let unload = lib.get_symbol::<unsafe extern fn(Self, PluginInstance) -> Self>("unload".as_bytes());
                    if let Ok(unload_fn) = unload {
                        self = unload_fn(self, plugin.instance);
                    }
                }
            }
        }

        // remove unloaded plugins entirely
        loop {
            let mut todo = false;
            for i in 0..plugins.len() {
                if plugins[i].state == PluginState::Unload {
                    if let Some(plugin_info) = &mut self.user_config.plugins {
                        plugin_info.remove_entry(&plugins[i].name);
                    }
                    self.libs.remove_entry(&plugins[i].name);
                    plugins.remove(i);
                    todo = true;
                    break;
                }
            }
            if !todo {
                break;
            }
        }
            
        // reload, actual reloading the lib of any libs which had changes
        for plugin in &mut plugins {
            if plugin.state == PluginState::Reload {                        
                // wait for lib reloader itself
                let lib = self.libs.get_mut(&plugin.name).expect("hotline::client: lib missing for plugin");
                let start = SystemTime::now();
                loop {
                    if lib.update().unwrap() {
                        break;
                    }
                    if start.elapsed().unwrap() > std::time::Duration::from_secs(10) {
                        println!("hotline::client: [warning] reloading plugin: {} timed out", plugin.name);
                        break;
                    }
                    std::hint::spin_loop();
                }

                // signal it's ok to continue
                plugin.reloader.complete_reload();

                // create a new instance of the plugin
                unsafe {
                    let create = lib.get_symbol::<unsafe extern fn() -> *mut core::ffi::c_void>("create".as_bytes());
                    if let Ok(create_fn) = create {
                        plugin.instance = create_fn();
                    }
                }
                // after reload, setup everything again
                plugin.state = PluginState::Setup;
            }
        }

        // setup
        for plugin in &plugins {
            let lib = self.libs.get(&plugin.name).expect("hotline::client: lib missing for plugin");
            unsafe {
                if plugin.state == PluginState::Setup {
                    let setup = lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("setup".as_bytes());
                    if let Ok(setup_fn) = setup {
                        self = setup_fn(self, plugin.instance);
                    }
                }
            }
        }
        
        // update
        for plugin in &mut plugins {
            let lib = self.libs.get(&plugin.name).expect("hotline::client: lib missing for plugin");
            unsafe {
                let update = lib.get_symbol::<unsafe extern fn(Self, *mut core::ffi::c_void) -> Self>("update".as_bytes());
                if let Ok(update_fn) = update {
                    self = update_fn(self, plugin.instance);
                }
            }
            plugin.state = PluginState::None;
        }

        // move plugins back and return self
        self.plugins = plugins;
        self
    }

    /// Allows users to pass serializable data which is stored into the `UserConfig` for the app.
    /// Plugin data is arrange as a json object / dictionary hash map as so:
    /// "plugin_data": {
    ///     "plugin_name": {
    ///         "plugin_data_members": true
    ///     }
    ///     "another_plugin_name": {
    ///         "another_plugin_name_data": true
    ///     }
    /// }
    pub fn serialise_plugin_data<T: Serialize>(&mut self, plugin_name: &str, data: &T) {
        let serialised = serde_json::to_string_pretty(&data).unwrap();
        if self.user_config.plugin_data.is_none() {
            self.user_config.plugin_data = Some(HashMap::new());
        }
        if let Some(plugin_data) = &mut self.user_config.plugin_data {
            *plugin_data.entry(plugin_name.to_string()).or_insert(String::new()) = serialised;
        }
    }

    /// Deserialises string json into a `T` returning defaults if the entry does not exist
    pub fn deserialise_plugin_data<'de, T: Deserialize<'de> + Default>(&'de mut self, plugin_name: &str) -> T {
        // deserialise user data saved from a previous session
        if let Some(plugin_data) = &self.user_config.plugin_data {
            if plugin_data.contains_key(plugin_name) {
                serde_json::from_slice(plugin_data[plugin_name].as_bytes()).unwrap()
            }
            else {
                T::default()
            }
        }
        else {
            T::default()
        }
    }
    
    /// Very simple run loop which can take control of your application, you could roll your own
    pub fn run(mut self) {
        while self.app.run() {

            self.new_frame();

            self.core_ui();
            self.pmfx.show_ui(&mut self.imgui, true);

            self = self.update_plugins();

            if let Some(tex) = self.pmfx.get_texture("main_colour") {
                self.imgui.image_window("main_dock", tex);
            }

            self.present("main_colour");
        }

        // save out values for next time
        self.save_user_config();
        self.imgui.save_ini_settings();

        self.wait_for_last_frame();
    }

    /// Very simple run loop which can take control of your application, you could roll your own
    pub fn run_once(mut self) {
        self.new_frame();
        
        //self.core_ui();
        
        self.pmfx.show_ui(&mut self.imgui, true);
        self = self.update_plugins();
        self.present("main_colour");

        self.wait_for_last_frame();
    }
}