#![allow(clippy::collapsible_if)] 

use crate::gfx::PipelineStatistics;
use crate::os;
use crate::gfx;

use crate::gfx::{ResourceState, RenderPass, CmdBuf, Subresource, QueryHeap, SwapChain, Texture};
use crate::reloader::{ReloadState, Reloader, ReloadResponder};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use maths_rs::{max, min, num::Base};

/// Hash type for quick checks of changed resources from pmfx
pub type PmfxHash = u64;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Everything you need to render a world view; command buffers will be automatically reset and submitted for you.
pub struct View<D: gfx::Device> {
    /// Name of the graph view instance, this is the same as the key that is stored in the pmfx `views` map.
    pub graph_view_name: String,
    /// Name of the pmfx view, this is the source view (camera, render targets)
    pub pmfx_view_name: String,
    /// Hash of the view name
    pub name_hash: PmfxHash,
    /// Colour hash (for debug markers, derived from name)
    pub colour_hash: u32,
    /// A pre-built render pass: multiple colour targets and depth possible
    pub pass: D::RenderPass,
    /// Pre-calculated viewport based on the output dimensions of the render target adjusted for user data from .pmfx
    pub viewport: gfx::Viewport,
    /// Pre-calculated viewport based on the output dimensions of the render target adjusted for user data from .pmfx
    pub scissor_rect: gfx::ScissorRect,
    /// A command buffer ready to be used to buffer draw / render commands
    pub cmd_buf: D::CmdBuf,
    /// Name of camera this view intends to be used with
    pub camera: String,
    /// This is the name of a single pipeline used for all draw calls in the view. supplied in data as `pipelines: ["name"]`
    pub view_pipeline: String,
}
pub type ViewRef<D> = Arc<Mutex<View<D>>>;

/// Compact mesh representation referincing and index buffer, vertex buffer and num index count
#[derive(Clone)]
pub struct Mesh<D: gfx::Device> {
    /// Vertex buffer
    pub vb: D::Buffer,
    // Index Buffer
    pub ib: D::Buffer,
    /// Number of indices to draw from the index buffer
    pub num_indices: u32
}

/// Additional info to wrap with a texture for tracking changes from windwow sizes or other associated bounds
struct TrackedTexture<D: gfx::Device>  {
    /// The texture itself
    texture: D::Texture,
    /// Optional ratio, which will contain window name and scale info if present
    ratio: Option<TextureSizeRatio>,
    /// Tuple of (width, height) to track the current size of the texture and compare for updates
    size: (u64, u64)
}

/// Information to track changes to 
struct PmfxTrackingInfo {
    /// Filepath to the data which the pmfx File was deserialised from
    filepath: std::path::PathBuf,
    /// Modified time of the .pmfx file this instance is associated with
    modified_time: SystemTime,
}

// pipelines (name) > permutation (mask : u32) which is tuple (build_hash, pipeline)
type FormatPipelineMap<T> = HashMap<String, HashMap<u32, (PmfxHash, T)>>;

// hash of the view in .0, the view itself in .1 the source view name which was used to generate the instance is stored in .2, 
type TrackedView<D> = (PmfxHash, Arc<Mutex<View<D>>>, String);

/// Pmfx instance,containing render objects and resources
pub struct Pmfx<D: gfx::Device> {
    /// Serialisation structure of a .pmfx file containing render states, pipelines and textures
    pmfx: File,
    /// Tracking info for check on data reloads, grouped by pmfx name
    pmfx_tracking: HashMap<String, PmfxTrackingInfo>,
    /// Folder paths for 
    pmfx_folders: HashMap<String, String>, 
    /// Updated by calling 'update_window' this will cause any tracked textures to check for resizes and rebuild textures if necessary
    window_sizes: HashMap<String, (f32, f32)>,
    /// Nested structure of: format (u64) > FormatPipelineMap
    render_pipelines: HashMap<PmfxHash, FormatPipelineMap<D::RenderPipeline>>,
    /// Compute Pipelines grouped by name then as a tuple (build_hash, pipeline)
    compute_pipelines: HashMap<String, (PmfxHash, D::ComputePipeline)>,
    /// Shaders stored along with their build hash for quick checks if reload is necessary
    shaders: HashMap<String, (PmfxHash, D::Shader)>,
    /// Texture map of tracked texture info
    textures: HashMap<String, (PmfxHash, TrackedTexture<D>)>,
    /// Built views that are used in view function dispatches, the source view name which was used to generate the instnace is stored in .2 for hash checking
    views: HashMap<String, TrackedView<D>>,
    /// View timing and GPU pipeline statistics
    view_stats: HashMap<String, ViewStats<D>>,
    /// Map of camera constants that can be retrieved by name for use as push constants
    cameras: HashMap<String, CameraConstants>,
    /// Auto-generated barriers to insert between view passes to ensure correct resource states
    barriers: HashMap<String, D::CmdBuf>,
    /// Vector of view names to execute in designated order
    command_queue: Vec<String>,
    /// Tracking texture references of views
    view_texture_refs: HashMap<String, HashSet<String>>,
    /// Container to hold overall GPU stats
    total_stats: TotalStats,
    /// Watches for filestamp changes and will trigger callbacks in the `PmfxReloadResponder`
    pub reloader: Reloader,
    /// Errors which occur through render systems can be pushed here for feedback to the user
    pub view_errors: Arc<Mutex<HashMap<String, String>>>,
    /// Tracks the currently active render graph name
    pub active_render_graph: String,
}

/// Contains frame statistics from the GPU for all pmfx jobs
pub struct TotalStats {
    /// Total GPU time spent in milliseconds
    pub gpu_time_ms: f64,
    /// Time of the first submission in seconds
    pub gpu_start: f64,
    /// Time of the final submission in seconds
    pub gpu_end: f64,
    /// Total pipeline statistics 
    pub pipeline_stats: PipelineStatistics
}

impl TotalStats {
    fn new() -> Self {
        Self {
            gpu_time_ms: 0.0,
            gpu_start: 0.0,
            gpu_end: 0.0,
            pipeline_stats: PipelineStatistics::default()
        }
    }
}

/// Resources to track and read back GPU-statistics for individual views
struct ViewStats<D: gfx::Device> {
    write_index: usize,
    read_index: usize,
    frame_fence_value: u64,
    fences: Vec<u64>,
    timestamp_heap: D::QueryHeap,
    timestamp_buffers: Vec<[D::Buffer; 2]>,
    pipeline_stats_heap: D::QueryHeap,
    pipeline_stats_buffers: Vec<D::Buffer>,
    pipeline_query_index: usize,
    start_timestamp: f64,
    end_timestamp: f64
}

impl<D> ViewStats<D> where D: gfx::Device {
    pub fn new_query_buffer(device: &mut D, elem_size: usize, num_elems: usize) -> D::Buffer {
        device.create_read_back_buffer(elem_size * num_elems).unwrap()
    }
    
