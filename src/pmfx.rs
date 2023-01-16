
use crate::os;
use crate::gfx;
use crate::gfx::ResourceState;
use crate::os::Window;
use gfx::CmdBuf;

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::collections::HashSet;



// TODO: should use hashes?
// use std::hash::Hash;

use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::Path;

/// Everything you need to render a world view
pub struct View<D: gfx::Device> {
    pub pass: D::RenderPass,
    pub viewport: gfx::Viewport,
    pub scissor_rect: gfx::ScissorRect,
    pub cmd_buf: D::CmdBuf
}
pub type ViewRef<D> = Arc<Mutex<View<D>>>;

/// Compact mesh representation
#[derive(Clone)]
pub struct Mesh<D: gfx::Device> {
    pub vb: D::Buffer,
    pub ib: D::Buffer,
    pub num_indices: u32
}

struct TrackedTexture<D: gfx::Device>  {
    texture: D::Texture,
    ratio: Option<TextureSizeRatio>,
    size: (u64, u64)
}

/// Pmfx instance,containing render objects and resources
pub struct Pmfx<D: gfx::Device> {
    pmfx: File,
    pmfx_folders: HashMap<String, String>, 
    window_sizes: HashMap<String, (f32, f32)>,
    render_pipelines: HashMap<String, D::RenderPipeline>,
    compute_pipelines: HashMap<String, D::ComputePipeline>,
    shaders: HashMap<String, D::Shader>,
    textures: HashMap<String, TrackedTexture<D>>,
    views: HashMap<String, Arc<Mutex<View<D>>>>,
    barriers: HashMap<String, D::CmdBuf>,
    execute_graph: Vec<String>,
    view_texture_refs: HashMap<String, HashSet<String>>

}

/// Serialisation layout for contents inside .pmfx file
#[derive(Serialize, Deserialize)]
struct File {
    pipelines: HashMap<String, PipelinePermutations>,
    depth_stencil_states: HashMap<String, gfx::DepthStencilInfo>,
    textures: HashMap<String, TextureInfo>,
    views: HashMap<String, ViewInfo>,
    graphs: HashMap<String, Vec<ViewInstanceInfo>>
}

