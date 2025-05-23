#![allow(clippy::collapsible_if)] 

use crate::gfx::Buffer;
use crate::gfx::PipelineStatistics;
use crate::gfx::RaytracingPipelineInfo;

use crate::os;
use crate::gfx;
use crate::primitives;
use crate::image;

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

use maths_rs::prelude::*;

/// Hash type for quick checks of changed resources from pmfx
pub type PmfxHash = u64;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// To lookup resources in a shader, these are passed to compute shaders:
/// index = srv (read), uav (write)
/// dimension is the resource dimension where 2d textures will be (w, h, 1) and 3d will be (w, h, d)
#[repr(C)]
pub struct ResourceUse {
    pub index: u32,
    pub dimension: Vec3u
} 

/// Everything you need to render a world view; command buffers will be automatically reset and submitted for you.
pub struct View<D: gfx::Device> {
    /// Name of the graph view instance, this is the same as the key that is stored in the pmfx `views` map.
    pub graph_pass_name: String,
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
    /// Dimension of a resource for use when blitting
    pub blit_dimension: Vec2f,
    /// A command buffer ready to be used to buffer draw / render commands
    pub cmd_buf: D::CmdBuf,
    /// Name of camera this view intends to be used with
    pub camera: String,
    /// This is the name of a single pipeline used for all draw calls in the view. supplied in data as `pipelines: ["name"]`
    pub view_pipeline: String,
    // A vector of resource view indices supplied by info inside the `uses` section
    // they will be supplied in order they are specified in the `pmfx` file
    // and may be srv or uav depending on `ResourceUsage`
    pub use_indices: Vec<ResourceUse>,
}
pub type ViewRef<D> = Arc<Mutex<View<D>>>;

/// Equivalent to a `View` this is a graph node which only requires compute
pub struct ComputePass<D: gfx::Device> {
    /// A command buffer ready to be used to buffer compute cmmands
    pub cmd_buf: D::CmdBuf,
    /// The name of a single pipeline used for this compute pass
    pub pass_pipline: String,
    /// Hash of the view name
    pub name_hash: PmfxHash,
    /// Colour hash (for debug markers, derived from name)
    pub colour_hash: u32,
    /// The number of threads specified in the shader
    pub numthreads: gfx::Size3,
    /// We can calulcate this based on resource dimension / thread count
    pub group_count: gfx::Size3,
    // An vector of resource view indices supplied by info inside the `uses` section
    // they will be supplied in order they are specified in the `pmfx` file
    // and may be srv or uav depending on `ResourceUsage`
    pub use_indices: Vec<ResourceUse>
}
pub type ComputePassRef<D> = Arc<Mutex<ComputePass<D>>>;

/// Compact mesh representation referincing and index buffer, vertex buffer and num index count
#[derive(Clone)]
pub struct Mesh<D: gfx::Device> {
    /// Vertex buffer
    pub vb: D::Buffer,
    // Index Buffer
    pub ib: D::Buffer,
    /// Number of indices to draw from the index buffer
    pub num_indices: u32,
    /// Bounding aabb min
    pub aabb_min: Vec3f,
    /// Bounding aabb mix
    pub aabb_max: Vec3f,
}

/// Additional info to wrap with a texture for tracking changes from windwow sizes or other associated bounds
struct TrackedTexture<D: gfx::Device>  {
    /// The texture itself
    texture: D::Texture,
    /// Optional ratio, which will contain window name and scale info if present
    ratio: Option<TextureSizeRatio>,
    /// Tuple of (width, height, depth) to track the current size of the texture and compare for updates
    size: (u64, u64, u32),
    /// Track texture type
    _tex_type: gfx::TextureType,
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
type TrackedComputePass<D> = (PmfxHash, Arc<Mutex<ComputePass<D>>>);

pub struct RaytracingPipelineBinding<D: gfx::Device> {
    pub pipeline: D::RaytracingPipeline,
    pub sbt: D::RaytracingShaderBindingTable,
}

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
    /// Raytracing pipeline and sbt binding pair
    raytracing_pipelines: HashMap<String, (PmfxHash, RaytracingPipelineBinding<D>)>,
    /// Shaders stored along with their build hash for quick checks if reload is necessary
    shaders: HashMap<String, (PmfxHash, D::Shader)>,
    /// Texture map of tracked texture info
    textures: HashMap<String, (PmfxHash, TrackedTexture<D>)>,
    /// Built views that are used in view function dispatches, the source view name which was used to generate the instnace is stored in .2 for hash checking
    views: HashMap<String, TrackedView<D>>,
    // Built compute passes that contain a command buffer and other compute dispatch info
    compute_passes: HashMap<String, TrackedComputePass<D>>,
    /// Pass timing and GPU pipeline statistics
    pass_stats: HashMap<String, PassStats<D>>,
    /// Map of camera constants that can be retrieved by name for use as push constants
    cameras: HashMap<String, CameraConstants>,
    /// Comtainer to hold world data for use on the GPU
    world_buffers: DynamicWorldBuffers<D>,
    /// Auto-generated barriers to insert between view passes to ensure correct resource states
    barriers: HashMap<String, D::CmdBuf>,
    /// Vector of view names to execute in designated order
    command_queue: Vec<String>,
    /// Tracking texture references of views
    view_texture_refs: HashMap<String, HashSet<String>>,
    /// Container to hold overall GPU stats
    total_stats: TotalStats,
    /// Heaps for shader resource view allocations
    pub shader_heap: D::Heap,
    /// Unit quad mesh for fullscreen passes on the raster pipeline
    pub unit_quad_mesh: Mesh<D>,
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
struct PassStats<D: gfx::Device> {
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

impl<D> PassStats<D> where D: gfx::Device {
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
    render_graphs: HashMap<String, HashMap<String, GraphPassInfo>>,
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
    generate_mips: Option<bool>,
    filepath: Option<String>,
    src_data: Option<bool>,
    width: u64,
    height: u64,
    depth: u32,
    mip_levels: u32,
    array_layers: u32,
    samples: u32,
    cubemap: bool,
    format: gfx::Format,
    usage: Vec<ResourceState>,
    hash: u64,
}

/// Pmfx texture serialisation layout, this data is emitted from pmfx-shader compiler
#[derive(Serialize, Deserialize)]
struct BlendInfo {
    alpha_to_coverage_enabled: bool,
    independent_blend_enabled: bool,
    render_target: Vec<String>,
}

/// Information to fille out gfx::RaytracingShaderBindingTableInfo. It's mostly the same but doesnt have the pipleine requirement and need for typed Device
#[derive(Serialize, Deserialize, Clone)]
struct RaytracingShaderBindingTableInfo {
    ray_generation_shader: String,
    #[serde(default)]
    miss_shaders: Vec<String>,
    #[serde(default)]
    hit_groups: Vec<String>,
    #[serde(default)]
    callable_shaders: Vec<String>
}

/// Pmfx pipeline serialisation layout, this data is emitted from pmfx-shader compiler
#[derive(Serialize, Deserialize, Clone)]
struct Pipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    lib: Option<Vec<String>>,
    hit_groups: Option<Vec<gfx::RaytracingHitGroup>>,
    sbt: Option<RaytracingShaderBindingTableInfo>,
    numthreads: Option<(u32, u32, u32)>,
    vertex_layout: Option<gfx::InputLayout>,
    pipeline_layout: gfx::PipelineLayout,
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

/// Resoure uage for a graph pass
#[derive(Serialize, Deserialize, Clone, Debug)]
enum ResourceUsage {
    /// Write to an un-ordeded access resource or rneder target resource
    Write,
    /// Read from the primary (resovled) resource
    Read,
    /// Read from an MSAA resource
    ReadMsaa,
    /// Read from resource and signal we want to read generated mip maps
    ReadMips
}

#[derive(Serialize, Deserialize, Clone)]
struct GraphPassInfo {
    /// For render passes, specifies the view (render target, camera etc
    view: Option<String>,
    /// Pipelines array that will use during this pass
    pipelines: Option<Vec<String>>,
    /// A function to call which can build draw or compute commands
    function: String,
    /// Dependency info for determining execute order
    depends_on: Option<Vec<String>>,
    /// Array of resources we wish to use during this pass, which can be passed to a shader (to know the srv indices)
    uses: Option<Vec<(String, ResourceUsage)>>,
    /// For compute passes the number of threads
    numthreads: Option<(u32, u32, u32)>,
    /// The name of a resource a compute shader wil distrubute work into
    target_dimension: Option<String>,
    /// Signify we want cubemap rendering
    cubemap: Option<bool>
}

/// A GPU buffer type which can resize and stretch like a vector
pub struct DynamicBuffer<D: gfx::Device, T: Sized> {
    len: usize,
    capacity: usize,
    buffers: Option<Vec<D::Buffer>>,
    usage: gfx::BufferUsage,
    bb: usize,
    num_buffers: usize,
    resource_type: std::marker::PhantomData<T>
}

impl<D, T> DynamicBuffer<D, T> where D: gfx::Device, T: Sized {
    /// creates a new empty buffer, you need to `reserve` space afterwards
    pub fn new(usage: gfx::BufferUsage, num_buffers: usize) -> Self {
        DynamicBuffer {
            len: 0,
            capacity: 0,
            buffers: None,
            usage,
            bb: 0,
            num_buffers,
            resource_type: std::marker::PhantomData
        }
    }