    pub fn new(device: &mut D, num_buffers: usize) -> Self {
        let mut timestamp_buffers = Vec::new();
        let mut fences = Vec::new();
        let mut pipeline_stats_buffers = Vec::new();
        let timestamp_size_bytes = D::get_timestamp_size_bytes();
        let pipeline_statistics_size_bytes = D::get_pipeline_statistics_size_bytes();
        for _ in 0..num_buffers {
            timestamp_buffers.push([
                Self::new_query_buffer(device, timestamp_size_bytes, 1),
                Self::new_query_buffer(device, timestamp_size_bytes, 1)
            ]);
            pipeline_stats_buffers.push(
                Self::new_query_buffer(device, pipeline_statistics_size_bytes, 1),
            );
            fences.push(0)
        }
        Self {
            frame_fence_value: 0,
            write_index: 0,
            read_index: 0,
            fences,
            start_timestamp: 0.0,
            end_timestamp: 0.0,
            timestamp_heap: device.create_query_heap(&gfx::QueryHeapInfo {
                heap_type: gfx::QueryType::Timestamp,
                num_queries: 2,
            }),
            timestamp_buffers,
            pipeline_stats_heap: device.create_query_heap(&gfx::QueryHeapInfo {
                heap_type: gfx::QueryType::PipelineStatistics,
                num_queries: 1,
            }),
            pipeline_stats_buffers,
            pipeline_query_index: usize::max_value()
        }
    }
}

/// Serialisation layout for contents inside .pmfx file
#[derive(Serialize, Deserialize)]
struct File {
    shaders: HashMap<String, PmfxHash>,
    pipelines: HashMap<String, PipelinePermutations>,
    depth_stencil_states: HashMap<String, gfx::DepthStencilInfo>,
    raster_states: HashMap<String, gfx::RasterInfo>,
    blend_states: HashMap<String, BlendInfo>,
    render_target_blend_states: HashMap<String, gfx::RenderTargetBlendInfo>,
    textures: HashMap<String, TextureInfo>,
    views: HashMap<String, ViewInfo>,
    render_graphs: HashMap<String, HashMap<String, GraphViewInfo>>,
    dependencies: Vec<String>
}

/// pmfx File serialisation, 
impl File {
    /// creates a new empty pmfx
    fn new() -> Self {
        File {
            shaders: HashMap::new(),
            pipelines: HashMap::new(),
            depth_stencil_states: HashMap::new(),
            raster_states: HashMap::new(),
            blend_states: HashMap::new(),
            render_target_blend_states: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            render_graphs: HashMap::new(),
            dependencies: Vec::new(),
        }
    }
}

/// Data to associate a Texture with a Window so when a window resizes we updat the texture dimensions to window size * scale
#[derive(Serialize, Deserialize, Clone)]
struct TextureSizeRatio {
    /// Window name to track size changes from
    window: String,
    /// Multiply the window dimensions * scale to get the final size of the texture
    scale: f32
}

/// Pmfx texture serialisation layout, this data is emitted from pmfx-shader compiler
#[derive(Serialize, Deserialize)]
struct TextureInfo {
    ratio: Option<TextureSizeRatio>,
    filepath: Option<String>,
    width: u64,
    height: u64,
    depth: u32,
    mip_levels: u32,
    array_layers: u32,
    samples: u32,
    format: gfx::Format,
    usage: Vec<ResourceState>,
    hash: u64
}

/// Pmfx texture serialisation layout, this data is emitted from pmfx-shader compiler
#[derive(Serialize, Deserialize)]
struct BlendInfo {
    alpha_to_coverage_enabled: bool,
    independent_blend_enabled: bool,
    render_target: Vec<String>,
}

/// Pmfx pipeline serialisation layout, this data is emitted from pmfx-shader compiler
#[derive(Serialize, Deserialize, Clone)]
struct Pipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    vertex_layout: Option<gfx::InputLayout>,
    descriptor_layout: gfx::DescriptorLayout,
    depth_stencil_state: Option<String>,
    raster_state: Option<String>,
    blend_state: Option<String>,
    topology: gfx::Topology,
    sample_mask: u32,
    hash: PmfxHash
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
    clear_stencil: Option<u8>,
    camera: String,
    hash: PmfxHash
}

#[derive(Serialize, Deserialize, Clone)]
struct GraphViewInfo {
    view: String,
    pipelines: Option<Vec<String>>,
    function: String,
    depends_on: Option<Vec<String>>,
}

#[repr(C)]
#[derive(Clone)]
pub struct CameraConstants {
    pub view_matrix: maths_rs::Mat4f,
    pub view_projection_matrix: maths_rs::Mat4f,
    pub view_position: maths_rs::Vec4f
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
fn info_from_state<T: Default + Clone>(name: &Option<String>, map: &HashMap<String, T>) -> Result<T, super::Error> {
    if let Some(name) = &name {
        if map.contains_key(name) {
            Ok(map[name].clone())
        }
        else {
            Err(
                super::Error {
                    msg: format!("hotline::pmfx:: missing render state in pmfx config `{}`", name)
                }
            )
        }
    }
    else {
        Ok(T::default())
    }
}

/// get a gfx::BlendState from the pmfx description which unpacks blend states by name and then
/// array of render target blend states by name
fn blend_info_from_state(
    name: &Option<String>,
    blend_states: &HashMap<String, BlendInfo>,
    render_target_blend_states: &HashMap<String, gfx::RenderTargetBlendInfo>) -> Result<gfx::BlendInfo, super::Error> {
    if let Some(name) = &name {
        if blend_states.contains_key(name) {
            let mut rtinfo = Vec::new();
            for name in &blend_states[name].render_target {
                rtinfo.push(info_from_state(&Some(name.to_string()), render_target_blend_states)?);
            }
            Ok(gfx::BlendInfo {
                alpha_to_coverage_enabled: blend_states[name].alpha_to_coverage_enabled,
                independent_blend_enabled: blend_states[name].independent_blend_enabled,
                render_target: rtinfo
            })
        }
        else {
            Err(
                super::Error {
                    msg: format!("hotline::pmfx:: missing blend state in pmfx config `{}`", name)
                }
            )
        }
    }
    else {
        Ok(gfx::BlendInfo {
            alpha_to_coverage_enabled: false,
            independent_blend_enabled: false,
            render_target: vec![gfx::RenderTargetBlendInfo::default()],
        })
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
        array_layers: pmfx_texture.array_layers,
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
            pmfx_tracking: HashMap::new(),
            pmfx_folders: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            shaders: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            view_stats: HashMap::new(),
            cameras: HashMap::new(),
            barriers: HashMap::new(),
            command_queue: Vec::new(),
            view_texture_refs: HashMap::new(),
            window_sizes: HashMap::new(),
            active_render_graph: String::new(),
            view_errors: Arc::new(Mutex::new(HashMap::new())),
            reloader: Reloader::create(Box::new(PmfxReloadResponder::new())),
            total_stats: TotalStats::new()
        }
    }

