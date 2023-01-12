
use crate::gfx;
use crate::primitives;

use gfx::CmdBuf;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// TODO: should use hashes?
// use std::hash::Hash;

use std::fs;
use std::sync::Arc;
use std::sync::Mutex;

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

/// Pmfx instance,containing render objects and resources
pub struct Pmfx<D: gfx::Device> {
    pmfx: File,
    pmfx_folders: HashMap<String, String>,
    render_pipelines: HashMap<String, D::RenderPipeline>,
    compute_pipelines: HashMap<String, D::ComputePipeline>,
    shaders: HashMap<String, D::Shader>,
    textures: HashMap<String, D::Texture>,
    tracked_textures: Vec<String>,
    views: HashMap<String, Arc<Mutex<View<D>>>>,
    // unit_quad_mesh: Mesh<D>
}

#[derive(Serialize, Deserialize)]
struct File {
    pipelines: HashMap<String, PipelinePermutations>,
    depth_stencil_states: HashMap<String, gfx::DepthStencilInfo>,
    textures: HashMap<String, TextureInfo>,
    views: HashMap<String, ViewInfo>,
}

impl File {
    fn new() -> Self {
        File {
            pipelines: HashMap::new(),
            depth_stencil_states: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new()
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TextureSizeRatio {
    window: String,
    scale: f32
}

#[derive(Serialize, Deserialize, PartialEq)]
enum TextureUsage {
    ShaderResource,
    RenderTarget,
    DepthStencil,
    UnorderedAccess,
    VideoDecodeTarget
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
    usage: Vec<TextureUsage>
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
fn to_gfx_texture_info(pmfx_texture: &TextureInfo) -> gfx::TextureInfo {
    // size from ratio
    let (width, height) = if let Some(_) = pmfx_texture.ratio {
        (1280, 720)
    }
    else {
        (pmfx_texture.width, pmfx_texture.height)
    };

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
    let initial_state = if pmfx_texture.usage.contains(&TextureUsage::ShaderResource) {
        gfx::ResourceState::ShaderResource
    }
    else if pmfx_texture.usage.contains(&TextureUsage::DepthStencil) {
        gfx::ResourceState::DepthStencil
    }
    else if pmfx_texture.usage.contains(&TextureUsage::RenderTarget) {
        gfx::ResourceState::RenderTarget
    }
    else {
        gfx::ResourceState::ShaderResource
    };

    // texture type bitflags from vec of enum
    let mut usage = gfx::TextureUsage::NONE;
    for pmfx_usage in &pmfx_texture.usage {
        match pmfx_usage {
            TextureUsage::ShaderResource => {
                usage |= gfx::TextureUsage::SHADER_RESOURCE
            }
            TextureUsage::UnorderedAccess => {
                usage |= gfx::TextureUsage::UNORDERED_ACCESS
            }
            TextureUsage::RenderTarget => {
                usage |= gfx::TextureUsage::RENDER_TARGET
            }
            TextureUsage::DepthStencil => {
                usage |= gfx::TextureUsage::DEPTH_STENCIL
            }
            TextureUsage::VideoDecodeTarget => {
                usage |= gfx::TextureUsage::VIDEO_DECODE_TARGET
            }
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
            tracked_textures: Vec::new(),
            views: HashMap::new()
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

    /// Creates a texture if it has not already been created from information specified in .pmfx file
    pub fn create_texture(&mut self, device: &mut D, texture_name: &str) -> Result<(), super::Error> {
        if !self.textures.contains_key(texture_name) && self.pmfx.textures.contains_key(texture_name) {
            // create texture from info specified in .pmfx file
            println!("hotline_rs::pmfx:: creating texture: {}", texture_name);
            let tex = device.create_texture::<u8>(&to_gfx_texture_info(&self.pmfx.textures[texture_name]), None)?;
            self.textures.insert(texture_name.to_string(), tex);
            // track textures which rely on ratio sizes
            if self.pmfx.textures[texture_name].ratio.is_some() {
                println!("hotline_rs::pmfx:: tracking texture: {}", texture_name);
                self.tracked_textures.push(texture_name.to_string())
            }
        }
        Ok(())
    }

    /// Returns a texture reference if the texture exists or none otherwise
    pub fn get_texture<'stack>(&'stack self, texture_name: &str) -> Option<&'stack D::Texture> {
        if self.textures.contains_key(texture_name) {
            Some(&self.textures[texture_name])
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
            }

            // create textures for depth stencils
            for name in &pmfx_view.depth_stencil {
                self.create_texture(device, name)?;
            }

            // array of targets by name
            for name in &pmfx_view.render_target {
                render_targets.push(self.get_texture(name).unwrap());
            }

            // get depth stencil by name
            let depth_stencil = if pmfx_view.depth_stencil.len() > 0 {
                let name = &pmfx_view.depth_stencil[0];
                Some(self.get_texture(name).unwrap())
            }
            else {
                None
            };

            // pass for render targets with depth stencil
            let render_target_pass = device
            .create_render_pass(&gfx::RenderPassInfo {
                render_targets: render_targets,
                rt_clear: to_gfx_clear_colour(pmfx_view.clear_colour),
                depth_stencil: depth_stencil,
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
                    width: 1280.0,
                    height: 720.0,
                    min_depth: 0.0,
                    max_depth: 1.0
                },
                scissor_rect: gfx::ScissorRect {
                    left: 0,
                    top: 0,
                    right: 1280,
                    bottom: 720
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
        self.reset(swap_chain)
    }

    /// Reset all command buffers
    pub fn reset(&mut self, swap_chain: &D::SwapChain) {
        for (_, view) in &self.views {
            let view = view.clone();
            view.lock().unwrap().cmd_buf.reset(swap_chain);
        }
    }

    /// Execute command buffers in order
    pub fn execute(
        &mut self,
        device: &mut D) {
        // execute views. this can become more strictly ordered later
        for (_, view) in &self.views {
            let view = view.clone();
            let view = &mut view.lock().unwrap();
            view.cmd_buf.close().unwrap();
            device.execute(&view.cmd_buf);
        }
    }
}