    /// Swap buffers once a frame for safe CPU writes an GPU in flight reads
    pub fn swap(&mut self) {
        self.bb = (self.bb + 1) % self.num_buffers
    }

    /// Access the internal buffer to write to this frame, for safe CPU writes an GPU in flight reads
    pub fn mut_buf(&mut self) -> &mut D::Buffer {
        &mut self.buffers.as_mut().unwrap()[self.bb]
    }

    /// Access immutable buffer to us in the current frame
    pub fn buf(&mut self) -> &D::Buffer {
        &self.buffers.as_ref().unwrap()[self.bb]
    }

    /// creates a new buffer if more cpaacity is required
    pub fn reserve(&mut self, device: &mut D, heap: &mut D::Heap, capacity: usize) {
        if capacity > self.capacity {
            let mut inner_buffers = Vec::new();
            for _ in 0..self.num_buffers {
                let buf = device.create_buffer_with_heap(&gfx::BufferInfo{
                    usage: self.usage,
                    cpu_access: gfx::CpuAccessFlags::WRITE | gfx::CpuAccessFlags::PERSISTENTLY_MAPPED,
                    format: gfx::Format::Unknown,
                    stride: std::mem::size_of::<T>(),
                    num_elements: capacity,
                    initial_state: if self.usage.contains(gfx::BufferUsage::CONSTANT_BUFFER) {
                        gfx::ResourceState::VertexConstantBuffer
                    }
                    else {
                        gfx::ResourceState::ShaderResource
                    }
                }, crate::data![], heap).unwrap();
                inner_buffers.push(buf);
            }

            self.capacity = capacity;
            self.buffers = Some(inner_buffers);
        }
    }

    /// Resets length to zero
    pub fn clear(&mut self) {
        self.len = 0
    }

    /// Returns the length of the data written to the buffer in elements
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the len is 0
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the capacity of the buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Write arbitrary data data to the buffer and update len, it should be either a u8 slice, a single &T or a slice of T
    pub fn write<T2: Sized>(&mut self, offset: usize, data: &[T2]) {
        self.mut_buf().write(
            offset,
            data
        ).unwrap();
        let write_offset = offset + data.len() * std::mem::size_of::<T2>();
        self.len = max(write_offset / std::mem::size_of::<T>(), self.len);
    }

    /// Push an item of type `T` to the dynamic buffer and update the len
    pub fn push(&mut self, item: &T) {
        let write_offset = self.len * std::mem::size_of::<T>();
        self.mut_buf().write(
            write_offset,
            gfx::as_u8_slice(item)
        ).unwrap();
        self.len += 1;
    }

    /// get's the appropriate resource index
    fn get_index(&self) -> usize {
        if let Some(buf) = &self.buffers {
            if self.usage.contains(gfx::BufferUsage::CONSTANT_BUFFER) {
                buf[self.bb].get_cbv_index().unwrap()
            }
            else {
                buf[self.bb].get_srv_index().unwrap()
            }
        }
        else {
            0
        }
    }