    /// Load a pmfx from a folder, where the folder contains a pmfx info.json and shader binaries in separate files within the directory
    /// You can load multiple pmfx files which will be merged together, shaders are grouped by pmfx_name/ps_main.psc
    /// Render graphs and pipleines must have unique names, if multiple pmfx name a pipeline the same name  
    pub fn load(&mut self, filepath: &str) -> Result<(), super::Error> {        
        // get the name for indexing by pmfx name/folder
        let folder = Path::new(filepath);
        let pmfx_name = if let Some(name) = folder.file_name() {
            String::from(name.to_os_string().to_str().unwrap())
        }
        else {
            String::from(filepath)
        };

        // check if we are already loaded
        if let std::collections::hash_map::Entry::Vacant(e) = self.pmfx_tracking.entry(pmfx_name.to_string()) {
             println!("hotline_rs::pmfx:: loading: {}", pmfx_name);
             //  deserialise pmfx pipelines from file
             let info_filepath = folder.join(format!("{}.json", pmfx_name));
             let pmfx_data = fs::read(&info_filepath)?;
             let file : File = serde_json::from_slice(&pmfx_data)?;
 
             // create tracking info to check if the pmfx has been rebuilt
             let file_metadata = fs::metadata(&info_filepath)?;
             e.insert(PmfxTrackingInfo {
                 modified_time: file_metadata.modified()?,
                 filepath: info_filepath
             });
 
             // add files from pmfx for tracking
             for dep in &file.dependencies {
                 self.reloader.add_file(dep);
             }
 
             // merge into pmfx
             self.merge_pmfx(file, filepath);
         }

        Ok(())
    }

    /// Merges the pmfx file `other` in the current `Pmfx` instance
    fn merge_pmfx(&mut self, other: File, other_filepath: &str) {
        // prepend the pmfx name to the shaders so we can avoid collisions
        for name in other.pipelines.keys() {
            if !self.pmfx_folders.contains_key(name) {
                // insert lookup path for shaders as they go into a folder: pmfx/shaders.vsc
                self.pmfx_folders.insert(name.to_string(), String::from(other_filepath));
            }
        }
        // extend the maps
        self.pmfx.shaders.extend(other.shaders);
        self.pmfx.pipelines.extend(other.pipelines);
        self.pmfx.depth_stencil_states.extend(other.depth_stencil_states);
        self.pmfx.raster_states.extend(other.raster_states);
        self.pmfx.blend_states.extend(other.blend_states);
        self.pmfx.render_target_blend_states.extend(other.render_target_blend_states);
        self.pmfx.textures.extend(other.textures);
        self.pmfx.views.extend(other.views);
        self.pmfx.render_graphs.extend(other.render_graphs);
        self.pmfx.dependencies.extend(other.dependencies);
    }

    /// Removes items from currently loaded maps (states, pipelines etc), if they do not exist in expected keys (in data).
    /*
    fn remove_stale_from_map<T, U>(loaded_map: &mut HashMap<String, T>, expected_keys: &HashMap<String, U>) {
        let keys = loaded_map.keys().map(|s| s.to_string()).collect::<Vec<String>>();
        for item in &keys {
            if !expected_keys.contains_key(item) {
                loaded_map.remove(item);
            }
        }
    }

    /// Removes stale states, views and pipelines which no longer exist in data
    fn remove_stale(&mut self) {
        Self::remove_stale_from_map(&mut self.views, &self.pmfx.views);
        Self::remove_stale_from_map(&mut self.view_stats, &self.pmfx.views);
        Self::remove_stale_from_map(&mut self.textures, &self.pmfx.textures);
        Self::remove_stale_from_map(&mut self.shaders, &self.pmfx.shaders);
        Self::remove_stale_from_map(&mut self.compute_pipelines, &self.pmfx.pipelines);

        // render pipelines are a bit more compliated beause of pass formats
        let formats = self.render_pipelines.keys().map(|h| *h).collect::<Vec<PmfxHash>>();
        for format in &formats {
            if let Some(pipelines) = self.render_pipelines.get_mut(format) {
                Self::remove_stale_from_map(pipelines, &self.pmfx.pipelines);
            }
        }
    }
    */

    /// Internal utility which will create a shader from file or `None` if no file is passed, or the shader does not exist
    fn create_shader(&mut self, device: &D, folder: &Path, file: &Option<String>) -> Result<(), super::Error> {
        let folder = folder.parent().unwrap();
        if let Some(file) = file {
            if !self.shaders.contains_key(file) {
                println!("hotline_rs::pmfx:: compiling shader: {}", file);
                let shader = create_shader_from_file(device, folder, Some(file.to_string()))?;
                if let Some(shader) = shader {
                    println!("hotline_rs::pmfx:: success: {}", file);
                    let hash = self.pmfx.shaders.get(file).unwrap();
                    self.shaders.insert(file.to_string(), (*hash, shader));
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
                Some(&self.shaders[file].1)
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
                let samples = pmfx_texture.samples as f32;
                // clamp to samples x samples so if we want 0 size we still have a valid texture
                let size = (max(size.0, samples), max(size.1, samples));
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
            self.textures.insert(texture_name.to_string(), (pmfx_tex.hash, TrackedTexture {
                texture: tex,
                ratio: self.pmfx.textures[texture_name].ratio.clone(),
                size
            }));
        }
        Ok(())
    }

    /// Returns a texture reference if the texture exists or none otherwise
    pub fn get_texture<'stack>(&'stack self, texture_name: &str) -> Option<&'stack D::Texture> {
        if self.textures.contains_key(texture_name) {
            Some(&self.textures[texture_name].1.texture)
        }
        else {
            None
        }
    }

    /// Returns the tuple (width, height) of a texture
    pub fn get_texture_2d_size(&self, texture_name: &str) -> Option<(u64, u64)> {
        if self.textures.contains_key(texture_name) {
            Some(self.textures[texture_name].1.size)
        }
        else {
            None
        }
    }