impl File {
    fn new() -> Self {
        File {
            pipelines: HashMap::new(),
            depth_stencil_states: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            graphs: HashMap::new()
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct TextureSizeRatio {
    window: String,
    scale: f32
}

#[derive(Serialize, Deserialize)]
struct TextureInfo {
    ratio: Option<TextureSizeRatio>,
    filepath: Option<String>,
    width: u64,
    height: u64,
    depth: u32,
    mip_levels: u32,
    array_levels: u32,
    samples: u32,
    format: gfx::Format,
    usage: Vec<ResourceState>
}

#[derive(Serialize, Deserialize)]
struct Pipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    vertex_layout: Option<gfx::InputLayout>,
    descriptor_layout: gfx::DescriptorLayout,
    blend_state: Option<String>,
    depth_stencil_state: Option<String>,
    raster_state: Option<String>,
    topology: Option<gfx::Topology>
}
type PipelinePermutations = HashMap<String, Pipeline>;

#[derive(Serialize, Deserialize, Clone)]
struct ViewInfo {
    render_target: Vec<String>,
    depth_stencil: Vec<String>,
    viewport: Vec<f32>,
    scissor: Vec<f32>,
    clear_colour: Option<Vec<f32>>,
    clear_depth: Option<f32>,
    clear_stencil: Option<u8>
}

#[derive(Serialize, Deserialize, Clone)]
struct ViewInstanceInfo {
    view: String
}

/// creates a shader from an option of filename, returning optional shader back
fn create_shader_from_file<D: gfx::Device>(device: &D, folder: &Path, file: Option<String>) -> Result<Option<D::Shader>, super::Error> {
    if let Some(shader) = file {
        let shader_filepath = folder.join(shader);
        let shader_data = fs::read(shader_filepath)?;                
        let shader_info = gfx::ShaderInfo {
            shader_type: gfx::ShaderType::Vertex,
            compile_info: None
        };
        Ok(Some(device.create_shader(&shader_info, &shader_data)?))
    }
    else {
        Ok(None)
    }
}

/// get gfx info from a pmfx state, returning default if it does not exist
fn info_from_state<T: Default + Copy>(name: &Option<String>, map: &HashMap<String, T>) -> T {
    if let Some(name) = &name {
        if map.contains_key(name) {
            map[name]
        }
        else {
            T::default()
        }
    }
    else {
        T::default()
    }
}

/// translate pmfx::TextureInfo to gfx::TextureInfo as pmfx::TextureInfo is slightly better equipped for user enty
fn to_gfx_texture_info(pmfx_texture: &TextureInfo, ratio_size: (u64, u64)) -> gfx::TextureInfo {
    // size from ratio
    let (width, height) = ratio_size;

    // infer texture type from dimensions
    let tex_type = if pmfx_texture.depth > 1 {
        gfx::TextureType::Texture3D
    }
    else if height > 1 {
        gfx::TextureType::Texture2D
    }
    else {
        gfx::TextureType::Texture1D
    };

    // derive initial state from usage
    let initial_state = if pmfx_texture.usage.contains(&ResourceState::ShaderResource) {
        ResourceState::ShaderResource
    }
    else if pmfx_texture.usage.contains(&ResourceState::DepthStencil) {
        ResourceState::DepthStencil
    }
    else if pmfx_texture.usage.contains(&ResourceState::RenderTarget) {
        ResourceState::RenderTarget
    }
    else {
        ResourceState::ShaderResource
    };

    // texture type bitflags from vec of enum
    let mut usage = gfx::TextureUsage::NONE;
    for pmfx_usage in &pmfx_texture.usage {
        match pmfx_usage {
            ResourceState::ShaderResource => {
                usage |= gfx::TextureUsage::SHADER_RESOURCE
            }
            ResourceState::UnorderedAccess => {
                usage |= gfx::TextureUsage::UNORDERED_ACCESS
            }
            ResourceState::RenderTarget => {
                usage |= gfx::TextureUsage::RENDER_TARGET
            }
            ResourceState::DepthStencil => {
                usage |= gfx::TextureUsage::DEPTH_STENCIL
            }
            _ => {}
        }
    }

    gfx::TextureInfo {
        width,
        height,
        tex_type,
        initial_state,
        usage,
        depth: pmfx_texture.depth,
        mip_levels: pmfx_texture.mip_levels,
        array_levels: pmfx_texture.array_levels,
        samples: pmfx_texture.samples,
        format: pmfx_texture.format,
    }
}

fn to_gfx_clear_colour(clear_colour: Option<Vec<f32>>) -> Option<gfx::ClearColour> {
    if let Some(col) = clear_colour {
        match col.len() {
            len if len >= 4 => {
                Some( gfx::ClearColour {
                    r: col[0],
                    g: col[1],
                    b: col[2],
                    a: col[3],
                })
            }
            3 => {
                Some( gfx::ClearColour {
                    r: col[0],
                    g: col[1],
                    b: col[2],
                    a: 1.0
                })
            }
            2 => {
                Some( gfx::ClearColour {
                    r: col[0],
                    g: col[1],
                    b: 0.0,
                    a: 1.0
                })
            }
            1 => {
                Some( gfx::ClearColour {
                    r: col[0],
                    g: 0.0,
                    b: 0.0,
                    a: 1.0
                })
            }
            _ => None
        }
    }
    else {
        None
    }
}

fn to_gfx_clear_depth_stencil(clear_depth: Option<f32>, clear_stencil: Option<u8>) -> Option<gfx::ClearDepthStencil> {
    if clear_depth.is_some() || clear_stencil.is_some() {
        Some( gfx::ClearDepthStencil {
            depth: clear_depth,
            stencil: clear_stencil
        })
    }
    else {
        None
    }
}

impl<D> Pmfx<D> where D: gfx::Device {
    /// Create a new empty pmfx instance
    pub fn create() -> Self {        
        Pmfx {
            pmfx: File::new(),
            pmfx_folders: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            shaders: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            barriers: HashMap::new(),
            execute_graph: Vec::new(),
            view_texture_refs: HashMap::new(),
            window_sizes: HashMap::new()
        }
    }

    /// Load a pmfx from a folder, where the folder contains a pmfx info.json and shader binaries in separate files
    /// within the directory
    pub fn load(&mut self, filepath: &str) -> Result<(), super::Error> {        
        // get the name for indexing by pmfx name/folder
        let folder = Path::new(filepath);
        let pmfx_name = if let Some(name) = folder.file_name() {
            String::from(name.to_os_string().to_str().unwrap())
        }
        else {
            String::from(filepath)
        };

        //  deserialise pmfx pipelines from file
        let info_filepath = folder.join(format!("{}.json", pmfx_name));
        let pmfx_data = fs::read(info_filepath)?;

        // TODO: this should merge in
        self.pmfx = serde_json::from_slice(&pmfx_data).unwrap();

        // insert lookup path for shaders as they go into a folder: pmfx/shaders.vsc
        for name in self.pmfx.pipelines.keys() {
            self.pmfx_folders.insert(name.to_string(), String::from(filepath));
        }
        
        Ok(())
    }

    fn create_shader<'stack>(shaders: &'stack mut HashMap<String, D::Shader>, device: &D, folder: &Path, file: &Option<String>) -> Result<(), super::Error> {
        if let Some(file) = file {
            if !shaders.contains_key(file) {
                println!("hotline_rs::pmfx:: compiling shader: {}", file);
                let shader = create_shader_from_file(device, folder, Some(file.to_string()));
                if let Some(shader) = shader.unwrap() {
                    println!("hotline_rs::pmfx:: success: {}", file);
                    shaders.insert(file.to_string(), shader);
                    Ok(())
                }
                else {
                    Ok(())
                }
            }
            else {
                Ok(())
            }
        }
        else {
            Ok(())
        }
    }

    /// Returns a shader reference for use when building pso
    pub fn get_shader<'stack>(&'stack self, file: &Option<String>) -> Option<&'stack D::Shader> {
        if let Some(file) = file {
            if self.shaders.contains_key(file) {
                Some(&self.shaders[file])
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    /// expands width and height for a texture account for ratio scaling linked to windows, pass the info.width / height
    /// of we have no ratio specified
    fn get_texture_size_from_ratio(&self, pmfx_texture: &TextureInfo) -> Result<(u64, u64), super::Error> {
        if let Some(ratio) = &pmfx_texture.ratio {
            if self.window_sizes.contains_key(&ratio.window) {
                let size = self.window_sizes[&ratio.window];
                Ok(((size.0 * ratio.scale) as u64, (size.1 * ratio.scale) as u64))
            }
            else {
                Err(super::Error {
                    msg: format!("hotline_rs::pmfx:: could not find window for ratio: {}", ratio.window),
                })
            }
        }
        else {
            Ok((pmfx_texture.width, pmfx_texture.height))
        }
    }

    /// Creates a texture if it has not already been created from information specified in .pmfx file
    pub fn create_texture(&mut self, device: &mut D, texture_name: &str) -> Result<(), super::Error> {
        if !self.textures.contains_key(texture_name) && self.pmfx.textures.contains_key(texture_name) {
            // create texture from info specified in .pmfx file
            println!("hotline_rs::pmfx:: creating texture: {}", texture_name);
            let pmfx_tex = &self.pmfx.textures[texture_name];
            let size = self.get_texture_size_from_ratio(pmfx_tex)?;
            let tex = device.create_texture::<u8>(&to_gfx_texture_info(pmfx_tex, size), None)?;
            self.textures.insert(texture_name.to_string(), TrackedTexture {
                texture: tex,
                ratio: self.pmfx.textures[texture_name].ratio.clone(),
                size
            });
        }
        Ok(())
    }

    /// Returns a texture reference if the texture exists or none otherwise
    pub fn get_texture<'stack>(&'stack self, texture_name: &str) -> Option<&'stack D::Texture> {
        if self.textures.contains_key(texture_name) {
            Some(&self.textures[texture_name].texture)
        }
        else {
            None
        }
    }