    pub fn get_lookup(&self) -> GpuBufferLookup {
        GpuBufferLookup {
            index: self.get_index() as u32,
            count: self.len as u32
        }
    }
}

pub struct DynamicWorldBuffers<D: gfx::Device> {
    /// Structured buffer containing bindless draw call information `DrawData`
    pub draw: DynamicBuffer<D, DrawData>,
    /// Structured buffer containing bindless draw call information `DrawData`
    pub extent: DynamicBuffer<D, ExtentData>,
    // Structured buffer containing `MaterialData`
    pub material: DynamicBuffer<D, MaterialData>,
    // Structured buffer containing `PointLightData`
    pub point_light: DynamicBuffer<D, PointLightData>,
    // Structured buffer containing `SpotLightData`
    pub spot_light: DynamicBuffer<D, SpotLightData>,
    // Structured buffer containing `DirectionalLightData`
    pub directional_light: DynamicBuffer<D, DirectionalLightData>,
    /// Structured buffer for shadow map matrices
    pub shadow_matrix: DynamicBuffer<D, Mat4f>,
    /// Constant buffer containing camera info
    pub camera: DynamicBuffer<D, CameraData>,
}

impl<D> Default for DynamicWorldBuffers<D> where D: gfx::Device {
    fn default() -> Self {
        Self {
            draw: DynamicBuffer::<D, DrawData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            extent: DynamicBuffer::<D, ExtentData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            material: DynamicBuffer::<D, MaterialData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            point_light: DynamicBuffer::<D, PointLightData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            spot_light: DynamicBuffer::<D, SpotLightData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            directional_light: DynamicBuffer::<D, DirectionalLightData>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
            camera: DynamicBuffer::<D, CameraData>::new(gfx::BufferUsage::CONSTANT_BUFFER, 3),
            shadow_matrix: DynamicBuffer::<D, Mat4f>::new(gfx::BufferUsage::SHADER_RESOURCE, 3),
        }
    }
}

/// Information to cerate `WorldBuffers` for rendering
#[derive(Default)]
pub struct WorldBufferReserveInfo {
    pub draw_capacity: usize,
    pub extent_capacity: usize,
    pub material_capacity: usize,
    pub point_light_capacity: usize,
    pub spot_light_capacity: usize,
    pub directional_light_capacity: usize,
    pub camera_capacity: usize,
    pub shadow_matrix_capacity: usize
}

/// GPU friendly structure containing camera view information
#[repr(C)]
#[derive(Clone)]
pub struct CameraConstants {
    pub view_matrix: maths_rs::Mat4f,
    pub view_projection_matrix: maths_rs::Mat4f,
    pub view_position: maths_rs::Vec4f
}

/// GPU friendly struct containing single entity draw data
#[repr(C)]
#[derive(Clone)]
pub struct DrawData {
    /// World matrix for transforming entity
    pub world_matrix: Mat34f, 
}

/// GPU friendly struct containing single entity draw data
#[repr(C)]
#[derive(Clone)]
pub struct ExtentData {
    /// Centre pos of aabb
    pub pos: Vec3f,
    /// Half extent of aabb
    pub extent: Vec3f 
}

/// GPU friendly structure containing lookup id's for bindless materials 
#[repr(C)]
#[derive(Clone)]
pub struct MaterialData {
    pub albedo_id: u32,
    pub normal_id: u32,
    pub roughness_id: u32,
    pub padding: u32
}

/// GPU lookup for shadow map srv and matrix index. to look into textures and shadows matrix in world buffers
#[repr(packed)]
#[derive(Clone, Copy, Default)]
pub struct ShadowMapInfo {
    pub srv_index: u32,
    pub matrix_index: u32
}

/// GPU friendly structure for point lights
#[repr(C)]
#[derive(Clone)]
pub struct PointLightData {
    pub pos: Vec3f,
    pub radius: f32,
    pub colour: Vec4f,
    pub shadow_map_info: ShadowMapInfo
}

/// GPU friendly structure for directional lights
#[repr(C)]
#[derive(Clone)]
pub struct DirectionalLightData {
    pub dir: Vec3f,
    pub colour: Vec4f,
    pub shadow_map_info: ShadowMapInfo
}

/// GPU friendly structure for spot lights
#[repr(C)]
#[derive(Clone)]
pub struct SpotLightData {
    pub pos: Vec3f,
    pub cutoff: f32,
    pub dir: Vec3f,
    pub falloff: f32,
    pub colour: Vec4f,
    pub shadow_map_info: ShadowMapInfo
}

/// GPU friendly structure for cameras
#[repr(C)]
#[derive(Clone)]
pub struct CameraData {
    pub view_projection_matrix: Mat4f,
    pub view_position: Vec4f,
    pub planes: [Vec4f; 6],
}

#[repr(C)]
#[derive(Clone, Default, Debug)]
pub struct GpuBufferLookup {
    /// index of the srv or cbv
    pub index: u32,
    /// number of elements in the buffer
    pub count: u32
}

#[repr(C)]
#[derive(Clone, Default, Debug)]
pub struct WorldBufferInfo {
    /// srv index of the draw buffer (contains entity world matrices and draw call data)
    pub draw: GpuBufferLookup,
    /// srv index of the extent buffer (contains data for culling entities)
    pub extent: GpuBufferLookup,
    /// srv index of the material buffer (contains ids of textures to look up and material parameters)
    pub material: GpuBufferLookup,
    /// srv index of the point light buffer
    pub point_light: GpuBufferLookup,
    /// srv index of the spot light buffer
    pub spot_light: GpuBufferLookup,
    /// srv index of the directional light buffer
    pub directional_light: GpuBufferLookup,
    /// cbv index of the camera
    pub camera: GpuBufferLookup,
    /// srv index of shadow matrices
    pub shadow_matrix: GpuBufferLookup
}

pub fn cubemap_camera_face(face: usize, pos: Vec3f, near: f32, far: f32) -> CameraConstants {
    let at = [
        vec3f(1.0, 0.0, 0.0),   //+x
        vec3f(-1.0, 0.0, 0.0),  //-x
        vec3f(0.0, 1.0, 0.0),   //+y
        vec3f(0.0, -1.0, 0.0),  //-y        
        vec3f(0.0, 0.0, 1.0),   //+z
        vec3f(0.0, 0.0, -1.0)   //-z
    ];

    let right = [
        vec3f(0.0, 0.0, -1.0),
        vec3f(0.0, 0.0, 1.0),
        vec3f(1.0, 0.0, 0.0),
        vec3f(1.0, 0.0, 0.0),
        vec3f(1.0, 0.0, 0.0),
        vec3f(-1.0, 0.0, -0.0)
    ];

    let up = [
        vec3f(0.0, 1.0, 0.0),
        vec3f(0.0, 1.0, 0.0),
        vec3f(0.0, 0.0, -1.0),
        vec3f(0.0, 0.0, 1.0),
        vec3f(0.0, 1.0, 0.0),
        vec3f(0.0, 1.0, 0.0)
    ];

    let view = Mat4f::from((
        Vec4f::from((right[face as usize], pos.x)),
        Vec4f::from((up[face as usize], pos.y)),
        Vec4f::from((at[face as usize], pos.z)),
        Vec4f::new(0.0, 0.0, 0.0, 1.0),
    ));

    let proj = Mat4f::create_perspective_projection_lh_yup(deg_to_rad(90.0), 1.0, near, far);

    let view = view.inverse();
    CameraConstants {
        view_matrix: view,
        view_projection_matrix: proj * view,
        view_position: Vec4f::from((pos, 1.0))
    }
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
    let tex_type = if pmfx_texture.cubemap { 
        gfx::TextureType::TextureCube
    } 
    else if pmfx_texture.depth > 1 {
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

    let mut mip_levels = pmfx_texture.mip_levels;
    if let Some(mips) = pmfx_texture.generate_mips {
        if mips {
            usage |= gfx::TextureUsage::GENERATE_MIP_MAPS;
            mip_levels = gfx::mip_levels_for_dimension(width, height);
        }
    }

    gfx::TextureInfo {
        width,
        height,
        tex_type,
        initial_state,
        usage,
        mip_levels,
        depth: pmfx_texture.depth,
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

fn get_shader_entry_point_name(shader_name: Option<String>) -> Option<String> {
    if let Some(shader_name) = shader_name {
        Path::new(&shader_name)
            .file_stem()
            .and_then(|os_str| os_str.to_str())
            .map(|s| s.to_string())
    }
    else {
        None
    }
}

impl<D> Pmfx<D> where D: gfx::Device {
    /// Create a new empty pmfx instance
    pub fn create(device: &mut D, shader_heap_size: usize) -> Self {
        // create a heap which pmfx can manage itself
        let shader_heap = device.create_heap(&gfx::HeapInfo {
            heap_type: gfx::HeapType::Shader,
            num_descriptors: shader_heap_size,
        });
        Pmfx {
            pmfx: File::new(),
            pmfx_tracking: HashMap::new(),
            pmfx_folders: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            raytracing_pipelines: HashMap::new(),
            shaders: HashMap::new(),
            textures: HashMap::new(),
            views: HashMap::new(),
            compute_passes: HashMap::new(),
            pass_stats: HashMap::new(),
            cameras: HashMap::new(),
            barriers: HashMap::new(),
            command_queue: Vec::new(),
            view_texture_refs: HashMap::new(),
            window_sizes: HashMap::new(),
            active_render_graph: String::new(),
            reloader: Reloader::create(Box::new(PmfxReloadResponder::new())),
            world_buffers: DynamicWorldBuffers::default(),
            shader_heap,
            unit_quad_mesh: primitives::create_unit_quad_mesh(device),
            total_stats: TotalStats::new(),
            view_errors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Resizes the set of world buffers used for rendering, this assumes that the buffers will be populated each frame
    /// creates new buffers if the requested count exceeds the capacity.
    pub fn reserve_world_buffers(&mut self, device: &mut D, info: WorldBufferReserveInfo) {
        self.world_buffers.draw.reserve(device, &mut self.shader_heap, info.draw_capacity);
        self.world_buffers.extent.reserve(device, &mut self.shader_heap, info.extent_capacity);
        self.world_buffers.material.reserve(device, &mut self.shader_heap, info.material_capacity);
        self.world_buffers.point_light.reserve(device, &mut self.shader_heap, info.point_light_capacity);
        self.world_buffers.spot_light.reserve(device, &mut self.shader_heap, info.spot_light_capacity);
        self.world_buffers.directional_light.reserve(device, &mut self.shader_heap, info.directional_light_capacity);
        self.world_buffers.camera.reserve(device, &mut self.shader_heap, info.camera_capacity);
        self.world_buffers.shadow_matrix.reserve(device, &mut self.shader_heap, info.shadow_matrix_capacity);
    }

    /// Returns a mutable refernce to the the world buffers, these are persistently mapped GPU buffers which can be
    /// updated using the `.write` function
    pub fn get_world_buffers_mut(&mut self) -> &mut DynamicWorldBuffers<D> {
        &mut self.world_buffers
    }

    /// Retunrs a `WorldBufferInfo` that contains the serv index and count of the various world buffers used
    /// during rendering
    pub fn get_world_buffer_info(&self) -> WorldBufferInfo {
        // construct on the fly
        WorldBufferInfo {
            draw: self.world_buffers.draw.get_lookup(),
            extent: self.world_buffers.extent.get_lookup(),
            material: self.world_buffers.material.get_lookup(),
            point_light: self.world_buffers.point_light.get_lookup(),
            spot_light: self.world_buffers.spot_light.get_lookup(),
            directional_light: self.world_buffers.directional_light.get_lookup(),
            camera: self.world_buffers.camera.get_lookup(),
            shadow_matrix: self.world_buffers.shadow_matrix.get_lookup()
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

            let (tex, size, tex_type) = if let Some(filepath) = &pmfx_tex.filepath {
                // load texture from file
                let data_path = if let Some(src_data) = pmfx_tex.src_data {
                    if src_data {
                        super::get_src_data_path(filepath)
                    }
                    else {
                        super::get_data_path(filepath)
                    }
                }
                else {
                    super::get_data_path(filepath)
                };

                let img = image::load_from_file(&data_path)?;
                (device.create_texture_with_heaps::<u8>(
                    &img.info,
                    gfx::TextureHeapInfo {
                        shader: Some(&mut self.shader_heap),
                        ..Default::default()
                    },
                    super::data![&img.data]
                )?, (img.info.width, img.info.height), gfx::TextureType::Texture2D)
            }
            else {
                // create a new empty texture
                let size = self.get_texture_size_from_ratio(pmfx_tex)?;
                let gfx_info = to_gfx_texture_info(pmfx_tex, size);
                (device.create_texture_with_heaps::<u8>(
                    &gfx_info,
                    gfx::TextureHeapInfo {
                        shader: Some(&mut self.shader_heap),
                        ..Default::default()
                    },
                    None)?, size, gfx_info.tex_type)
            };

            self.textures.insert(texture_name.to_string(), (pmfx_tex.hash, TrackedTexture {
                texture: tex,
                ratio: self.pmfx.textures[texture_name].ratio.clone(),
                size: (size.0, size.1, pmfx_tex.depth),
                _tex_type: tex_type
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
            let size = self.textures[texture_name].1.size;
            Some((size.0, size.1))
        }
        else {
            None
        }
    }

    /// Returns the tuple (width, height, depth) of a texture
    pub fn get_texture_3d_size(&self, texture_name: &str) -> Option<(u64, u64, u32)> {
        if self.textures.contains_key(texture_name) {
            Some(self.textures[texture_name].1.size)
        }
        else {
            None
        }
    }

    /// Return 2d or 3d texture dimension where applicable with 1 default in unused dimension and zero if the texture is not found
    pub fn get_texture_dimension(&self, texture_name: &str) -> Vec3u {
        if self.textures.contains_key(texture_name) {
            let size = self.textures[texture_name].1.size;
            Vec3u::new(size.0 as u32, size.1 as u32, size.2)
        }
        else {
            Vec3u::zero()
        }
    }

    /// Retruns a vector of resource use indices specified in `pmfx` pass and based on `ResourceUsage`
    /// creates resources that do not yet exist
    fn get_resource_use_indices(&mut self, device: &mut D, info: &GraphPassInfo) -> Result<Vec<ResourceUse>, super::Error> {
        // create textures we may use        
        let mut use_indices = Vec::new();
        if let Some(uses) = &info.uses {
            for (resource, usage) in uses {
                self.create_texture(device, resource)?;

                let tex = if let Some(tex) = self.get_texture(resource) {
                    tex
                }
                else {
                    return Err(super::Error{
                        msg: format!("missing texture: {} with usage: {:?}", resource, usage)
                    });
                };

                let index = match usage {
                    ResourceUsage::Write => {
                        if let Some(uav) = tex.get_uav_index() {
                            uav
                        }
                        else {
                            return Err(super::Error{
                                msg: format!("error: texture: {} was not created with TextureUsage::UNORDERED_ACCESS for usage: {:?}", resource, usage)
                            });
                        }
                    }
                    ResourceUsage::Read | ResourceUsage::ReadMips => {
                        if let Some(srv) = tex.get_srv_index() {
                            srv
                        }
                        else {
                            return Err(super::Error{
                                msg: format!("error: texture: {} was not created with TextureUsage::SHADER_RESOURCE for usage: {:?}", resource, usage)
                            });
                        }
                    }
                    ResourceUsage::ReadMsaa => {
                        if let Some(srv) = tex.get_msaa_srv_index() {
                            srv
                        }
                        else {
                            return Err(super::Error{
                                msg: format!("error: texture: {} was not created samples > 1 for usage: {:?}", resource, usage)
                            });
                        }
                    }
                };
                use_indices.push(ResourceUse {
                    index: index as u32,
                    dimension: self.get_texture_dimension(resource)
                });
            }
        }
        Ok(use_indices)
    }

    fn create_view_pass_inner(
        &mut self, device: 
        &mut D, view_name: &str, 
        graph_pass_name: &str, 
        info: &GraphPassInfo,
        pmfx_view: &ViewInfo,
        array_slice: usize,
        cubemap: bool
    ) -> Result<(), super::Error> {
        
        // make a custom name for multi pass
        let graph_pass_multi_name = if array_slice > 0 {
            format!("{}_{}", graph_pass_name, array_slice)
        }
        else {
            graph_pass_name.to_string()
        };

        // array of targets by name
        let mut size = (0, 0);
        let mut render_targets = Vec::new();
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
            render_targets: render_targets.to_vec(),
            rt_clear: to_gfx_clear_colour(pmfx_view.clear_colour.clone()),
            depth_stencil,
            ds_clear: to_gfx_clear_depth_stencil(pmfx_view.clear_depth, pmfx_view.clear_stencil),
            resolve: false,
            discard: false,
            array_slice: array_slice
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
        graph_pass_multi_name.hash(&mut hash);
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

        //
        let use_indices = self.get_resource_use_indices(device, info)?;

        // get blit dimension, tbh this should probably be a rect
        let target_size = if let Some(target) = &info.target_dimension {
            if let Some(dim) = self.get_texture_2d_size(target) {
                dim
            }
            else {
                (0, 0)
            }
        }
        else {
            (0, 0)
        };

        let camera_name = if cubemap {
            format!("{}_{}", pmfx_view.camera.to_string(), array_slice)
        }
        else {
            pmfx_view.camera.to_string()
        };

        let view = View::<D> {
            graph_pass_name: graph_pass_multi_name.to_string(),
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
            camera: camera_name,
            view_pipeline,
            use_indices,
            blit_dimension: Vec2f::from((target_size.0 as f32, target_size.1 as f32))
        };

        self.views.insert(graph_pass_multi_name.to_string(), 
            (pmfx_view.hash, Arc::new(Mutex::new(view)), view_name.to_string()));

        // create stats
        self.pass_stats.insert(graph_pass_multi_name.to_string(), PassStats::new(device, 2));

        Ok(())
    }   

    /// Create a view pass from information specified in pmfx file
    fn create_view_pass(&mut self, device: &mut D, view_name: &str, graph_pass_name: &str, info: &GraphPassInfo) -> Result<(), super::Error> {
        if !self.views.contains_key(graph_pass_name) && self.pmfx.views.contains_key(view_name) {

            println!("hotline_rs::pmfx:: creating graph view: {} for {}", graph_pass_name, view_name);

            // create pass from targets
            let pmfx_view = self.pmfx.views[view_name].clone();

            // create textures for view 
            let mut cubemap = false;
            for name in &pmfx_view.render_target {
                self.create_texture(device, name)?;
                self.view_texture_refs.entry(name.to_string())
                    .or_insert(HashSet::new()).insert(graph_pass_name.to_string());

                if self.pmfx.textures[name].cubemap {
                    cubemap = true;
                }
            }

            // create textures for depth stencils
            for name in &pmfx_view.depth_stencil {
                self.create_texture(device, name)?;
                self.view_texture_refs.entry(name.to_string())
                .or_insert(HashSet::new()).insert(graph_pass_name.to_string());

                if self.pmfx.textures[name].cubemap {
                    cubemap = true;
                }
            }

            let mut pass_count = 1;
            if cubemap {
                pass_count = 6;
            }

            for i in 0..pass_count {
                self.create_view_pass_inner(
                    device, view_name, graph_pass_name, info, &pmfx_view, i, cubemap)?;
            }
        }

        Ok(())
    }

    /// Return a reference to a view if the view exists or error otherwise
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

    /// Create a compute pass from info specified in the pmfx file
    fn create_compute_pass(&mut self, device: &mut D, graph_pass_name: &str, info: &GraphPassInfo) -> Result<(), super::Error> {
        let pass_pipeline = if let Some(pipelines) = &info.pipelines {
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

        // create textures we may use
        let use_indices = self.get_resource_use_indices(device, info)?;

        // get the target dimension
        let target_size = if let Some(target) = &info.target_dimension {
            if let Some(dim) = self.get_texture_3d_size(target) {
                dim
            }
            else if let Some(dim) = self.get_texture_2d_size(target) {
                (dim.0, dim.1, 1)
            }
            else {
                (1, 1, 1)
            }
        }
        else {
            (1, 1, 1)
        };

        // get the thread count
        let numthreads = if let Some(threads) = info.numthreads {
            threads
        }
        else {
            let pipeline = self.pmfx.pipelines.get(&pass_pipeline).unwrap();
            if let Some(num_threads) = pipeline["0"].numthreads {
                num_threads
            }
            else {
                (1, 1, 1)
            }
        };

        // hashes
        let mut hash = DefaultHasher::new();
        graph_pass_name.hash(&mut hash);
        let name_hash : PmfxHash = hash.finish();

        // colour hash
        let mut hash = DefaultHasher::new();
        graph_pass_name.hash(&mut hash);
        let colour_hash : u32 = hash.finish() as u32 | 0xff000000;
        
        let pass = ComputePass {
            cmd_buf: device.create_cmd_buf(2),
            pass_pipline: pass_pipeline,
            name_hash,
            colour_hash,
            group_count: gfx::Size3 {
                x: ceil(target_size.0 as f32 / numthreads.0 as f32) as u32,
                y: ceil(target_size.1 as f32 / numthreads.1 as f32) as u32,
                z: ceil(target_size.2 as f32 / numthreads.2 as f32) as u32,
            },
            numthreads: gfx::Size3 {
                x: numthreads.0,
                y: numthreads.1,
                z: numthreads.2,
            },
            use_indices
        };

        self.compute_passes.insert(
            graph_pass_name.to_string(),
            (0, Arc::new(Mutex::new(pass)))
        );

        // create stats
        self.pass_stats.insert(graph_pass_name.to_string(), PassStats::new(device, 2));

        Ok(())
    }

    /// Return a reference to a compute pass if the pass exists or error otherwise
    pub fn get_compute_pass(&self, pass_name: &str) -> Result<ComputePassRef<D>, super::Error> {
        if self.compute_passes.contains_key(pass_name) {
            Ok(self.compute_passes[pass_name].1.clone())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: compute_pass: {} not found", pass_name)
            })
        }
    }

    /// Create all views required for a render graph if necessary, skip if a view already exists
    pub fn create_render_graph_views(&mut self, device: &mut D, graph_name: &str) -> Result<(), super::Error> {
        // create views for all of the nodes
        if self.pmfx.render_graphs.contains_key(graph_name) {
            let pmfx_graph = self.pmfx.render_graphs[graph_name].clone();
            for (graph_pass_name, node) in &pmfx_graph {
                if let Some(view) = &node.view {
                    // create view pass for view node
                    self.create_view_pass(device, view, graph_pass_name, node)?;
                }
                else {
                    // create compute pass for compute node
                    self.create_compute_pass(device, graph_pass_name, node)?;
                }
            }
        }
        Ok(())
    }

    pub fn generate_mip_maps(
        &mut self,
        device: &mut D,
        texture_name: &str) -> Result<(), super::Error> {
        if let Some(tex) = self.get_texture(texture_name) {
            let mut cmd_buf = device.create_cmd_buf(1);
            let barrier_name = format!("barrier_generate_mip_maps-{}", texture_name);
            cmd_buf.begin_event(0xffdc789a, &format!("generate_mip_maps: {}", &texture_name));
            cmd_buf.generate_mip_maps(tex, device, &self.shader_heap)?;
            cmd_buf.end_event();
            cmd_buf.close()?;
            self.barriers.insert(barrier_name.to_string(), cmd_buf);
            // add barrier placeholder in the command_queue
            self.command_queue.push(barrier_name);
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
            let barrier_name = format!("barrier_resolve-{}-{} ({:?})", view_name, texture_name, target_state);
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

            // add barrier placeholder in the command_queue
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
                // add barrier placeholder in the command_queue
                let barrier_name = format!("barrier_{}-{} ({:?})", view_name, texture_name, target_state);
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
                texture_barriers.remove(texture_name);
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
                for (graph_pass_name, instance) in &pmfx_graph {

                    if let Some(view) = &instance.view {
                        // allow missing views to be safely handled
                        if !self.pmfx.views.contains_key(view) {
                            println!("hotline_rs::pmfx:: [warning] missing view {}", view);
                            to_add -= 1;
                            continue;
                        }
                    }
    
                    // already added this pass
                    if dependencies.contains(graph_pass_name) {
                        continue;
                    }
    
                    // wait for dependencies
                    if let Some(depends_on) = &instance.depends_on {
                        let mut passes = false;
                        if !depends_on.is_empty() {
                            for d in depends_on {
                                if !pmfx_graph.contains_key(d) {
                                    passes = true;
                                    println!("hotline_rs::pmfx:: [warning] graph pass {} missing dependency {}. ignoring", 
                                        graph_pass_name, d);
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

                    if let Some(uses) = &instance.uses {
                        // check resource uses
                        for u in uses {
                            let mut resolve = false;
                            let mut gen_mips = false;

                            let resolvable = if let Some(tex) = self.get_texture(&u.0) {
                                if tex.is_resolvable() {
                                    true
                                }
                                else {
                                    false
                                }
                            }
                            else {
                                false
                            };

                            let res_state = match u.1 {
                                ResourceUsage::Write => {
                                    ResourceState::UnorderedAccess
                                },
                                ResourceUsage::Read => {
                                    resolve = resolvable;
                                    ResourceState::ShaderResource
                                },
                                ResourceUsage::ReadMips => {
                                    resolve = resolvable;
                                    gen_mips = true;
                                    ResourceState::ShaderResource
                                },
                                ResourceUsage::ReadMsaa => {
                                    ResourceState::ShaderResource
                                },
                            };

                            // resolve and generate mips
                            if resolve {
                                self.create_resolve_transition(
                                    device, 
                                    &mut barriers, 
                                    &graph_pass_name, 
                                    &u.0,
                                    ResourceState::ShaderResource,
                                )?;
                            }
                            
                            // generate mips on non msaa resources
                            if gen_mips {
                                // generate_mip_maps mips expects us to be in ShaderResource state
                                self.create_texture_transition_barrier(
                                    device, 
                                    &mut barriers, 
                                    &graph_pass_name, 
                                    &u.0,
                                    ResourceState::ShaderResource)?;

                                self.generate_mip_maps(device, &u.0)?;

                                // generate_mip_maps transitions to ShaderResource
                                *barriers.get_mut(&u.0).unwrap() = ResourceState::ShaderResource;
                            }

                            // transition to target state
                            self.create_texture_transition_barrier(
                                device, 
                                &mut barriers, 
                                &graph_pass_name, 
                                &u.0,
                                res_state)?;
                        }
                    }
                    
                    if let Some(view) = &instance.view {
                        // create transitions by inspecting view info
                        let pmfx_view = self.pmfx.views[view].clone();
        
                        // if we need to write to a target we must make sure it is transitioned into render target state
                        for rt_name in pmfx_view.render_target {
                            self.create_texture_transition_barrier(
                                device, &mut barriers, view, &rt_name, ResourceState::RenderTarget)?;
        
                        }
        
                        // same for depth stencils
                        for ds_name in pmfx_view.depth_stencil {
                            self.create_texture_transition_barrier(
                                device, &mut barriers, view, &ds_name, ResourceState::DepthStencil)?;
        
                        }

                        // create pipelines requested for this view instance with the pass format
                        if let Some(pipelines) = &instance.pipelines {
                            for pipeline in pipelines {
                                let view = self.get_view(graph_pass_name)?;
                                let view = view.clone();
                                let view = view.lock().unwrap();
                                self.create_render_pipeline(device, pipeline, &view.pass)?;
                            }
                        }
                    }
                    else if let Some(pipelines) = &instance.pipelines {
                        for pipeline in pipelines {
                            self.create_compute_pipeline(device, pipeline)?;
                        }
                    }

                    // add single pass
                    self.command_queue.push(graph_pass_name.to_string());

                    // add additional 5 passes for cubemaps
                    if let Some(cubemap) = instance.cubemap {
                        if cubemap {
                            for i in 1..6 {
                                self.command_queue.push(format!("{}_{}", graph_pass_name, i));
                            }
                        }
                    }

                    // push a view on
                    added += 1;
                    dependencies.insert(graph_pass_name.to_string());
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

    /// Create a ComputePipeline instance for the combination of pmfx_pipeline settings
    pub fn create_compute_pipeline(&mut self, device: &D, pipeline_name: &str) -> Result<(), super::Error> {              
        if self.pmfx.pipelines.contains_key(pipeline_name) {
            // first create shaders if necessary
            let folder = self.pmfx_folders.get(pipeline_name)
                .unwrap_or_else(|| panic!("hotline_rs::pmfx:: expected to find pipeline {} in pmfx_folders", pipeline_name)).to_string();

            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {
                self.create_shader(device, Path::new(&folder), &pipeline.cs)?;
            }

            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {    
                let cs = self.get_shader(&pipeline.cs);
                if let Some(cs) = cs {
                    let pso = device.create_compute_pipeline(&gfx::ComputePipelineInfo {
                        cs,
                        pipeline_layout: pipeline.pipeline_layout.clone(),
                    })?;
                    println!("hotline_rs::pmfx:: compiled compute pipeline: {}", pipeline_name);

                    // TODO: permutations
                    //let mask = permutation.parse().unwrap();
                    //permutations.insert(mask, (pipeline.hash, pso));
                    
                    self.compute_pipelines.insert(pipeline_name.to_string(), (pipeline.hash, pso));
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

    /// Create a RenderPipeline instance for the combination of pmfx_pipeline settings and an associated RenderPass
    pub fn create_render_pipeline(&mut self, device: &D, pipeline_name: &str, pass: &D::RenderPass) -> Result<(), super::Error> {              
        if self.pmfx.pipelines.contains_key(pipeline_name) {
            // first create shaders if necessary
            let folder = self.pmfx_folders.get(pipeline_name)
                .unwrap_or_else(|| panic!("hotline_rs::pmfx:: expected to find pipeline {} in pmfx_folders", pipeline_name)).to_string();
            
            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {
                self.create_shader(device, Path::new(&folder), &pipeline.vs)?;
                self.create_shader(device, Path::new(&folder), &pipeline.ps)?;
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
                    let vertex_layout = pipeline.vertex_layout.as_ref().unwrap();
                    let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
                        vs: self.get_shader(&pipeline.vs),
                        fs: self.get_shader(&pipeline.ps),
                        input_layout: vertex_layout.to_vec(),
                        pipeline_layout: pipeline.pipeline_layout.clone(),
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

            Ok(())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find pipeline: {}", pipeline_name),
            })
        }
    }

    // Create a raytracing pipeline define in pmfx, with rg, ch, ah or mi shader stages
    pub fn create_raytracing_pipeline(&mut self, device: &D, pipeline_name: &str) -> Result<(), super::Error> {
        if self.pmfx.pipelines.contains_key(pipeline_name) {
            // first create shaders if necessary
            let folder = self.pmfx_folders.get(pipeline_name)
                .unwrap_or_else(|| panic!("hotline_rs::pmfx:: expected to find pipeline {} in pmfx_folders", pipeline_name)).to_string();

            // for each permutation compile its lib shaders
            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {
                let lib = pipeline.lib.expect("hotline_rs::pmfx:: ray tracing pipeline expects a lib member with a set of raytacing shaders");
                for shader in lib {
                    self.create_shader(device, Path::new(&folder), &Some(shader))?;
                }
            }
            
            // for each permutation create a pipeline
            for (_, pipeline) in self.pmfx.pipelines[pipeline_name].clone() {    
                let shaders = pipeline.lib.expect("hotline_rs::pmfx:: ray tracing pipeline expects a lib member with a set of raytacing shaders")
                    .iter()
                    .map(|x| gfx::RaytracingShader {
                        shader: self.get_shader(&Some(x.to_string())).unwrap(),
                        entry_point: get_shader_entry_point_name(Some(x.to_string())).unwrap(),
                    })
                    .collect();

                let raytracing_pipeline = device.create_raytracing_pipeline(&RaytracingPipelineInfo{
                    shaders,
                    pipeline_layout: pipeline.pipeline_layout.clone(),
                    hit_groups: pipeline.hit_groups
                })?;

                let sbt_info = pipeline.sbt.expect("hotline_rs::pmfx:: ray tracing pipeline expects a shader binding table (sbt) to be defined in pmfx");
                let sbt = device.create_raytracing_shader_binding_table(&gfx::RaytracingShaderBindingTableInfo{
                    ray_generation_shader: sbt_info.ray_generation_shader,
                    miss_shaders: sbt_info.miss_shaders,
                    callable_shaders: sbt_info.callable_shaders,
                    hit_groups: sbt_info.hit_groups,
                    pipeline: &raytracing_pipeline
                })?;

                // insert pipeline into the hash map w/ hash
                self.raytracing_pipelines.insert(pipeline_name.to_string(), (pipeline.hash, RaytracingPipelineBinding {
                    pipeline: raytracing_pipeline,
                    sbt
                }));
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

    /// Returns a pmfx pipline for a random / unknown render target format... prefer to use `get_render_pipeline_for_format` 
    /// if you know the format the target you are rendering in to.
    pub fn get_render_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Result<&'stack D::RenderPipeline, super::Error> {
        for format in self.render_pipelines.values() {
            if format.contains_key(pipeline_name) {
                return Ok(&format[pipeline_name][&0].1);
            }
        }
        Err(super::Error {
            msg: format!("hotline_rs::pmfx:: could not find render pipeline for any format: {}", pipeline_name),
        })
    }

    /// Returns a pmfx defined pipeline compatible with the supplied format hash if it exists
    pub fn get_render_pipeline_permutation_for_format<'stack>(&'stack self, pipeline_name: &str, permutation: u32, format_hash: u64) -> Result<&'stack D::RenderPipeline, super::Error> {
        if let Some(formats) = &self.render_pipelines.get(&format_hash) {
            if formats.contains_key(pipeline_name) {
                Ok(&formats[pipeline_name][&permutation].1)
            }
            else {
                Err(super::Error {
                    msg: format!("hotline_rs::pmfx:: could not find render pipeline for format: {} ({})", pipeline_name, format_hash),
                })
            }
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find render pipeline: {}", pipeline_name),
            })
        }
    }

    /// Fetch a prebuilt ComputePipeline
    pub fn get_compute_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Result<&'stack D::ComputePipeline, super::Error> {
        if self.compute_pipelines.contains_key(pipeline_name) {
            Ok(&self.compute_pipelines[pipeline_name].1)
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find compute pipeline: {}", pipeline_name),
            })
        }
    }
 
    /// Fetch a prebuilt RaytracingPipelineBinding which is contains a RaytracingPipeline and RaytracingShaderBindingTable
    pub fn get_raytracing_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Result<&'stack RaytracingPipelineBinding<D>, super::Error> {
        if self.raytracing_pipelines.contains_key(pipeline_name) {
            Ok(&self.raytracing_pipelines[pipeline_name].1)
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find raytracing pipeline: {}", pipeline_name),
            })
        }
    }

    /// Obtain stats for the current frame and caulcuate time deltas between start / end
    fn gather_stats(&mut self, device: &mut D, swap_chain: &D::SwapChain) {
        let mut min_frame_timestamp = f64::max_value();
        let mut max_frame_timestamp = f64::zero();
        let mut total_pipeline_stats = PipelineStatistics::default();
        let timestamp_size_bytes = D::get_timestamp_size_bytes();
        for (name, stats) in &mut self.pass_stats {
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

        // swap the world buffers
        self.world_buffers.camera.swap();
        self.world_buffers.draw.swap();
        self.world_buffers.point_light.swap();
        self.world_buffers.directional_light.swap();
        self.world_buffers.spot_light.swap();

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
                    self.pass_stats.remove(&view.1);
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
                        self.create_render_pipeline(device, &pipeline.1, &view.pass)?;
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
            self.textures.remove(texture_name);
            // create with new dimensions from 'window_sizes'
            self.create_texture(device, texture_name)?;
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
                                    self.pass_stats.remove(view_name);
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

    /// Update camera constants for the named camera, will create a new entry if one does not exist
    pub fn update_cubemap_camera_constants(&mut self, name: &str, pos: Vec3f, near: f32, far: f32) {
        // add a camera for each face
        for i in 0..6 {
            let name = format!("{}_{}", name, i);
            let constants = cubemap_camera_face(i, pos, near, far);
            *self.cameras.entry(name.to_string()).or_insert(constants.clone()) = constants.clone();
        }
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

    fn stats_start(cmd_buf: &mut D::CmdBuf, pass_stats: &mut PassStats<D>) {
        // sync to the frame
        pass_stats.fences[pass_stats.write_index] = pass_stats.frame_fence_value;

        // view timestamps
        pass_stats.timestamp_heap.reset();
        let buf = &mut pass_stats.timestamp_buffers[pass_stats.write_index][0];
        cmd_buf.timestamp_query(&mut pass_stats.timestamp_heap, buf);

        // view pipeline stats
        pass_stats.pipeline_stats_heap.reset();
        pass_stats.pipeline_query_index = cmd_buf.begin_query(
            &mut pass_stats.pipeline_stats_heap, 
            gfx::QueryType::PipelineStatistics
        );
    }

    fn stats_end(cmd_buf: &mut D::CmdBuf, pass_stats: &mut PassStats<D>) {
        // end timestamp
        let buf = &mut pass_stats.timestamp_buffers[pass_stats.write_index][1];
        cmd_buf.timestamp_query(&mut pass_stats.timestamp_heap, buf);

        // end pipeline stats query
        if pass_stats.pipeline_query_index != usize::max_value() {
            let buf = &mut pass_stats.pipeline_stats_buffers[pass_stats.write_index];
            cmd_buf.end_query(
                &mut pass_stats.pipeline_stats_heap, 
                gfx::QueryType::PipelineStatistics,
                pass_stats.pipeline_query_index,
                buf,
            );
            pass_stats.pipeline_query_index = usize::max_value();
        }
    }

    /// Resets all command buffers, this assumes they have been used and need to be reset for the next frame
    pub fn reset(&mut self, swap_chain: &D::SwapChain) {
        // TODO: collapse minimise
        for (name, view) in &self.views {
            // rest only command buffers that are in use
            if self.command_queue.contains(name) {
                let view = view.clone();
                let mut view = view.1.lock().unwrap();
                view.cmd_buf.reset(swap_chain);

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.pass_stats.remove(name).unwrap();
                Self::stats_start(&mut view.cmd_buf, &mut stats);
                self.pass_stats.insert(name.to_string(), stats);
            }
        }
        for (name, pass) in &self.compute_passes {
            // rest only command buffers that are in use
            if self.command_queue.contains(name) {
                let pass = pass.clone();
                let mut pass = pass.1.lock().unwrap();
                pass.cmd_buf.reset(swap_chain);

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.pass_stats.remove(name).unwrap();
                Self::stats_start(&mut pass.cmd_buf, &mut stats);
                self.pass_stats.insert(name.to_string(), stats);
            }
        }
    }

    /// Returns a vector of information to call render functions. It returns a tuple (function_name, view_name)
    /// which is called as so: `function_name(view)` so functions can be re-used for different views
    pub fn get_render_graph_function_info(&self, render_graph: &str) -> Vec<(String, String)> {
        if self.pmfx.render_graphs.contains_key(render_graph) {
            let mut passes = Vec::new();
            for (name, pass) in &self.pmfx.render_graphs[render_graph] {
                passes.push((pass.function.to_string(), name.to_string()));
                // add additional cubemap passes
                if let Some(cubemap) = pass.cubemap {
                    if cubemap {
                        for i in 1..6 {
                            passes.push((pass.function.to_string(), format!("{}_{}", name, i).to_string()));
                        }
                    }
                }
            }
            passes
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
            // TODO: collapse minimise
            else if self.views.contains_key(node) {
                // execute a view
                let view = self.views[node].clone();
                let view = &mut view.1.lock().unwrap();

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.pass_stats.remove(node).unwrap();
                Self::stats_end(&mut view.cmd_buf, &mut stats);
                self.pass_stats.insert(node.to_string(), stats);

                view.cmd_buf.close().unwrap();
                device.execute(&view.cmd_buf);
            }
            else if self.compute_passes.contains_key(node) {
                // dispatch a compute pass
                let pass = self.compute_passes[node].clone();
                let pass = &mut pass.1.lock().unwrap();

                // inserts markers for timing and tracking pipeline stats
                let mut stats = self.pass_stats.remove(node).unwrap();
                Self::stats_end(&mut pass.cmd_buf, &mut stats);
                self.pass_stats.insert(node.to_string(), stats);

                pass.cmd_buf.close().unwrap();
                device.execute(&pass.cmd_buf);
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
impl<D, A> imgui::UserInterface<D, A> for Pmfx<D> where D: gfx::Device, A: os::App, D::RenderPipeline: gfx::Pipeline {
    fn show_ui(&mut self, imgui: &mut imgui::ImGui<D, A>, open: bool) -> bool {
        if open {
            let mut imgui_open = open;
            if imgui.begin("textures", &mut imgui_open, imgui::WindowFlags::ALWAYS_HORIZONTAL_SCROLLBAR) {
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

            if imgui.begin("pmfx", &mut imgui_open, imgui::WindowFlags::ALWAYS_HORIZONTAL_SCROLLBAR) {
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