    /// Create a view from information specified in pmfx file
    fn create_view(&mut self, device: &mut D, view_name: &str, graph_view_name: &str, info: &GraphViewInfo) -> Result<(), super::Error> {
        if !self.views.contains_key(graph_view_name) && self.pmfx.views.contains_key(view_name) {

            println!("hotline_rs::pmfx:: creating graph view: {} for {}", graph_view_name, view_name);

            // create pass from targets
            let pmfx_view = self.pmfx.views[view_name].clone();

            // create textures for targets
            let mut render_targets = Vec::new();
            for name in &pmfx_view.render_target {
                self.create_texture(device, name)?;
                self.view_texture_refs.entry(name.to_string())
                    .or_insert(HashSet::new()).insert(graph_view_name.to_string());
            }

            // create textures for depth stencils
            for name in &pmfx_view.depth_stencil {
                self.create_texture(device, name)?;

                self.view_texture_refs.entry(name.to_string())
                .or_insert(HashSet::new()).insert(graph_view_name.to_string());
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
            })?;

            // assing a view pipleine (if we supply 1 pipeline) for all draw calls in the view, otherwise leave it emptu
            let view_pipeline = if let Some(pipelines) = &info.pipelines {
                if pipelines.len() == 1 {
                    pipelines[0].to_string()
                }
                else {
                    String::new()
                }
            }
            else {
                String::new()
            };

            // hashes
            let mut hash = DefaultHasher::new();
            graph_view_name.hash(&mut hash);
            let name_hash : PmfxHash = hash.finish();

            // colour hash
            let mut hash = DefaultHasher::new();
            view_name.hash(&mut hash);
            let colour_hash : u32 = hash.finish() as u32 | 0xff000000;

            // validate viewport f32 count
            if pmfx_view.viewport.len() != 6 {
                return Err(super::Error {
                    msg: format!("hotline_rs::pmfx:: viewport expects array of 6 floats, found {}", pmfx_view.viewport.len())
                });
            }

            // validate scissor f32 count
            if pmfx_view.scissor.len() != 4 {
                return Err(super::Error {
                    msg: format!("hotline_rs::pmfx:: scissor expects array of 4 floats, found {}", pmfx_view.viewport.len())
                });
            }

            let view = View::<D> {
                graph_view_name: graph_view_name.to_string(),
                pmfx_view_name: view_name.to_string(),
                name_hash,
                colour_hash,
                pass: render_target_pass,
                viewport: gfx::Viewport {
                    x: size.0 as f32 * pmfx_view.viewport[0],
                    y: size.1 as f32 * pmfx_view.viewport[1],
                    width: size.0 as f32 * pmfx_view.viewport[2],
                    height: size.1 as f32 * pmfx_view.viewport[3],
                    min_depth: pmfx_view.viewport[4],
                    max_depth: pmfx_view.viewport[5],
                },
                scissor_rect: gfx::ScissorRect {
                    left: (size.0 as f32 * pmfx_view.scissor[0]) as i32,
                    top: (size.1 as f32 * pmfx_view.scissor[1]) as i32,
                    right: (size.0 as f32 * pmfx_view.scissor[2]) as i32,
                    bottom: (size.1 as f32 * pmfx_view.scissor[3]) as i32
                },
                cmd_buf: device.create_cmd_buf(2),
                camera: pmfx_view.camera.to_string(),
                view_pipeline
            };

            self.views.insert(graph_view_name.to_string(), 
                (pmfx_view.hash, Arc::new(Mutex::new(view)), view_name.to_string()));

            // create stats
            self.view_stats.insert(graph_view_name.to_string(), ViewStats::new(device, 2));
        }