    /// Returns the tuple (width, height) of a texture
    pub fn get_texture_2d_size(&self, texture_name: &str) -> Option<(u64, u64)> {
        if self.textures.contains_key(texture_name) {
            Some(self.textures[texture_name].size)
        }
        else {
            None
        }
    }

    /// Create a view from information specified in pmfx file
    pub fn create_view(&mut self, device: &mut D, view_name: &str) -> Result<(), super::Error> {
        // create textures
        if !self.views.contains_key(view_name) && self.pmfx.views.contains_key(view_name) {
            // create pass from targets
            let pmfx_view = self.pmfx.views[view_name].clone();

            // create textures for targets
            let mut render_targets = Vec::new();
            for name in &pmfx_view.render_target {
                self.create_texture(device, name)?;

                // TODO: tidy
                if !self.view_texture_refs.contains_key(name) {
                    self.view_texture_refs.insert(name.to_string(), HashSet::new());
                }
                self.view_texture_refs.get_mut(name).unwrap().insert(view_name.to_string());
            }

            // create textures for depth stencils
            for name in &pmfx_view.depth_stencil {
                self.create_texture(device, name)?;

                // TODO: tidy
                if !self.view_texture_refs.contains_key(name) {
                    self.view_texture_refs.insert(name.to_string(), HashSet::new());
                }
                self.view_texture_refs.get_mut(name).unwrap().insert(view_name.to_string());
            }

            let mut size = (0, 0);

            // array of targets by name
            for name in &pmfx_view.render_target {
                render_targets.push(self.get_texture(name).unwrap());
                size = self.get_texture_2d_size(name).unwrap();
            }

            // get depth stencil by name
            let depth_stencil = if !pmfx_view.depth_stencil.is_empty() {
                let name = &pmfx_view.depth_stencil[0];
                size = self.get_texture_2d_size(name).unwrap();
                Some(self.get_texture(name).unwrap())
            }
            else {
                None
            };

            // pass for render targets with depth stencil
            let render_target_pass = device
            .create_render_pass(&gfx::RenderPassInfo {
                render_targets,
                rt_clear: to_gfx_clear_colour(pmfx_view.clear_colour),
                depth_stencil,
                ds_clear: to_gfx_clear_depth_stencil(pmfx_view.clear_depth, pmfx_view.clear_stencil),
                resolve: false,
                discard: false,
            })
            .unwrap();

            let view = View::<D> {
                pass: render_target_pass,
                viewport: gfx::Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: size.0 as f32,
                    height: size.1 as f32,
                    min_depth: 0.0,
                    max_depth: 1.0
                },
                scissor_rect: gfx::ScissorRect {
                    left: 0,
                    top: 0,
                    right: size.0 as i32,
                    bottom: size.1 as i32
                },
                cmd_buf: device.create_cmd_buf(2)
            };

            self.views.insert(view_name.to_string(), Arc::new(Mutex::new(view)));
        }

        Ok(())
    }

    /// Return a reference to a view if the view exists or none otherwise
    pub fn get_view(&self, view_name: &str) -> Option<ViewRef<D>> {
        if self.views.contains_key(view_name) {
            Some(self.views[view_name].clone())
        }
        else {
            None
        }
    }

    /// Create all views required for a render graph if necessary, skip if a view already exists
    pub fn create_graph_views(&mut self, device: &mut D, graph_name: &str) -> Result<(), super::Error> {
        // create views for all of the nodes
        if self.pmfx.graphs.contains_key(graph_name) {
            let pmfx_graph = self.pmfx.graphs[graph_name].clone();
            for node in pmfx_graph {
                // create view for each node
                self.create_view(device, &node.view)?;
            }
        }
        Ok(())
    }

    fn create_texture_transition_barrier(
        &mut self,
        device: &mut D,
        texture_barriers: &mut HashMap<String, ResourceState>, 
        view_name: &str, 
        texture_name: &str, 
        target_state: ResourceState) -> Result<(), super::Error> {
        if texture_barriers.contains_key(texture_name) {
            let state = texture_barriers[texture_name];
            if state != target_state {
                // add barrier placeholder in the execute order
                let barrier_name = format!("barrier_{}-{}", view_name, texture_name);
                self.execute_graph.push(barrier_name.to_string());

                // create a command buffer
                let mut cmd_buf = device.create_cmd_buf(1);
                cmd_buf.transition_barrier(&gfx::TransitionBarrier {
                    texture: Some(self.get_texture(&texture_name).unwrap()),
                    buffer: None,
                    state_before: state,
                    state_after: target_state,
                });
                cmd_buf.close()?;
                self.barriers.insert(barrier_name.to_string(), cmd_buf);
    
                // update track state
                texture_barriers.remove(texture_name);
                texture_barriers.insert(texture_name.to_string(), target_state);
            }
        }
        Ok(())
    }

    /// Create a render graph wih automatic resource barrier generation from info specified insie .pmfx file
    pub fn create_graph(&mut self, device: &mut D, graph_name: &str) -> Result<(), super::Error> {

        // create views for any nodes in the graph
        self.create_graph_views(device, graph_name)?;

        // gather up all render targets and check which ones want to be both written to and also uses as shader resources
        let mut barriers = HashMap::new();
        for (name, texture) in &self.pmfx.textures {
            if texture.usage.contains(&ResourceState::ShaderResource) {
                if texture.usage.contains(&ResourceState::RenderTarget) || 
                    texture.usage.contains(&ResourceState::DepthStencil) {
                        barriers.insert(name.to_string(), ResourceState::ShaderResource);
                }
            }
        }
        
        // go through the graph sequentially, as the command lists are executed in order but generated async
        if self.pmfx.graphs.contains_key(graph_name) {
            let pmfx_graph = self.pmfx.graphs[graph_name].clone();
            for node in pmfx_graph {
                // create transitions by inspecting view info
                let pmfx_view = self.pmfx.views[&node.view].clone();

                // if we need to write to a target we must make sure it is transitioned into render target state
                for rt_name in pmfx_view.render_target {
                    self.create_texture_transition_barrier(
                        device, &mut barriers, &node.view, &rt_name, ResourceState::RenderTarget)?;

                }

                // same for depth stencils
                for ds_name in pmfx_view.depth_stencil {
                    self.create_texture_transition_barrier(
                        device, &mut barriers, &node.view, &ds_name, ResourceState::DepthStencil)?;

                }

                // if we want to read from we can put this in pmfx
                // TODO: barriers to transition to ShaderResource

                // push a view on
                self.execute_graph.push(node.view.to_string());
            }
        }

        // finally all targets which are in the 'barriers' array are transitioned to shader resources (for debug views)
        let mut srvs = Vec::new();
        for name in barriers.keys() {
            srvs.push(name.to_string());
        }

        for name in srvs {
            self.create_texture_transition_barrier(
                device, &mut barriers, "eof", &name, ResourceState::ShaderResource)?;
        }

        Ok(())
    }

    /// Create a RenderPipeline instance for the combination of pmfx_pipeline settings and an associated RenderPass
    pub fn create_pipeline(&mut self, device: &D, pipeline_name: &str, pass: &D::RenderPass) -> Result<(), super::Error> {        
        // grab the pmfx pipeline info
        if self.pmfx.pipelines.contains_key(pipeline_name) {
            let pipeline = &self.pmfx.pipelines[pipeline_name]["0"];
            let shaders = &mut self.shaders;

            // TODO: shader array
            let folder = self.pmfx_folders[pipeline_name].to_string();
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.vs)?;
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.ps)?;
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.cs)?;

            // TODO: infer compute or graphics pipeline from pmfx
            let cs = self.get_shader(&pipeline.cs);
            if let Some(cs) = cs {
                let pso = device.create_compute_pipeline(&gfx::ComputePipelineInfo {
                    cs,
                    descriptor_layout: pipeline.descriptor_layout.clone(),
                })?;
                println!("hotline_rs::pmfx:: compiled compute pipeline: {}", pipeline_name);
                self.compute_pipelines.insert(pipeline_name.to_string(), pso);
            }
            else {
                let vertex_layout = pipeline.vertex_layout.as_ref().unwrap();
                let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
                    vs: self.get_shader(&pipeline.vs),
                    fs: self.get_shader(&pipeline.ps),
                    input_layout: vertex_layout.to_vec(),
                    descriptor_layout: pipeline.descriptor_layout.clone(),
                    raster_info: gfx::RasterInfo::default(),
                    depth_stencil_info: info_from_state(&pipeline.depth_stencil_state, &self.pmfx.depth_stencil_states),
                    blend_info: gfx::BlendInfo {
                        alpha_to_coverage_enabled: false,
                        independent_blend_enabled: false,
                        render_target: vec![gfx::RenderTargetBlendInfo::default()],
                    },
                    topology: 
                        if let Some(topology) = pipeline.topology {
                            topology
                        }
                        else {
                            gfx::Topology::TriangleList
                        },
                    patch_index: 0,
                    pass,
                })?;
                println!("hotline_rs::pmfx:: compiled render pipeline: {}", pipeline_name);
                self.render_pipelines.insert(pipeline_name.to_string(), pso);
            }
            Ok(())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find pipeline: {}", pipeline_name),
            })
        }
    }

    /// Fetch a prebuilt RenderPipeline
    pub fn get_render_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::RenderPipeline> {
        if self.render_pipelines.contains_key(pipeline_name) {
            Some(&self.render_pipelines[pipeline_name])
        }
        else {
            None
        }
    }

    /// Fetch a prebuilt ComputePipeline
    pub fn get_compute_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::ComputePipeline> {
        if self.compute_pipelines.contains_key(pipeline_name) {
            Some(&self.compute_pipelines[pipeline_name])
        }
        else {
            None
        }
    }

    /// Start a new frame and syncronise command buffers to the designated swap chain
    pub fn new_frame(&mut self, swap_chain: &D::SwapChain) {
        self.reset(swap_chain);
    }

    /// Update render targets or views associated with a window, this will resize textures and rebuild views
    /// which need to be modified if a window size changes
    pub fn update_window<A: os::App>(&mut self, device: &mut D, window: &A::Window, name: &str) {
        let size = window.get_size();
        let size = (size.x as f32, size.y as f32);
        let mut rebuild_views = Vec::new();
        let mut recreate_textures = Vec::new();
        if self.window_sizes.contains_key(name) {
            if self.window_sizes[name] != size {
                // update tracked textures
                for (texture_name, texture) in &self.textures {
                    if let Some(ratio) = &texture.ratio {
                        if ratio.window == name {
                            print!("texture needs update: {}", ratio.window);
                            if self.view_texture_refs.contains_key(texture_name) {
                                for view_name in &self.view_texture_refs[texture_name] {
                                    self.views.remove(view_name);
                                    rebuild_views.push(view_name.to_string());
                                    recreate_textures.push(texture_name.to_string());
                                }
                            }
                        }
                    }
                }

                // update the size
                self.window_sizes.remove(name);
            }
        }
        // insert window to track
        self.window_sizes.insert(name.to_string(), size);

        // recreate textures for the new sizes
        for texture_name in recreate_textures {
            self.textures.remove(&texture_name);
            self.create_texture(device, &texture_name).unwrap();
        }

        // recreate views with updated data
        for view in &rebuild_views {
            self.create_view(device, view).unwrap();
        }
    }

    /// Reset all command buffers
    pub fn reset(&mut self, swap_chain: &D::SwapChain) {
        for view in self.views.values() {
            let view = view.clone();
            view.lock().unwrap().cmd_buf.reset(swap_chain);
        }
    }

    /// Execute command buffers in order
    pub fn execute(
        &mut self,
        device: &mut D) {

        for node in &self.execute_graph {
            println!("execute: {}", node);
            if self.barriers.contains_key(node) {
                // transition barriers
                device.execute(&self.barriers[node]);
            }
            else if self.views.contains_key(node) {
                // dispatch a view
                let view = self.views[node].clone();
                let view = &mut view.lock().unwrap();
                view.cmd_buf.close().unwrap();
                device.execute(&view.cmd_buf);
            }
        }
    }
}