        Ok(())
    }

    /// Return a reference to a view if the view exists or none otherwise
    pub fn get_view(&self, view_name: &str) -> Result<ViewRef<D>, super::Error> {
        if self.views.contains_key(view_name) {
            Ok(self.views[view_name].1.clone())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: view: {} not found", view_name)
            })
        }
    }

    /// Create all views required for a render graph if necessary, skip if a view already exists
    pub fn create_render_graph_views(&mut self, device: &mut D, graph_name: &str) -> Result<(), super::Error> {
        // create views for all of the nodes
        if self.pmfx.render_graphs.contains_key(graph_name) {
            let pmfx_graph = self.pmfx.render_graphs[graph_name].clone();
            for (graph_view_name, node) in &pmfx_graph {
                // create view for each node
                self.create_view(device, &node.view, graph_view_name, node)?;
            }
        }
        Ok(())
    }

    fn create_resolve_transition(
        &mut self,
        device: &mut D,
        texture_barriers: &mut HashMap<String, ResourceState>, 
        view_name: &str, 
        texture_name: &str, 
        target_state: ResourceState) -> Result<(), super::Error> {
        if texture_barriers.contains_key(texture_name) {
            let state = texture_barriers[texture_name];
            let barrier_name = format!("barrier_resolve-{}-{}", view_name, texture_name);
            if let Some(tex) = self.get_texture(texture_name) {
                // prevent resolving non msaa surfaces
                if !tex.is_resolvable() {
                    return Err(super::Error {
                        msg: format!("hotline_rs::pmfx:: texture: {} is not resolvable", texture_name),
                    });
                }

                // transition main resource into resolve src
                let mut cmd_buf = device.create_cmd_buf(1);
                cmd_buf.begin_event(0xffdc789a, &format!("resolve: {}", &texture_name));

                cmd_buf.transition_barrier(&gfx::TransitionBarrier {
                    texture: Some(self.get_texture(texture_name).unwrap()),
                    buffer: None,
                    state_before: state,
                    state_after: ResourceState::ResolveSrc,
                });

                // transition resolve resource into resolve dst
                cmd_buf.transition_barrier_subresource(&gfx::TransitionBarrier {
                        texture: Some(self.get_texture(texture_name).unwrap()),
                        buffer: None,
                        state_before: target_state,
                        state_after: ResourceState::ResolveDst,
                    },
                    Subresource::ResolveResource
                );
                
                // perform the resolve
                cmd_buf.resolve_texture_subresource(tex, 0)?;

                // transition the resolve to shader resource for sampling
                cmd_buf.transition_barrier_subresource(&gfx::TransitionBarrier {
                        texture: Some(self.get_texture(texture_name).unwrap()),
                        buffer: None,
                        state_before: ResourceState::ResolveDst,
                        state_after: target_state,
                    },
                    Subresource::ResolveResource
                );
                cmd_buf.end_event();

                // insert barrier
                cmd_buf.close()?;
                self.barriers.insert(barrier_name.to_string(), cmd_buf);

                // update track state
                texture_barriers.remove(texture_name);
                texture_barriers.insert(texture_name.to_string(), ResourceState::ResolveSrc);

                
            }

            // add barrier placeholder in the execute order
            self.command_queue.push(barrier_name);
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
                self.command_queue.push(barrier_name.to_string());          

                // create a command buffer
                let mut cmd_buf = device.create_cmd_buf(1);
                cmd_buf.begin_event(
                    0xfff1b023, 
                    &format!("transition_barrier: {} ({} -> {})", &texture_name, state, target_state)
                );
                cmd_buf.transition_barrier(&gfx::TransitionBarrier {
                    texture: Some(self.get_texture(texture_name).unwrap()),
                    buffer: None,
                    state_before: state,
                    state_after: target_state,
                });
                cmd_buf.end_event();
                cmd_buf.close()?;
                self.barriers.insert(barrier_name, cmd_buf);
    
                // update track state
                texture_barriers.remove(texture_name    );
                texture_barriers.insert(texture_name.to_string(), target_state);
            }
        }
        Ok(())
    }

    /// Unloads all views, so that a subsequent call to `create_render_graph` wiill build from clean
    /// make sure call this after `SwapChain::wait_for_last_fame()` so any dropped resources will
    /// not be in use on the GPU
    pub fn unload_views(&mut self) {
        self.views.clear();
    }

    /// Create a render graph wih automatic resource barrier generation from info specified insie .pmfx file
    pub fn create_render_graph(&mut self, device: &mut D, graph_name: &str) -> Result<(), super::Error> {        
        // go through the graph sequentially, as the command lists are executed in order but generated 
        if self.pmfx.render_graphs.contains_key(graph_name) {

            // create views for any nodes in the graph
            self.create_render_graph_views(device, graph_name)?;

            // currently we just have 1 single execute graph and barrier set
            self.barriers.clear();
            self.command_queue.clear();

            let mut barriers = self.pmfx.textures.iter().filter(|tex|{
                tex.1.usage.contains(&ResourceState::ShaderResource) || 
                tex.1.usage.contains(&ResourceState::RenderTarget) ||
                tex.1.usage.contains(&ResourceState::DepthStencil)
            }).map(|tex|{
              (tex.0.to_string(), ResourceState::ShaderResource)  
            }).collect::<HashMap<String, ResourceState>>();

            // loop over the graph multiple times adding views in depends on order, until we add all the views
            let mut to_add = self.pmfx.render_graphs[graph_name].len();
           
            let mut added = 0;
            let mut dependencies = HashSet::new();
            while added < to_add {
                let pmfx_graph = self.pmfx.render_graphs[graph_name].clone();
                for (graph_view_name, instance) in &pmfx_graph {
                    // allow missing views to be safely handled
                    if !self.pmfx.views.contains_key(&instance.view) {
                        println!("hotline_rs::pmfx:: [warning] missing view {}", instance.view);
                        to_add -= 1;
                        continue;
                    }
    
                    // already added this view
                    if dependencies.contains(graph_view_name) {
                        continue;
                    }
    
                    // wait for dependencies
                    if let Some(depends_on) = &instance.depends_on {
                        let mut passes = false;
                        if !depends_on.is_empty() {
                            for d in depends_on {
                                if !pmfx_graph.contains_key(d) {
                                    passes = true;
                                    println!("hotline_rs::pmfx:: [warning] view {} missing dependency {}. ignoring", 
                                        instance.view, d);
                                }
                                else if dependencies.contains(d) {
                                    passes = true;
                                }
                                else {
                                    passes = false;
                                }
                            }
                        }

                        if !passes {
                            continue;
                        }
                    }
                    
                    // create transitions by inspecting view info
                    let pmfx_view = self.pmfx.views[&instance.view].clone();
    
                    // if we need to write to a target we must make sure it is transitioned into render target state
                    for rt_name in pmfx_view.render_target {
                        self.create_texture_transition_barrier(
                            device, &mut barriers, &instance.view, &rt_name, ResourceState::RenderTarget)?;
    
                    }
    
                    // same for depth stencils
                    for ds_name in pmfx_view.depth_stencil {
                        self.create_texture_transition_barrier(
                            device, &mut barriers, &instance.view, &ds_name, ResourceState::DepthStencil)?;
    
                    }
    
                    // create pipelines requested for this view instance with the pass format
                    if let Some(view_pipelines) = &instance.pipelines {
                        for pipeline in view_pipelines {
                            let view = self.get_view(graph_view_name)?;
                            let view = view.clone();
                            let view = view.lock().unwrap();
                            self.create_pipeline(device, pipeline, &view.pass)?;
                        }
                    }
    
                    // push a view on
                    added += 1;
                    dependencies.insert(graph_view_name.to_string());
                    self.command_queue.push(graph_view_name.to_string());
                }
            }
            
            // finally all targets which are in the 'barriers' array are transitioned to shader resources (for debug views)
            let srvs = barriers.keys().map(|k|{
                k.to_string()
            }).collect::<Vec<String>>();

            for name in srvs {
                let result = self.create_resolve_transition(
                    device, &mut barriers, "eof", &name, ResourceState::ShaderResource);
               
                if result.is_err() {
                    // TODO: tell user without spewing out errors
                }

                self.create_texture_transition_barrier(
                    device, &mut barriers, "eof", &name, ResourceState::ShaderResource)?;
            }

            // track the current render graph for if we need to rebuild due to resize, or file modification
            self.active_render_graph = graph_name.to_string();

            Ok(())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find render graph: {}", graph_name),
            })
        }
    }

    /// Create a RenderPipeline instance for the combination of pmfx_pipeline settings and an associated RenderPass
    pub fn create_pipeline(&mut self, device: &D, pipeline_name: &str, pass: &D::RenderPass) -> Result<(), super::Error> {              
        if self.pmfx.pipelines.contains_key(pipeline_name) {
            // first create shaders if necessary
            let folder = self.pmfx_folders.get(pipeline_name)
                .expect(&format!("hotline_rs::pmfx:: expected to find pipeline {} in pmfx_folders", pipeline_name)).to_string();
            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {
                self.create_shader(device, Path::new(&folder), &pipeline.vs)?;
                self.create_shader(device, Path::new(&folder), &pipeline.ps)?;
                self.create_shader(device, Path::new(&folder), &pipeline.cs)?;
            }
            
            // create entry for this format if it does not exist
            let fmt = pass.get_format_hash();
            let format_pipeline = self.render_pipelines.entry(fmt).or_insert(HashMap::new());
            
            // create entry for this pipeline permutation set if it does not exist
            if !format_pipeline.contains_key(pipeline_name) {
                println!("hotline_rs::pmfx:: creating pipeline: {}", pipeline_name);
                format_pipeline.insert(pipeline_name.to_string(), HashMap::new());
                // we create a pipeline per-permutation
                for (permutation, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {    
                    // TODO: infer compute or graphics pipeline from pmfx
                    let cs = self.get_shader(&pipeline.cs);
                    if let Some(cs) = cs {
                        let pso = device.create_compute_pipeline(&gfx::ComputePipelineInfo {
                            cs,
                            descriptor_layout: pipeline.descriptor_layout.clone(),
                        })?;
                        println!("hotline_rs::pmfx:: compiled compute pipeline: {}", pipeline_name);
                        self.compute_pipelines.insert(pipeline_name.to_string(), (pipeline.hash, pso));
                    }
                    else {
                        let vertex_layout = pipeline.vertex_layout.as_ref().unwrap();
                        let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
                            vs: self.get_shader(&pipeline.vs),
                            fs: self.get_shader(&pipeline.ps),
                            input_layout: vertex_layout.to_vec(),
                            descriptor_layout: pipeline.descriptor_layout.clone(),
                            raster_info: info_from_state(&pipeline.raster_state, &self.pmfx.raster_states)?,
                            depth_stencil_info: info_from_state(&pipeline.depth_stencil_state, &self.pmfx.depth_stencil_states)?,
                            blend_info: blend_info_from_state(
                                &pipeline.blend_state, &self.pmfx.blend_states, &self.pmfx.render_target_blend_states)?,
                            topology: pipeline.topology,
                            sample_mask:pipeline.sample_mask,
                            pass: Some(pass),
                            ..Default::default()
                        })?;
                        
                        println!("hotline_rs::pmfx:: compiled render pipeline: {}", pipeline_name);
                        let format_pipeline = self.render_pipelines.get_mut(&fmt).unwrap();
                        let permutations = format_pipeline.get_mut(pipeline_name).unwrap();  

                        let mask = permutation.parse().unwrap();
                        permutations.insert(mask, (pipeline.hash, pso));
                    }
                }
            }

            Ok(())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find pipeline: {}", pipeline_name),
            })
        }
    }

    /// Returns a pmfx defined pipeline compatible with the supplied format hash if it exists
    pub fn get_render_pipeline_for_format<'stack>(&'stack self, pipeline_name: &str, format_hash: u64) -> Result<&'stack D::RenderPipeline, super::Error> {
        self.get_render_pipeline_permutation_for_format(pipeline_name, 0, format_hash)
    }

    /// Returns a pmfx defined pipeline compatible with the supplied format hash if it exists
    pub fn get_render_pipeline_permutation_for_format<'stack>(&'stack self, pipeline_name: &str, permutation: u32, format_hash: u64) -> Result<&'stack D::RenderPipeline, super::Error> {
        if let Some(formats) = &self.render_pipelines.get(&format_hash) {
            if formats.contains_key(pipeline_name) {
                Ok(&formats[pipeline_name][&permutation].1)
            }
            else {
                Err(super::Error {
                    msg: format!("hotline_rs::pmfx:: could not find pipeline for format: {} ({})", pipeline_name, format_hash),
                })
            }
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find pipeline: {}", pipeline_name),
            })
        }
    }

    /// Fetch a prebuilt ComputePipeline
    pub fn get_compute_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::ComputePipeline> {
        if self.compute_pipelines.contains_key(pipeline_name) {
            Some(&self.compute_pipelines[pipeline_name].1)
        }
        else {
            None
        }
    }

    /// Obtain stats for the current frame and caulcuate time deltas between start / end
    fn gather_stats(&mut self, device: &mut D, swap_chain: &D::SwapChain) {
        let mut min_frame_timestamp = f64::max_value();
        let mut max_frame_timestamp = f64::zero();
        let mut total_pipeline_stats = PipelineStatistics::default();
        let timestamp_size_bytes = D::get_timestamp_size_bytes();
        for (name, stats) in &mut self.view_stats {
            if self.command_queue.contains(name) {
                stats.frame_fence_value = swap_chain.get_frame_fence_value();
                let i = stats.read_index;
                let write_fence = stats.fences[i];
                if write_fence < swap_chain.get_frame_fence_value() {
                    // start timestamp
                    let timestamps = device.read_timestamps(
                        swap_chain, &stats.timestamp_buffers[i][0], timestamp_size_bytes, write_fence);
                    if !timestamps.is_empty() {
                        stats.start_timestamp = timestamps[0];
                        min_frame_timestamp = min(stats.start_timestamp, min_frame_timestamp);
    
                    }
                    // end timestamp
                    let timestamps = device.read_timestamps(
                        swap_chain, &stats.timestamp_buffers[i][1], timestamp_size_bytes, write_fence);
                    if !timestamps.is_empty() {
                        stats.end_timestamp = timestamps[0];
                        max_frame_timestamp = max(stats.end_timestamp, max_frame_timestamp);
                    }
                    // pipeline stats
                    let pipeline_stats = device.read_pipeline_statistics(
                        swap_chain, &stats.pipeline_stats_buffers[i], write_fence);
                    if let Some(pipeline_stats) = pipeline_stats {
                        total_pipeline_stats += pipeline_stats;
                    }
                }
            }
        }
        self.total_stats.gpu_start = min_frame_timestamp;
        self.total_stats.gpu_end = max_frame_timestamp;
        self.total_stats.gpu_time_ms = (max_frame_timestamp - min_frame_timestamp) * 1000.0;
        self.total_stats.pipeline_stats = total_pipeline_stats;
    }

    /// Start a new frame and syncronise command buffers to the designated swap chain
    pub fn new_frame(&mut self, device: &mut D, swap_chain: &D::SwapChain) -> Result<(), super::Error> {
        // check if we have any reloads available
        if self.reloader.check_for_reload() == ReloadState::Available {
            // wait for last GPU frame so we can drop the resources
            swap_chain.wait_for_last_frame();
            self.reload(device)?;
            self.reloader.complete_reload();
        }

        // gather render stats
        self.gather_stats(device, swap_chain);

        // reset command buffers
        self.reset(swap_chain);

        Ok(())
    }

    /// Reload all active resources based on hashes
    pub fn reload(&mut self, device: &mut D) -> Result<(), super::Error> {        
        let reload_paths = self.pmfx_tracking.iter_mut().filter(|(_, tracking)| {
            fs::metadata(&tracking.filepath).unwrap().modified().unwrap() > tracking.modified_time
        }).map(|tracking| {
            tracking.1.filepath.to_string_lossy().to_string()
        }).collect::<Vec<String>>();

        let mut rebuild_graph = false;
        for reload_filepath in reload_paths {
            if !reload_filepath.is_empty() {
                println!("hotline_rs::pmfx:: reload from {}", reload_filepath);
                let pmfx_data = fs::read(&reload_filepath).expect("hotline_rs::pmfx:: failed to read file");
                
                let file : File = serde_json::from_slice(&pmfx_data)?;
                self.merge_pmfx(file, PathBuf::from(&reload_filepath).parent().unwrap().to_str().unwrap());

                // remove stale states
                // self.remove_stale();

                // find textures that need reloading
                let reload_textures = self.textures.iter().filter(|(k, v)| {
                    self.pmfx.textures.get(*k).map_or_else(|| false, |src| {
                        src.hash != v.0
                    })
                }).map(|(k, _)| {
                    k.to_string()
                }).collect::<HashSet<String>>();

                // Get views to reload from changed textures
                let reload_texture_views = reload_textures.iter().fold(HashSet::new(), |mut v, t|{
                    v.extend(self.get_view_texture_refs(t));
                    v
                });

                // Find views that have changed by hash
                let mut reload_views = Vec::new();
                for (name, view) in &self.views {
                    if self.pmfx.views.contains_key(&view.2) &&
                    (self.pmfx.views.get(&view.2).unwrap().hash != view.0 || reload_texture_views.contains(name)) {
                        reload_views.push((view.2.to_string(), name.to_string()));
                    }
                }

                // find pipelines that need reloading
                let mut reload_pipelines = Vec::new();
                for (hash, formats) in &self.render_pipelines {
                    for (name, permutations) in formats {
                        for (mask, pipeline) in permutations {

                            let build_hash = self.pmfx.pipelines
                                .get(name).unwrap()
                                .get(&mask.to_string()).unwrap()
                                .hash;

                            if pipeline.0 != build_hash {
                                reload_pipelines.push((*hash, name.to_string(), *mask));
                            }
                        }
                    }
                }

                // find shaders that need reloading
                let mut reload_shaders = Vec::new();
                for (name, shader) in &self.shaders {
                    if self.pmfx.shaders.contains_key(name) {
                        if *self.pmfx.shaders.get(name).unwrap() != shader.0 {
                            reload_shaders.push(name.to_string());
                        }
                    }
                }

                // reload textures
                self.recreate_textures(device, &reload_textures)?;

                // reload views
                for view in &reload_views {
                    println!("hotline::pmfx:: reloading view: {}", view.1);
                    self.views.remove(&view.1);
                    self.view_stats.remove(&view.1);
                    rebuild_graph = true;
                }

                // reload shaders
                for shader in &reload_shaders {
                    println!("hotline::pmfx:: reloading shader: {}", shader);
                    self.shaders.remove(shader);
                }
                
                // reload pipelines tuple = (format_hash, pipeline_name, permutation_mask)
                for pipeline in &reload_pipelines {
                    println!("hotline::pmfx:: reloading pipeline: {}", pipeline.1);
                    
                    // TODO: here we could only remove affected permutations
                    let format_pipelines = self.render_pipelines.get_mut(&pipeline.0).unwrap();
                    format_pipelines.remove(&pipeline.1);

                    // find first with the same format
                    let compatiblew_view = self.views.iter().find(|(_, view)| {
                        let pass = &view.1.lock().unwrap().pass;
                        pass.get_format_hash() == pipeline.0
                    }).map(|v| v.0);

                    // create pipeline with the pass from compatible view
                    if let Some(compatiblew_view) = compatiblew_view {
                        let view = self.get_view(compatiblew_view)?.clone();
                        let view = view.lock().unwrap();
                        self.create_pipeline(device, &pipeline.1, &view.pass)?;
                    }
                    else {
                        println!("hotline::pmfx:: warning pipeline was not reloaded: {}", pipeline.1);
                    }
                }

                // update the timestamp on the tracking info
                self.pmfx_tracking.get_mut(&reload_filepath).map(|t| {
                    t.modified_time = SystemTime::now();
                    t
                });
            }

            // 
            if rebuild_graph {
                self.create_render_graph(device, &self.active_render_graph.to_string())?;
            }
        }
        Ok(())
    }

    /// Recreate the textures in `texture_names` call this when you know size / sample count has changed
    /// and the tracking info is updated
    fn recreate_textures(&mut self, device: &mut D, texture_names: &HashSet<String>) -> Result<(), super::Error> {
        for texture_name in texture_names {
            // remove the old and destroy
            let tex = self.textures.remove(texture_name);
            if let Some(tex) = tex {
                device.destroy_texture(tex.1.texture);
                // create with new dimensions from 'window_sizes'
                self.create_texture(device, texture_name)?;
            }
        }
        Ok(())
    }

    /// Returns `Vec<String>` containing view names associated with the texture name
    fn get_view_texture_refs(&self, texture_name: &str) -> HashSet<String> {
        if self.view_texture_refs.contains_key(texture_name) {
            self.view_texture_refs[texture_name].clone()
        }
        else {
            HashSet::new()
        }
    }

    pub fn get_window_size(&self, window_name: &str) -> (f32, f32) {
        if self.window_sizes.contains_key(window_name) {
            self.window_sizes[window_name]
        }
        else {
            (0.0, 0.0)
        }
    }

    pub fn get_window_aspect(&self, window_name: &str) -> f32 {
        if self.window_sizes.contains_key(window_name) {
            let size = self.window_sizes[window_name];
            size.0 / size.1
        }
        else {
            0.0
        }
    }

    /// Update render targets or views associated with a window, this will resize textures and rebuild views
    /// which need to be modified if a window size changes
    pub fn update_window(&mut self, device: &mut D, size: (f32, f32), name: &str) -> Result<(), super::Error> {
        let mut rebuild_views = HashSet::new();
        let mut recreate_texture_names = HashSet::new();
        if self.window_sizes.contains_key(name) {
            if self.window_sizes[name] != size {
                // update tracked textures
                for (texture_name, texture) in &self.textures {
                    if let Some(ratio) = &texture.1.ratio {
                        if ratio.window == name {
                            recreate_texture_names.insert(texture_name.to_string());
                            if self.view_texture_refs.contains_key(texture_name) {
                                for view_name in &self.view_texture_refs[texture_name] {
                                    self.views.remove(view_name);
                                    self.view_stats.remove(view_name);
                                    rebuild_views.insert(view_name.to_string());
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
        self.recreate_textures(device, &recreate_texture_names)?;

        // recreate the active render graph
        if !rebuild_views.is_empty() {
            if !self.active_render_graph.is_empty() {
                self.create_render_graph(device, &self.active_render_graph.to_string())?;
            }
        }

        Ok(())
    }

    /// Update camera constants for the named camera, will create a new entry if one does not exist
    pub fn update_camera_constants(&mut self, name: &str, constants: &CameraConstants) {
        *self.cameras.entry(name.to_string()).or_insert(constants.clone()) = constants.clone();
    }

    /// Borrow camera constants to push into a command buffer, return `None` if they do not exist
    pub fn get_camera_constants(&self, name: &str) -> Result<&CameraConstants, super::Error> {
        if let Some(cam) = &self.cameras.get(name) {
            Ok(cam)
        }
        else {
            Err(super::Error {
                msg: format!("hotline::pmfx:: could not find camera {}", name)
            })
        }
    }

    fn stats_start(view: &mut View<D>, view_stats: &mut ViewStats<D>) {
        // sync to the frame
        view_stats.fences[view_stats.write_index] = view_stats.frame_fence_value;

        // view timestamps
        view_stats.timestamp_heap.reset();
        let buf = &mut view_stats.timestamp_buffers[view_stats.write_index][0];
        view.cmd_buf.timestamp_query(&mut view_stats.timestamp_heap, buf);

        // view pipeline stats
        view_stats.pipeline_stats_heap.reset();
        view_stats.pipeline_query_index = view.cmd_buf.begin_query(
            &mut view_stats.pipeline_stats_heap, 
            gfx::QueryType::PipelineStatistics
        );
    }

    fn stats_end(view: &mut View<D>, view_stats: &mut ViewStats<D>) {
        // end timestamp
        let buf = &mut view_stats.timestamp_buffers[view_stats.write_index][1];
        view.cmd_buf.timestamp_query(&mut view_stats.timestamp_heap, buf);

        // end pipeline stats query
        if view_stats.pipeline_query_index != usize::max_value() {
            let buf = &mut view_stats.pipeline_stats_buffers[view_stats.write_index];
            view.cmd_buf.end_query(
                &mut view_stats.pipeline_stats_heap, 
                gfx::QueryType::PipelineStatistics,
                view_stats.pipeline_query_index,
                buf,
            );
            view_stats.pipeline_query_index = usize::max_value();
        }
    }

    /// Resets all command buffers, this assumes they have been used and need to be reset for the next frame
    pub fn reset(&mut self, swap_chain: &D::SwapChain) {
        for (name, view) in &self.views {
            // rest only command buffers that are in use
            if self.command_queue.contains(name) {
                let view = view.clone();
                let mut view = view.1.lock().unwrap();
                view.cmd_buf.reset(swap_chain);

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.view_stats.remove(name).unwrap();
                Self::stats_start(&mut view, &mut stats);
                self.view_stats.insert(name.to_string(), stats);
            }
        }
    }

    /// Returns a vector of information to call render functions. It returns a tuple (function_name, view_name)
    /// which is called as so: `function_name(view)` so functions can be re-used for different views
    pub fn get_render_graph_function_info(&self, render_graph: &str) -> Vec<(String, String)> {
        if self.pmfx.render_graphs.contains_key(render_graph) {
            self.pmfx.render_graphs[render_graph].iter().map(|graph|{
                (graph.1.function.to_string(), graph.0.to_string())
            }).collect()
        }
        else {
            Vec::new()
        }
    }

    /// Returns the build hash for the render graph so you can compare if the graph has rebuilt and needs reloading
    pub fn get_render_graph_hash(&self, render_graph: &str) -> PmfxHash {
        // this could be calculated at build time
        if self.pmfx.render_graphs.contains_key(render_graph) {
            self.pmfx.render_graphs[render_graph].keys().fold(DefaultHasher::new(), |mut hasher, name|{
                name.hash(&mut hasher);
                hasher
            }).finish()
        }
        else {
            0
        }
    }

    pub fn get_render_graph_execute_order(&self) -> &Vec<String> {
        &self.command_queue
    }

    /// Execute command buffers in order
    pub fn execute(
        &mut self,
        device: &mut D) {
        for node in &self.command_queue {
            if self.barriers.contains_key(node) {
                // transition barriers
                device.execute(&self.barriers[node]);
            }
            else if self.views.contains_key(node) {
                // dispatch a view
                let view = self.views[node].clone();
                let view = &mut view.1.lock().unwrap();

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.view_stats.remove(node).unwrap();
                Self::stats_end(view, &mut stats);
                self.view_stats.insert(node.to_string(), stats);

                view.cmd_buf.close().unwrap();
                device.execute(&view.cmd_buf);
            }
        }
    }

    /// Log an error with an assosiated view and message.
    pub fn log_error(&self, view_name: &str, msg: &str) {
        let mut errors = self.view_errors.lock().unwrap();
        errors.entry(view_name.to_string()).or_insert(msg.to_string());
    }

    /// Return the total statistics for the previous frame
    pub fn get_total_stats(&self) -> &TotalStats {
        &self.total_stats
    }
}

use crate::imgui;
impl<D, A> imgui::UserInterface<D, A> for Pmfx<D> where D: gfx::Device, A: os::App {
    fn show_ui(&mut self, imgui: &mut imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("textures", &mut imgui_open, imgui::WindowFlags::NONE) {
                for texture in self.textures.values() {
                    
                    let thumb_size = 256.0;
                    let aspect = texture.1.size.0 as f32 / texture.1.size.1 as f32;
                    let w = thumb_size * aspect;
                    let h = thumb_size;

                    imgui.image(&texture.1.texture, w, h);

                    imgui.same_line();
                    imgui.spacing();
                    imgui.same_line();
                }
            }
            imgui.end();

            if imgui.begin("pmfx", &mut imgui_open, imgui::WindowFlags::NONE) {
                imgui.text("Shaders");
                imgui.separator();
                for shader in self.pmfx.shaders.keys() {
                    imgui.text(shader);
                }
                imgui.separator();

                imgui.text("Pipelines");
                imgui.separator();
                for pipeline in self.pmfx.pipelines.keys() {
                    imgui.text(pipeline);
                }
                imgui.separator();

                imgui.text("Render Graphs");
                imgui.separator();
                for graph in self.pmfx.render_graphs.keys() {
                    imgui.text(graph);
                }
                imgui.separator();

                imgui.text("Cameras");
                imgui.separator();
                for camera in self.cameras.keys() {
                    imgui.text(camera);
                }
                imgui.separator();
            }
            imgui.end();

            if imgui.begin("perf", &mut imgui_open, imgui::WindowFlags::NONE) {
                imgui.text(&format!("gpu: {:.2} (ms)", self.total_stats.gpu_time_ms));
                imgui.separator();
                imgui.text("pipeline statistics");
                imgui.separator();
                imgui.text(&format!("input_assembler_vertices: {}", self.total_stats.pipeline_stats.input_assembler_vertices));
                imgui.text(&format!("input_assembler_primitives: {}", self.total_stats.pipeline_stats.input_assembler_primitives));
                imgui.text(&format!("vertex_shader_invocations: {}", self.total_stats.pipeline_stats.vertex_shader_invocations));
                imgui.text(&format!("pixel_shader_primitives: {}", self.total_stats.pipeline_stats.pixel_shader_primitives));
                imgui.text(&format!("compute_shader_invocations: {}", self.total_stats.pipeline_stats.compute_shader_invocations));
            }
            imgui.end();

            imgui_open
        } 
        else {
            false
        }
    }
}

struct PmfxReloadResponder {
    files: Vec<String>,
    start_time: SystemTime
}

impl PmfxReloadResponder {
    fn new() -> Self {
        PmfxReloadResponder {
            files: Vec::new(),
            start_time: SystemTime::now()
        }
    }
}

impl ReloadResponder for PmfxReloadResponder {
    fn add_file(&mut self, filepath: &str) {
        self.files.push(filepath.to_string());
    }  

    fn get_files(&self) -> Vec<String> {
        self.files.to_vec()
    }

    fn get_last_mtime(&self) -> SystemTime {
        self.start_time
    }

    fn build(&mut self) -> std::process::ExitStatus {
        let hotline_path = super::get_data_path("../..");
        let pmbuild = super::get_data_path("../../hotline-data/pmbuild.cmd");
        let output = std::process::Command::new(pmbuild)
            .current_dir(hotline_path)
            .arg("win32-data")
            .arg("-pmfx")
            .output()
            .expect("hotline::hot_lib:: hot pmfx failed to compile!");

        if !output.stdout.is_empty() {
            println!("{}", String::from_utf8(output.stdout).unwrap());
        }

        if !output.stderr.is_empty() {
            println!("{}", String::from_utf8(output.stderr).unwrap());
        }

        if output.status.success() {
            self.start_time = SystemTime::now();
        }

        output.status
    }
}