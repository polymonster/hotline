#![cfg(target_os = "macos")]
// objc 0.2.x's sel_impl macro uses cfg(cargo-clippy) which is not a valid identifier
// so it cannot be declared via check-cfg; suppress the lint for this file.
#![allow(unexpected_cfgs)]

use crate::os_platform;
use crate::os::Window;

use bevy_ecs::system::lifetimeless::Read;
use cocoa::foundation::NSUInteger;
use metal::MTLScissorRect;
use metal::MTLStepFunction;
use metal::MTLTextureUsage;
use metal::MTLVertexFormat;
use metal::MTLViewport;
use metal::TextureDescriptor;
use metal::VertexAttributeDescriptorArray;

use super::*;
use super::Device as SuperDevice;
use super::ReadBackRequest as SuperReadBackRequest;
use super::Heap as SuperHeap;
use super::Pipeline as SuperPipleline;

use std::alloc::Layout;
use std::collections::HashMap;
use std::result;

use cocoa::{appkit::NSView, base::id as cocoa_id};
#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};
use core_graphics_types::geometry::CGSize;

use std::path::Path;

const MEGA_BYTE : usize = 1024 * 1024 * 1024;

const fn to_mtl_vertex_format(format: super::Format) -> MTLVertexFormat {
    match format {
        super::Format::Unknown => MTLVertexFormat::Invalid,
        super::Format::R16n => MTLVertexFormat::ShortNormalized,
        super::Format::R16u => MTLVertexFormat::UShort,
        super::Format::R16i => MTLVertexFormat::Short,
        super::Format::R16f => MTLVertexFormat::Half,
        super::Format::R32u => MTLVertexFormat::UInt,
        super::Format::R32i => MTLVertexFormat::Int,
        super::Format::R32f => MTLVertexFormat::Float,
        super::Format::RG16u => MTLVertexFormat::UShort2,
        super::Format::RG16i => MTLVertexFormat::Short2,
        super::Format::RG16f => MTLVertexFormat::Half2,
        super::Format::RG32u => MTLVertexFormat::UInt2,
        super::Format::RG32i => MTLVertexFormat::Int2,
        super::Format::RG32f => MTLVertexFormat::Float2,
        super::Format::RGB32u => MTLVertexFormat::UInt3,
        super::Format::RGB32i => MTLVertexFormat::Int3,
        super::Format::RGB32f => MTLVertexFormat::Float3,
        super::Format::RGBA8n => MTLVertexFormat::UChar4Normalized,
        super::Format::RGBA8u => MTLVertexFormat::UChar4,
        super::Format::RGBA8i => MTLVertexFormat::Char4,
        super::Format::RGBA16u => MTLVertexFormat::UShort4,
        super::Format::RGBA16i => MTLVertexFormat::Short4,
        super::Format::RGBA16f => MTLVertexFormat::Half4,
        super::Format::RGBA32u => MTLVertexFormat::UInt4,
        super::Format::RGBA32i => MTLVertexFormat::Int4,
        super::Format::RGBA32f => MTLVertexFormat::Float4,
        _ => panic!("hotline_rs::gfx::mtl unsupported vertex format")
    }
}

fn to_mtl_primitive_type(topology: Topology) -> metal::MTLPrimitiveType {
    match topology {
        Topology::PointList => metal::MTLPrimitiveType::Point,
        Topology::LineList => metal::MTLPrimitiveType::Line,
        Topology::LineStrip => metal::MTLPrimitiveType::LineStrip,
        Topology::TriangleList => metal::MTLPrimitiveType::Triangle,
        Topology::TriangleStrip => metal::MTLPrimitiveType::TriangleStrip,
        _ => metal::MTLPrimitiveType::Triangle,
    }
}

fn to_mtl_blend_factor(factor: &super::BlendFactor) -> metal::MTLBlendFactor {
    match factor {
        super::BlendFactor::Zero => metal::MTLBlendFactor::Zero,
        super::BlendFactor::One => metal::MTLBlendFactor::One,
        super::BlendFactor::SrcColour => metal::MTLBlendFactor::SourceColor,
        super::BlendFactor::InvSrcColour => metal::MTLBlendFactor::OneMinusSourceColor,
        super::BlendFactor::SrcAlpha => metal::MTLBlendFactor::SourceAlpha,
        super::BlendFactor::InvSrcAlpha => metal::MTLBlendFactor::OneMinusSourceAlpha,
        super::BlendFactor::DstAlpha => metal::MTLBlendFactor::DestinationAlpha,
        super::BlendFactor::InvDstAlpha => metal::MTLBlendFactor::OneMinusDestinationAlpha,
        super::BlendFactor::DstColour => metal::MTLBlendFactor::DestinationColor,
        super::BlendFactor::InvDstColour => metal::MTLBlendFactor::OneMinusDestinationColor,
        super::BlendFactor::SrcAlphaSat => metal::MTLBlendFactor::SourceAlphaSaturated,
        super::BlendFactor::BlendFactor => metal::MTLBlendFactor::BlendColor,
        super::BlendFactor::InvBlendFactor => metal::MTLBlendFactor::OneMinusBlendColor,
        super::BlendFactor::Src1Colour => metal::MTLBlendFactor::Source1Color,
        super::BlendFactor::InvSrc1Colour => metal::MTLBlendFactor::OneMinusSource1Color,
        super::BlendFactor::Src1Alpha => metal::MTLBlendFactor::Source1Alpha,
        super::BlendFactor::InvSrc1Alpha => metal::MTLBlendFactor::OneMinusSource1Alpha,
    }
}

fn to_mtl_blend_op(op: &super::BlendOp) -> metal::MTLBlendOperation {
    match op {
        super::BlendOp::Add => metal::MTLBlendOperation::Add,
        super::BlendOp::Subtract => metal::MTLBlendOperation::Subtract,
        super::BlendOp::RevSubtract => metal::MTLBlendOperation::ReverseSubtract,
        super::BlendOp::Min => metal::MTLBlendOperation::Min,
        super::BlendOp::Max => metal::MTLBlendOperation::Max,
    }
}

fn to_mtl_write_mask(mask: &super::WriteMask) -> metal::MTLColorWriteMask {
    let mut mtl_mask = metal::MTLColorWriteMask::empty();
    if mask.contains(super::WriteMask::RED) {
        mtl_mask |= metal::MTLColorWriteMask::Red;
    }
    if mask.contains(super::WriteMask::GREEN) {
        mtl_mask |= metal::MTLColorWriteMask::Green;
    }
    if mask.contains(super::WriteMask::BLUE) {
        mtl_mask |= metal::MTLColorWriteMask::Blue;
    }
    if mask.contains(super::WriteMask::ALPHA) {
        mtl_mask |= metal::MTLColorWriteMask::Alpha;
    }
    mtl_mask
}

fn to_mtl_texture_type(tex_type: super::TextureType) -> metal::MTLTextureType {
    match tex_type {
        super::TextureType::Texture1D => metal::MTLTextureType::D1,
        super::TextureType::Texture1DArray => metal::MTLTextureType::D1Array,
        super::TextureType::Texture2D => metal::MTLTextureType::D2,
        super::TextureType::Texture2DArray => metal::MTLTextureType::D2Array,
        super::TextureType::Texture3D => metal::MTLTextureType::D3,
        super::TextureType::TextureCube => metal::MTLTextureType::Cube,
        super::TextureType::TextureCubeArray => metal::MTLTextureType::CubeArray,
    }
}

fn to_mtl_texture_usage(usage: TextureUsage) -> MTLTextureUsage {
    let mut mtl_usage : MTLTextureUsage = MTLTextureUsage::Unknown;
    if usage.contains(super::TextureUsage::SHADER_RESOURCE) {
        mtl_usage.insert(MTLTextureUsage::ShaderRead);
    }
    if usage.contains(super::TextureUsage::UNORDERED_ACCESS) {
        mtl_usage.insert(MTLTextureUsage::ShaderWrite);
    }
    if usage.contains(super::TextureUsage::RENDER_TARGET) {
        mtl_usage.insert(MTLTextureUsage::RenderTarget);
    }
    if usage.contains(super::TextureUsage::RENDER_TARGET) ||
        usage.contains(super::TextureUsage::DEPTH_STENCIL) {
            mtl_usage.insert(MTLTextureUsage::RenderTarget);
    }
    mtl_usage
}

fn to_mtl_compare_func(func: super::ComparisonFunc) -> metal::MTLCompareFunction {
    match func {
        super::ComparisonFunc::Never => metal::MTLCompareFunction::Never,
        super::ComparisonFunc::Less => metal::MTLCompareFunction::Less,
        super::ComparisonFunc::Equal => metal::MTLCompareFunction::Equal,
        super::ComparisonFunc::LessEqual => metal::MTLCompareFunction::LessEqual,
        super::ComparisonFunc::Greater => metal::MTLCompareFunction::Greater,
        super::ComparisonFunc::NotEqual => metal::MTLCompareFunction::NotEqual,
        super::ComparisonFunc::GreaterEqual => metal::MTLCompareFunction::GreaterEqual,
        super::ComparisonFunc::Always => metal::MTLCompareFunction::Always,
    }
}

fn to_mtl_sampler_address_mode(mode: super::SamplerAddressMode) -> metal::MTLSamplerAddressMode {
    match mode {
        super::SamplerAddressMode::Wrap => metal::MTLSamplerAddressMode::Repeat,
        super::SamplerAddressMode::Mirror => metal::MTLSamplerAddressMode::MirrorRepeat,
        super::SamplerAddressMode::Clamp => metal::MTLSamplerAddressMode::ClampToEdge,
        super::SamplerAddressMode::Border => metal::MTLSamplerAddressMode::ClampToBorderColor,
        super::SamplerAddressMode::MirrorOnce => metal::MTLSamplerAddressMode::MirrorClampToEdge,
    }
}

fn to_mtl_sampler_min_mag_filter(filter: super::SamplerFilter) -> metal::MTLSamplerMinMagFilter {
    match filter {
        super::SamplerFilter::Point => metal::MTLSamplerMinMagFilter::Nearest,
        super::SamplerFilter::Linear | super::SamplerFilter::Anisotropic => metal::MTLSamplerMinMagFilter::Linear,
    }
}

fn to_mtl_sampler_mip_filter(filter: super::SamplerFilter) -> metal::MTLSamplerMipFilter {
    match filter {
        super::SamplerFilter::Point => metal::MTLSamplerMipFilter::Nearest,
        super::SamplerFilter::Linear | super::SamplerFilter::Anisotropic => metal::MTLSamplerMipFilter::Linear,
    }
}

fn to_mtl_stencil_op(op: super::StencilOp) -> metal::MTLStencilOperation {
    match op {
        super::StencilOp::Keep => metal::MTLStencilOperation::Keep,
        super::StencilOp::Zero => metal::MTLStencilOperation::Zero,
        super::StencilOp::Replace => metal::MTLStencilOperation::Replace,
        super::StencilOp::IncrSat => metal::MTLStencilOperation::IncrementClamp,
        super::StencilOp::DecrSat => metal::MTLStencilOperation::DecrementClamp,
        super::StencilOp::Invert => metal::MTLStencilOperation::Invert,
        super::StencilOp::Incr => metal::MTLStencilOperation::IncrementWrap,
        super::StencilOp::Decr => metal::MTLStencilOperation::DecrementWrap,
    }
}

fn has_stencil_component(format: metal::MTLPixelFormat) -> bool {
    matches!(format,
        metal::MTLPixelFormat::Depth32Float_Stencil8
    )
}

fn is_depth_format(format: metal::MTLPixelFormat) -> bool {
    matches!(format,
        metal::MTLPixelFormat::Depth32Float_Stencil8
        | metal::MTLPixelFormat::Depth32Float
        | metal::MTLPixelFormat::Depth16Unorm
    )
}

fn to_mtl_cull_mode(cull_mode: super::CullMode) -> metal::MTLCullMode {
    match cull_mode {
        super::CullMode::None => metal::MTLCullMode::None,
        super::CullMode::Front => metal::MTLCullMode::Front,
        super::CullMode::Back => metal::MTLCullMode::Back,
    }
}

fn to_mtl_winding(front_ccw: bool) -> metal::MTLWinding {
    if front_ccw {
        metal::MTLWinding::CounterClockwise
    } else {
        metal::MTLWinding::Clockwise
    }
}

fn to_mtl_triangle_fill_mode(fill_mode: super::FillMode) -> metal::MTLTriangleFillMode {
    match fill_mode {
        super::FillMode::Solid => metal::MTLTriangleFillMode::Fill,
        super::FillMode::Wireframe => metal::MTLTriangleFillMode::Lines,
    }
}

fn to_mtl_index_type(stride: usize) -> metal::MTLIndexType {
    match stride {
        2 => metal::MTLIndexType::UInt16,
        4 => metal::MTLIndexType::UInt32,
        _ => panic!("Invalid index stride: {}, expected 2 or 4", stride),
    }
}

fn to_mtl_pixel_format(format: super::Format) -> metal::MTLPixelFormat {
    match format {
        super::Format::Unknown => metal::MTLPixelFormat::Invalid,
        super::Format::R16n => metal::MTLPixelFormat::R16Unorm,
        super::Format::R16u => metal::MTLPixelFormat::R16Uint,
        super::Format::R16i => metal::MTLPixelFormat::R16Sint,
        super::Format::R16f => metal::MTLPixelFormat::R16Float,
        super::Format::R32u => metal::MTLPixelFormat::R32Uint,
        super::Format::R32i => metal::MTLPixelFormat::R32Sint,
        super::Format::R32f => metal::MTLPixelFormat::R32Float,
        super::Format::RG16f => metal::MTLPixelFormat::RG16Float,
        super::Format::RG16u => metal::MTLPixelFormat::RG16Uint,
        super::Format::RG16i => metal::MTLPixelFormat::RG16Sint,
        super::Format::RG32u => metal::MTLPixelFormat::RG32Uint,
        super::Format::RG32i => metal::MTLPixelFormat::RG32Sint,
        super::Format::RG32f => metal::MTLPixelFormat::RG32Float,
        super::Format::RGB32u |
        super::Format::RGB32i |
        super::Format::RGB32f => panic!("hotline_rs::gfx::mtl RGB32 formats not supported in Metal"),
        super::Format::RGBA8nSRGB => metal::MTLPixelFormat::RGBA8Unorm_sRGB,
        super::Format::RGBA8n => metal::MTLPixelFormat::RGBA8Unorm,
        super::Format::RGBA8u => metal::MTLPixelFormat::RGBA8Uint,
        super::Format::RGBA8i => metal::MTLPixelFormat::RGBA8Sint,
        super::Format::BGRA8n => metal::MTLPixelFormat::BGRA8Unorm,
        super::Format::BGRX8n => metal::MTLPixelFormat::BGRA8Unorm,
        super::Format::BGRA8nSRGB => metal::MTLPixelFormat::BGRA8Unorm_sRGB,
        super::Format::BGRX8nSRGB => metal::MTLPixelFormat::BGRA8Unorm_sRGB,
        super::Format::RGBA16u => metal::MTLPixelFormat::RGBA16Uint,
        super::Format::RGBA16i => metal::MTLPixelFormat::RGBA16Sint,
        super::Format::RGBA16f => metal::MTLPixelFormat::RGBA16Float,
        super::Format::RGBA32u => metal::MTLPixelFormat::RGBA32Uint,
        super::Format::RGBA32i => metal::MTLPixelFormat::RGBA32Sint,
        super::Format::RGBA32f => metal::MTLPixelFormat::RGBA32Float,
        super::Format::D32fS8X24u => metal::MTLPixelFormat::Depth32Float_Stencil8,
        super::Format::D32f => metal::MTLPixelFormat::Depth32Float,
        super::Format::D24nS8u => metal::MTLPixelFormat::Depth32Float_Stencil8, // D24S8 not supported on Apple Silicon
        super::Format::D16n => metal::MTLPixelFormat::Depth16Unorm,
        super::Format::BC1n => metal::MTLPixelFormat::BC1_RGBA,
        super::Format::BC1nSRGB => metal::MTLPixelFormat::BC1_RGBA_sRGB,
        super::Format::BC2n => metal::MTLPixelFormat::BC2_RGBA,
        super::Format::BC2nSRGB => metal::MTLPixelFormat::BC2_RGBA_sRGB,
        super::Format::BC3n => metal::MTLPixelFormat::BC3_RGBA,
        super::Format::BC3nSRGB => metal::MTLPixelFormat::BC3_RGBA_sRGB,
        super::Format::BC4n => metal::MTLPixelFormat::BC4_RUnorm,
        super::Format::BC5n => metal::MTLPixelFormat::BC5_RGUnorm,
    }
}

fn to_mtl_data_type(resource_type: super::ResourceType) -> metal::MTLDataType {
    match resource_type {
        super::ResourceType::StructuredBuffer |
        super::ResourceType::RWStructuredBuffer |
        super::ResourceType::ConstantBuffer |
        super::ResourceType::ByteAddressBuffer |
        super::ResourceType::RWByteAddressBuffer |
        super::ResourceType::Buffer => metal::MTLDataType::Pointer,
        _ => metal::MTLDataType::Texture, // Texture2D, RWTexture2D, etc.
    }
}

// HLSL register kind ('t', 'u', 'b', 's') for a DescriptorType. Used to group bindings into MSL
// descriptor sets keyed by (kind, register_number) so e.g. t0 and u0 never share a [[buffer(N)]].
fn descriptor_register_kind(ty: super::DescriptorType) -> char {
    match ty {
        super::DescriptorType::ShaderResource => 't',
        super::DescriptorType::UnorderedAccess => 'u',
        super::DescriptorType::ConstantBuffer | super::DescriptorType::PushConstants => 'b',
        super::DescriptorType::Sampler => 's',
    }
}

#[derive(Clone)]
pub struct Device {
    metal_device: metal::Device,
    command_queue: metal::CommandQueue,
    shader_heap: Heap,
    adapter_info: AdapterInfo,
    heap_alloc_id: u16,
    /// True when the GPU can sample timestamp counters at encoder stage boundaries, which lets us
    /// take real per-encoder GPU timestamps. When false we fall back to MTLCommandBuffer's whole-CB
    /// GPUStartTime / GPUEndTime (see timestamp_query / read_timestamps).
    supports_stage_boundary_timestamps: bool,
}

/// MTLCounterSamplingPoint::atStageBoundary — sampling at the boundary between encoder stages.
const MTL_COUNTER_SAMPLING_POINT_AT_STAGE_BOUNDARY: NSUInteger = 0;
/// MTLCounterDontSample sentinel: a stage index that should not record a timestamp.
const MTL_COUNTER_DONT_SAMPLE: NSUInteger = NSUInteger::MAX;

#[derive(Clone)]
pub struct SwapChain {
    layer: metal::MetalLayer,
    drawable: metal::MetalDrawable,
    view: *mut objc::runtime::Object,
    backbuffer_clear: Option<ClearColour>,
    backbuffer_texture: Texture,
    backbuffer_pass: RenderPass,
    backbuffer_pass_no_clear: RenderPass,
    num_buffers: u32,
    // GPU-side fence: present CB signals, each new CB waits — serialises GPU frames without blocking CPU
    frame_event: metal::Event,
    frame_value: u64,
    // CPU-side ring: blocks the CPU only when num_buffers frames are already in flight,
    // preventing shared-memory DynamicBuffer slots from being overwritten before the GPU is done
    in_flight: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<metal::CommandBuffer>>>,
}

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
    }

    fn wait_for_last_frame(&self) {
        let mut in_flight = self.in_flight.lock().unwrap();
        if in_flight.len() >= self.num_buffers as usize {
            if let Some(oldest) = in_flight.pop_front() {
                drop(in_flight);
                oldest.wait_until_completed();
            }
        }
    }

    fn get_num_buffers(&self) -> u32 {
        self.num_buffers
    }

    fn get_frame_fence_value(&self) -> u64 {
        self.frame_value
    }

    fn update<A: os::App>(&mut self, device: &mut Device, window: &A::Window, cmd: &mut CmdBuf) -> bool {
        objc::rc::autoreleasepool(|| {
            let draw_size = window.get_size();
            self.layer.set_contents_scale(window.get_dpi_scale() as f64);
            self.layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

            let drawable = self.layer.next_drawable()
                .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

            self.drawable = drawable.to_owned();

            self.backbuffer_texture = Texture {
                metal_texture: drawable.texture().to_owned(),
                resolved_texture: None,
                srv_index: None,
                msaa_srv_index: None,
                uav_index: None,
                resolvable: false,
                heap_id: None
            };

            self.backbuffer_pass = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, self.backbuffer_clear);
            self.backbuffer_pass_no_clear = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, None);
        });

        // TODO: check usage
        true
    }

    fn get_backbuffer_index(&self) -> u32 {
        0
    }

    fn get_backbuffer_texture(&self) -> &Texture {
        &self.backbuffer_texture
    }

    fn get_backbuffer_pass(&self) -> &RenderPass {
        &self.backbuffer_pass
    }

    fn get_backbuffer_pass_mut(&mut self) -> &mut RenderPass {
        &mut self.backbuffer_pass
    }

    fn get_backbuffer_pass_no_clear(&self) -> &RenderPass {
        &self.backbuffer_pass_no_clear
    }

    fn get_backbuffer_pass_no_clear_mut(&mut self) -> &mut RenderPass {
        &mut self.backbuffer_pass_no_clear
    }

    fn swap(&mut self, device: &mut Device) {
        objc::rc::autoreleasepool(|| {
            self.frame_value += 1;
            let in_flight_count = self.in_flight.lock().unwrap().len();
            let cmd = device.command_queue.new_command_buffer().to_owned();
            cmd.present_drawable(&self.drawable);
            cmd.encode_signal_event(&self.frame_event, self.frame_value);
            cmd.commit();
            self.in_flight.lock().unwrap().push_back(cmd);
        });
    }
}

pub struct CmdBuf {
    cmd_queue: metal::CommandQueue,
    cmd: Option<metal::CommandBuffer>,
    render_encoder: Option<metal::RenderCommandEncoder>,
    compute_encoder: Option<metal::ComputeCommandEncoder>,
    bound_index_buffer: Option<metal::Buffer>,
    bound_index_stride: usize,
    bound_render_pipeline: Option<*const RenderPipeline>,
    bound_compute_pipeline: Option<*const ComputePipeline>,
    metal_device: metal::Device,
    transient_buffers: Vec<metal::Buffer>,
    vertex_binder: HashMap<SlotKey, PipelineStageBinder>,
    fragment_binder: HashMap<SlotKey, PipelineStageBinder>,
    compute_binder: HashMap<SlotKey, PipelineStageBinder>,
    deferred_ops: Vec<DeferredBarrierOp>,
    pending_timestamp: Option<(metal::CounterSampleBuffer, NSUInteger)>,
}

/// A unit of render-graph barrier work to replay each frame on Metal. Transition barriers are
/// absent because Metal tracks hazards automatically for resources in a `Tracked` heap.
#[derive(Clone)]
enum DeferredBarrierOp {
    /// Resolve an MSAA texture into its single-sample resolve backing via a load/no-clear pass.
    Resolve {
        msaa: metal::Texture,
        resolve: metal::Texture,
    },
    /// Regenerate the mip chain of a sampled texture from mip 0 with a blit encoder.
    GenerateMips {
        texture: metal::Texture,
    },
}

impl Clone for CmdBuf {
    fn clone(&self) -> Self {
        CmdBuf {
            cmd_queue: self.cmd_queue.clone(),
            cmd: self.cmd.clone(),
            render_encoder: self.render_encoder.clone(),
            compute_encoder: self.compute_encoder.clone(),
            bound_index_buffer: self.bound_index_buffer.clone(),
            bound_index_stride: self.bound_index_stride,
            bound_render_pipeline: self.bound_render_pipeline,
            bound_compute_pipeline: self.bound_compute_pipeline,
            metal_device: self.metal_device.clone(),
            transient_buffers: self.transient_buffers.clone(),
            vertex_binder: self.vertex_binder.clone(),
            fragment_binder: self.fragment_binder.clone(),
            compute_binder: self.compute_binder.clone(),
            deferred_ops: self.deferred_ops.clone(),
            pending_timestamp: self.pending_timestamp.clone(),
        }
    }
}

impl CmdBuf {
    fn allocate_stage_bindings(
        &mut self,
        binder: &HashMap<SlotKey, PipelineStageBinder>,
        stage: super::ShaderType,
    ) {
        let encoder = match self.render_encoder.as_ref() {
            Some(e) => e,
            None => return,
        };

        // Bind push constants using setVertexBytes/setFragmentBytes (zero allocations)
        // Skip binders that have not changed since the last draw
        for b in binder.values() {
            if let PipelineStageBinder::PushConstants(pc) = b {
                if !pc.dirty {
                    continue;
                }
                let data_size = (pc.num_32_bit_constants * 4) as u64;
                let data_ptr = pc.data.as_ptr() as *const std::ffi::c_void;

                match stage {
                    super::ShaderType::Vertex => {
                        encoder.set_vertex_bytes(pc.buffer_index as u64, data_size, data_ptr);
                    }
                    super::ShaderType::Fragment => {
                        encoder.set_fragment_bytes(pc.buffer_index as u64, data_size, data_ptr);
                    }
                    _ => unimplemented!(),
                }
            }
        }

        // Group resource bindings by buffer_index, skipping groups where nothing is dirty
        let mut groups: HashMap<u32, Vec<&ResourceBinder>> = HashMap::new();
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if rb.bound_resource.is_some() && rb.dirty {
                    groups.entry(rb.buffer_index).or_default().push(rb);
                }
            }
        }

        // TODO: move to to_mtl_stage function + implement the others
        let render_stage = match stage {
            super::ShaderType::Vertex => metal::MTLRenderStages::Vertex,
            super::ShaderType::Fragment => metal::MTLRenderStages::Fragment,
            _ => unimplemented!(),
        };

        // Allocate resource bindings (grouped by buffer_index)
        for (buffer_index, mut binders) in groups {
            // Sort by binding_index to ensure deterministic order
            binders.sort_by_key(|rb| rb.binding_index);

            let arg_descs: Vec<metal::ArgumentDescriptor> = binders.iter().map(|rb| {
                let arg_desc = metal::ArgumentDescriptor::new();
                arg_desc.set_index(rb.binding_index as u64);
                arg_desc.set_data_type(rb.data_type);
                arg_desc.set_array_length(rb.array_length);
                arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);
                arg_desc.to_owned()
            }).collect();

            let arg_encoder = self.metal_device.new_argument_encoder(
                metal::Array::from_owned_slice(&arg_descs)
            );
            let arg_buffer = self.metal_device.new_buffer(
                arg_encoder.encoded_length(),
                metal::MTLResourceOptions::StorageModeShared
            );
            arg_encoder.set_argument_buffer(&arg_buffer, 0);

            for rb in &binders {
                if let Some(ref binding) = rb.bound_resource {
                    let heap = unsafe { &*binding.heap_ptr };
                    encoder.use_heap_at(&heap.mtl_heap, render_stage);

                    match rb.data_type {
                        metal::MTLDataType::Texture => {
                            if let Some(texture) = heap.texture_slots.get(binding.offset).and_then(|t| t.as_ref()) {
                                arg_encoder.set_texture(rb.binding_index as u64, texture);
                            }
                        }
                        metal::MTLDataType::Pointer => {
                            if let Some(buffer) = heap.buffer_slots.get(binding.offset).and_then(|b| b.as_ref()) {
                                arg_encoder.set_buffer(rb.binding_index as u64, buffer, 0);
                            }
                        }
                        _ => {}
                    }
                }
            }

            match stage {
                super::ShaderType::Vertex => encoder.set_vertex_buffer(buffer_index as u64, Some(&arg_buffer), 0),
                super::ShaderType::Fragment => encoder.set_fragment_buffer(buffer_index as u64, Some(&arg_buffer), 0),
                _ => unimplemented!(),
            }
            self.transient_buffers.push(arg_buffer);
        }
    }

    fn allocate_stage_resources(&mut self) {
        let vertex_binder = self.vertex_binder.clone();
        let fragment_binder = self.fragment_binder.clone();

        self.allocate_stage_bindings(&vertex_binder, super::ShaderType::Vertex);
        self.allocate_stage_bindings(&fragment_binder, super::ShaderType::Fragment);

        // Clear dirty flags on originals now that encoding is done
        for b in self.vertex_binder.values_mut() {
            match b {
                PipelineStageBinder::PushConstants(pc) => pc.dirty = false,
                PipelineStageBinder::Resource(rb) => rb.dirty = false,
            }
        }
        for b in self.fragment_binder.values_mut() {
            match b {
                PipelineStageBinder::PushConstants(pc) => pc.dirty = false,
                PipelineStageBinder::Resource(rb) => rb.dirty = false,
            }
        }
    }

    /// Flush the compute binder state onto the active compute encoder before a dispatch. Push
    /// constants go via setBytes; explicitly bound (non-bindless) resources are encoded into a
    /// transient argument buffer - bindless heap argument buffers are already bound by `set_heap`.
    fn allocate_compute_resources(&mut self) {
        let encoder = match self.compute_encoder.as_ref() {
            Some(e) => e,
            None => return,
        };
        let binder = self.compute_binder.clone();

        // push constants
        for b in binder.values() {
            if let PipelineStageBinder::PushConstants(pc) = b {
                if !pc.dirty {
                    continue;
                }
                let data_size = (pc.num_32_bit_constants * 4) as u64;
                let data_ptr = pc.data.as_ptr() as *const std::ffi::c_void;
                encoder.set_bytes(pc.buffer_index as u64, data_size, data_ptr);
            }
        }

        // explicitly bound resources, grouped by buffer_index
        let mut groups: HashMap<u32, Vec<&ResourceBinder>> = HashMap::new();
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if rb.bound_resource.is_some() && rb.dirty {
                    groups.entry(rb.buffer_index).or_default().push(rb);
                }
            }
        }

        for (buffer_index, mut binders) in groups {
            binders.sort_by_key(|rb| rb.binding_index);

            let arg_descs: Vec<metal::ArgumentDescriptor> = binders.iter().map(|rb| {
                let arg_desc = metal::ArgumentDescriptor::new();
                arg_desc.set_index(rb.binding_index as u64);
                arg_desc.set_data_type(rb.data_type);
                arg_desc.set_array_length(rb.array_length);
                arg_desc.set_access(metal::MTLArgumentAccess::ReadWrite);
                arg_desc.to_owned()
            }).collect();

            let arg_encoder = self.metal_device.new_argument_encoder(
                metal::Array::from_owned_slice(&arg_descs)
            );
            let arg_buffer = self.metal_device.new_buffer(
                arg_encoder.encoded_length(),
                metal::MTLResourceOptions::StorageModeShared
            );
            arg_encoder.set_argument_buffer(&arg_buffer, 0);

            for rb in &binders {
                if let Some(ref binding) = rb.bound_resource {
                    let heap = unsafe { &*binding.heap_ptr };
                    encoder.use_heap(&heap.mtl_heap);

                    match rb.data_type {
                        metal::MTLDataType::Texture => {
                            if let Some(texture) = heap.texture_slots.get(binding.offset).and_then(|t| t.as_ref()) {
                                arg_encoder.set_texture(rb.binding_index as u64, texture);
                            }
                        }
                        metal::MTLDataType::Pointer => {
                            if let Some(buffer) = heap.buffer_slots.get(binding.offset).and_then(|b| b.as_ref()) {
                                arg_encoder.set_buffer(rb.binding_index as u64, buffer, 0);
                            }
                        }
                        _ => {}
                    }
                }
            }

            encoder.set_buffer(buffer_index as u64, Some(&arg_buffer), 0);
            self.transient_buffers.push(arg_buffer);
        }

        // clear dirty flags now encoding is done
        for b in self.compute_binder.values_mut() {
            match b {
                PipelineStageBinder::PushConstants(pc) => pc.dirty = false,
                PipelineStageBinder::Resource(rb) => rb.dirty = false,
            }
        }
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        objc::rc::autoreleasepool(|| {
            let cmd = self.cmd_queue.new_command_buffer().to_owned();
            // GPU waits for the previous frame's present signal before executing any work
            if swap_chain.frame_value > 0 {
                cmd.encode_wait_for_event(&swap_chain.frame_event, swap_chain.frame_value);
            }
            self.cmd = Some(cmd);
            self.transient_buffers.clear();
        });
    }

    fn close(&mut self) -> result::Result<(), super::Error> {
        objc::rc::autoreleasepool(|| {
            // close any open compute encoder before committing
            if let Some(enc) = self.compute_encoder.take() {
                enc.end_encoding();
            }
            self.cmd.as_ref().expect("hotline_rs::gfx::mtl expected call to CmdBuf::reset before close").commit();
            self.cmd = None;
            Ok(())
        })
    }

    fn get_backbuffer_index(&self) -> u32 {
        0
    }

    fn begin_render_pass(&mut self, render_pass: &RenderPass) {
        objc::rc::autoreleasepool(|| {
            // catch double begin
            assert!(self.render_encoder.is_none(),
                "hotline_rs::gfx::mtl begin_render_pass called without matching CmdBuf::end_render_pass");

            // close any open compute encoder - Metal forbids two live encoders on one cmd buffer
            if let Some(enc) = self.compute_encoder.take() {
                enc.end_encoding();
            }

            // if a timestamp pair is armed, sample the GPU clock at this encoder's stage boundaries:
            // start_of_vertex = pass start, end_of_fragment = pass end (the inner boundaries are left
            // as MTLCounterDontSample). Set on the descriptor before the encoder is created.
            if let Some((sample_buffer, start)) = self.pending_timestamp.take() {
                if let Some(att) = render_pass.desc.sample_buffer_attachments().object_at(0) {
                    att.set_sample_buffer(&sample_buffer);
                    att.set_start_of_vertex_sample_index(start);
                    att.set_end_of_vertex_sample_index(MTL_COUNTER_DONT_SAMPLE);
                    att.set_start_of_fragment_sample_index(MTL_COUNTER_DONT_SAMPLE);
                    att.set_end_of_fragment_sample_index(start + 1);
                }
            }

            // catch mismatched close/reset
            let render_encoder = self.cmd.as_ref()
                .expect("hotline_rs::gfx::mtl expected call to CmdBuf::reset after close")
                .new_render_command_encoder(&render_pass.desc).to_owned();

            // new encoder
            self.render_encoder = Some(render_encoder);
        });
    }

    fn end_render_pass(&mut self) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder.as_ref()
                .expect("hotline_rs::gfx::mtl end_render_pass called without matching begin")
                .end_encoding();
            self.render_encoder = None;
        });
    }

    fn begin_event(&mut self, colour: u32, name: &str) {
        if let Some(enc) = self.render_encoder.as_ref() {
            enc.push_debug_group(name);
        } else if let Some(enc) = self.compute_encoder.as_ref() {
            enc.push_debug_group(name);
        } else if let Some(cmd) = self.cmd.as_ref() {
            cmd.push_debug_group(name);
        }
    }

    fn end_event(&mut self) {
        if let Some(enc) = self.render_encoder.as_ref() {
            enc.pop_debug_group();
        } else if let Some(enc) = self.compute_encoder.as_ref() {
            enc.pop_debug_group();
        } else if let Some(cmd) = self.cmd.as_ref() {
            cmd.pop_debug_group();
        }
    }

    fn timestamp_query(&mut self, heap: &mut QueryHeap, resolve_buffer: &mut Buffer) {
        let idx = heap.alloc_index;
        heap.alloc_index += 1;
        resolve_buffer.counter_sample_index = idx;
        resolve_buffer.counter_cmd = self.cmd.clone();

        if let Some(sample_buffer) = heap.sample_buffer.as_ref() {
            // counter-sampling path: tag the buffer so read_timestamps resolves the counter, and on
            // the start sample of a pair arm the next encoder to record both stage-boundary samples
            // ([idx, idx + 1]). Multiple start/end pairs in one CB each arm their own encoder.
            resolve_buffer.counter_sample_buffer = Some(sample_buffer.to_owned());
            if idx % 2 == 0 {
                self.pending_timestamp = Some((sample_buffer.to_owned(), idx as NSUInteger));
            }
        }
        else {
            // fallback path: no counter buffer, read GPUStartTime / GPUEndTime of the pass CB.
            resolve_buffer.counter_sample_buffer = None;
        }
    }

    fn begin_query(&mut self, heap: &mut QueryHeap, query_type: QueryType) -> usize {
        0
    }

    fn end_query(&mut self, heap: &mut QueryHeap, query_type: QueryType, index: usize, resolve_buffer: &mut Buffer) {
    }

    fn transition_barrier(&mut self, barrier: &TransitionBarrier<Device>) {
    }

    fn transition_barrier_subresource(&mut self, barrier: &TransitionBarrier<Device>, subresource: Subresource) {
    }

    fn uav_barrier(&mut self, resource: UavResource<Device>) {
        unimplemented!()
    }

    fn set_viewport(&mut self, viewport: &super::Viewport) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
            .set_viewport(MTLViewport {
                originX: viewport.x as f64,
                originY: viewport.y as f64,
                width: viewport.width as f64,
                height: viewport.height as f64,
                znear: viewport.min_depth as f64,
                zfar: viewport.max_depth as f64,
            });
        });
    }

    fn set_scissor_rect(&mut self, scissor_rect: &super::ScissorRect) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
            .set_scissor_rect(MTLScissorRect {
                x: scissor_rect.left as u64,
                y: scissor_rect.top as u64,
                width: (scissor_rect.right - scissor_rect.left) as u64,
                height: (scissor_rect.bottom - scissor_rect.top) as u64,
            });
        });
    }

    fn set_vertex_buffer(&mut self, buffer: &Buffer, slot: u32) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .set_vertex_buffer(slot as NSUInteger, Some(&buffer.metal_buffer), 0);
        });
    }

    fn set_index_buffer(&mut self, buffer: &Buffer) {
        self.bound_index_buffer = Some(buffer.metal_buffer.clone());
        self.bound_index_stride = buffer.element_stride;
    }

    fn set_render_pipeline(&mut self, pipeline: &RenderPipeline) {
        objc::rc::autoreleasepool(|| {
            let encoder = self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

            encoder.set_render_pipeline_state(&pipeline.pipeline_state);

            // Set depth stencil state
            encoder.set_depth_stencil_state(&pipeline.depth_stencil_state);

            // Set rasterizer state
            let raster = &pipeline.raster_info;
            encoder.set_cull_mode(to_mtl_cull_mode(raster.cull_mode));
            encoder.set_front_facing_winding(to_mtl_winding(raster.front_ccw));
            encoder.set_triangle_fill_mode(to_mtl_triangle_fill_mode(raster.fill_mode));
            encoder.set_depth_bias(raster.depth_bias as f32, raster.slope_scaled_depth_bias, raster.depth_bias_clamp);

            // Bind sampler argument buffer at buffer(0) in fragment shader
            if let Some(ref sampler_arg_buffer) = pipeline.sampler_argument_buffer {
                encoder.set_fragment_buffer(
                    0,
                    Some(sampler_arg_buffer),
                    0
                );
            }

            // store pipeline pointer for push_render_constants
            self.bound_render_pipeline = Some(pipeline as *const RenderPipeline);

            // Clone binder templates from pipeline to command buffer
            self.vertex_binder = pipeline.vertex_binder.clone();
            self.fragment_binder = pipeline.fragment_binder.clone();
        });
    }

    fn set_compute_pipeline(&mut self, pipeline: &ComputePipeline) {
        objc::rc::autoreleasepool(|| {
            // open a compute encoder lazily; reused across dispatches until a render pass or close
            if self.compute_encoder.is_none() {
                let cmd = self.cmd.as_ref()
                    .expect("hotline_rs::gfx::mtl expected a call to CmdBuf::reset before set_compute_pipeline");
                // if a timestamp pair is armed, sample the GPU clock at this encoder's boundaries
                let encoder = if let Some((sample_buffer, start)) = self.pending_timestamp.take() {
                    let desc = metal::ComputePassDescriptor::new();
                    if let Some(att) = desc.sample_buffer_attachments().object_at(0) {
                        att.set_sample_buffer(&sample_buffer);
                        att.set_start_of_encoder_sample_index(start);
                        att.set_end_of_encoder_sample_index(start + 1);
                    }
                    cmd.compute_command_encoder_with_descriptor(desc).to_owned()
                }
                else {
                    cmd.new_compute_command_encoder().to_owned()
                };
                self.compute_encoder = Some(encoder);
            }

            self.compute_encoder.as_ref().unwrap()
                .set_compute_pipeline_state(&pipeline.pipeline_state);

            // store pipeline pointer and clone binder template into command buffer state
            self.bound_compute_pipeline = Some(pipeline as *const ComputePipeline);
            self.compute_binder = pipeline.compute_binder.clone();
        });
    }

    fn set_raytracing_pipeline(&mut self, pipeline: &RaytracingPipeline) {
        unimplemented!()
    }

    fn set_heap<T: SuperPipleline>(&mut self, pipeline: &T, heap: &Heap) {
        // compute pipelines bind the heap argument buffers on the compute encoder (single stage)
        if matches!(T::get_pipeline_type(), super::PipelineType::Compute) {
            let encoder = self.compute_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to set_compute_pipeline before set_heap");
            let cp: &ComputePipeline = unsafe { std::mem::transmute(pipeline) };

            encoder.use_heap(&heap.mtl_heap);
            for (_key, slot) in &cp.compute_binder {
                if let PipelineStageBinder::Resource(res) = slot {
                    let arg_buffer = match res.data_type {
                        metal::MTLDataType::Texture => heap.get_texture_argument_buffer(),
                        metal::MTLDataType::Pointer => heap.get_buffer_argument_buffer(),
                        _ => continue,
                    };
                    encoder.set_buffer(res.buffer_index as u64, Some(arg_buffer), 0);
                }
            }
            return;
        }

        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        // Cast pipeline to RenderPipeline to access slot_lookup
        let rp: &RenderPipeline = unsafe { std::mem::transmute(pipeline) };

        // vertex bindings
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Vertex);
        for (key, slot) in &rp.vertex_binder {
            match slot {
                PipelineStageBinder::Resource(res) => {
                    let arg_buffer = match res.data_type {
                        metal::MTLDataType::Texture => heap.get_texture_argument_buffer(),
                        metal::MTLDataType::Pointer => heap.get_buffer_argument_buffer(),
                        _ => continue,
                    };
                    encoder.set_vertex_buffer(res.buffer_index as u64, Some(arg_buffer), 0);
                }
                _ => {}
            }
        }

        // fragment bindings
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Fragment);
        for (key, slot) in &rp.fragment_binder {
            match slot {
                PipelineStageBinder::Resource(res) => {
                    let arg_buffer = match res.data_type {
                        metal::MTLDataType::Texture => heap.get_texture_argument_buffer(),
                        metal::MTLDataType::Pointer => heap.get_buffer_argument_buffer(),
                        _ => continue,
                    };
                    encoder.set_fragment_buffer(res.buffer_index as u64, Some(arg_buffer), 0);
                }
                _ => {}
            }
        }
    }

    fn set_binding<T: SuperPipleline>(&mut self, _pipeline: &T, register: u32, space: u32, descriptor_type: super::DescriptorType, heap: &Heap, offset: usize) -> Option<()> {
        let key: SlotKey = (register, space, descriptor_type);
        let heap_ptr = heap as *const Heap;

        // Write to vertex binder if present
        if let Some(binder) = self.vertex_binder.get_mut(&key) {
            if let PipelineStageBinder::Resource(ref mut rb) = binder {
                rb.bound_resource = Some(ResourceBinding { heap_ptr, offset });
                rb.dirty = true;
            }
        }

        // Write to fragment binder if present
        if let Some(binder) = self.fragment_binder.get_mut(&key) {
            if let PipelineStageBinder::Resource(ref mut rb) = binder {
                rb.bound_resource = Some(ResourceBinding { heap_ptr, offset });
                rb.dirty = true;
            }
        }

        // Write to compute binder if present
        if let Some(binder) = self.compute_binder.get_mut(&key) {
            if let PipelineStageBinder::Resource(ref mut rb) = binder {
                rb.bound_resource = Some(ResourceBinding { heap_ptr, offset });
                rb.dirty = true;
            }
        }

        Some(())
    }

    fn set_marker(&mut self, colour: u32, name: &str) {
    }

    fn push_render_constants<P: SuperPipleline, T: Sized>(&mut self, _pipeline: &P, register: u32, space: u32, num_values: u32, dest_offset: u32, data: &[T]) -> Option<()> {
        let key = (register, space, super::DescriptorType::PushConstants);

        let data_size_dwords = num_values as usize;
        let data_u32 = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u32,
                data_size_dwords
            )
        };

        let mut result = None;

        // Write to vertex binder if matching key found
        if let Some(PipelineStageBinder::PushConstants(ref mut pc)) = self.vertex_binder.get_mut(&key) {
            let dest_start = dest_offset as usize;
            let dest_end = dest_start + data_size_dwords;
            if dest_end <= pc.data.len() {
                pc.data[dest_start..dest_end].copy_from_slice(data_u32);
                pc.dirty = true;
            }
            result = Some(());
        }

        // Write to fragment binder if matching key found
        if let Some(PipelineStageBinder::PushConstants(ref mut pc)) = self.fragment_binder.get_mut(&key) {
            let dest_start = dest_offset as usize;
            let dest_end = dest_start + data_size_dwords;
            if dest_end <= pc.data.len() {
                pc.data[dest_start..dest_end].copy_from_slice(data_u32);
                pc.dirty = true;
            }
            result = Some(());
        }

        result
    }

    fn push_compute_constants<P: SuperPipleline, T: Sized>(&mut self, _pipeline: &P, register: u32, space: u32, num_values: u32, dest_offset: u32, data: &[T]) -> Option<()> {
        let key = (register, space, super::DescriptorType::PushConstants);

        let data_size_dwords = num_values as usize;
        let data_u32 = unsafe {
            std::slice::from_raw_parts(
                data.as_ptr() as *const u32,
                data_size_dwords
            )
        };

        if let Some(PipelineStageBinder::PushConstants(ref mut pc)) = self.compute_binder.get_mut(&key) {
            let dest_start = dest_offset as usize;
            let dest_end = dest_start + data_size_dwords;
            if dest_end <= pc.data.len() {
                pc.data[dest_start..dest_end].copy_from_slice(data_u32);
                pc.dirty = true;
            }
            return Some(());
        }

        None
    }

    fn draw_instanced(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        objc::rc::autoreleasepool(|| {
            self.allocate_stage_resources();

            let primitive_type = self.bound_render_pipeline
                .map(|p| unsafe { (*p).topology })
                .map(to_mtl_primitive_type)
                .unwrap_or(metal::MTLPrimitiveType::Triangle);

            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .draw_primitives_instanced_base_instance(
                    primitive_type,
                    start_vertex as u64,
                    vertex_count as u64,
                    instance_count as u64,
                    start_instance as u64
                );
        });
    }

    fn draw_indexed_instanced(
        &mut self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        objc::rc::autoreleasepool(|| {
            self.allocate_stage_resources();

            let primitive_type = self.bound_render_pipeline
                .map(|p| unsafe { (*p).topology })
                .map(to_mtl_primitive_type)
                .unwrap_or(metal::MTLPrimitiveType::Triangle);

            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .draw_indexed_primitives_instanced_base_instance(
                    primitive_type,
                    index_count as u64,
                    to_mtl_index_type(self.bound_index_stride),
                    &self.bound_index_buffer.as_ref().unwrap(),
                    start_index as u64 * self.bound_index_stride as u64,
                    instance_count as u64,
                    base_vertex as i64,
                    start_instance as u64
                );
        })
    }

    fn dispatch(&mut self, group_count: Size3, numthreads: Size3) {
        objc::rc::autoreleasepool(|| {
            self.allocate_compute_resources();

            let threadgroups = metal::MTLSize::new(
                group_count.x as u64, group_count.y as u64, group_count.z as u64);
            let threads_per_group = metal::MTLSize::new(
                numthreads.x as u64, numthreads.y as u64, numthreads.z as u64);

            self.compute_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to set_compute_pipeline before dispatch")
                .dispatch_thread_groups(threadgroups, threads_per_group);
        });
    }

    fn execute_indirect(
        &mut self,
        command: &CommandSignature,
        max_command_count: u32,
        argument_buffer: &Buffer,
        argument_buffer_offset: usize,
        counter_buffer: Option<&Buffer>,
        counter_buffer_offset: usize
    ) {
    }

    fn read_back_backbuffer(&mut self, swap_chain: &SwapChain) -> result::Result<ReadBackRequest, super::Error> {
        Ok(ReadBackRequest {

        })
    }

    fn resolve_texture_subresource(&mut self, texture: &Texture, _subresource: u32) -> result::Result<(), super::Error> {
        // Record the resolve as deferred barrier work; Device::execute replays it into a fresh
        // command buffer each frame (a committed Metal command buffer can't be re-submitted).
        if let Some(resolve) = texture.resolved_texture.as_ref() {
            self.deferred_ops.push(DeferredBarrierOp::Resolve {
                msaa: texture.metal_texture.to_owned(),
                resolve: resolve.to_owned(),
            });
        }
        Ok(())
    }

    fn generate_mip_maps(&mut self, texture: &Texture, _device: &Device, _heap: &Heap) -> result::Result<(), super::Error> {
        // Record mip generation as deferred barrier work (replayed per-frame by Device::execute).
        // Generate on the texture shaders actually sample: the resolve backing for an MSAA target
        // (its mip 0 is filled by the preceding resolve op), otherwise the texture itself.
        let target = texture.resolved_texture.as_ref().unwrap_or(&texture.metal_texture);
        if target.mipmap_level_count() > 1 {
            self.deferred_ops.push(DeferredBarrierOp::GenerateMips {
                texture: target.to_owned(),
            });
        }
        Ok(())
    }

    fn copy_buffer_region(
        &mut self,
        dst_buffer: &Buffer,
        dst_offset: usize,
        src_buffer: &Buffer,
        src_offset: usize,
        num_bytes: usize
    ) {
    }

    fn copy_texture_region(
        &mut self,
        dst_texture: &Texture,
        subresource_index: u32,
        dst_x: u32,
        dst_y: u32,
        dst_z: u32,
        src_texture: &Texture,
        src_region: Option<Region>
    ) {
    }

    fn dispatch_rays(&mut self, sbt: &RaytracingShaderBindingTable, numthreads: Size3) {
        unimplemented!()
    }

    fn update_raytracing_tlas(&mut self, tlas: &RaytracingTLAS, instance_buffer: &Buffer, instance_count: usize, mode: AccelerationStructureRebuildMode) {
        unimplemented!()
    }
}

#[derive(Clone)]
pub struct Buffer {
    metal_buffer: metal::Buffer,
    element_stride: usize,
    srv_index: Option<usize>,
    uav_index: Option<usize>,
    cbv_index: Option<usize>,
    counter_sample_buffer: Option<metal::CounterSampleBuffer>,
    counter_sample_index: usize,
    // Metal substitute for a D3D12 GPU fence: wait_until_completed before resolving counter data
    counter_cmd: Option<metal::CommandBuffer>,
}

impl super::Buffer<Device> for Buffer {
    fn update<T: Sized>(&mut self, offset: usize, data: &[T]) -> result::Result<(), super::Error> {
        unsafe {
            let data_ptr = self.metal_buffer.contents() as *mut u8;
            let dest_ptr = data_ptr.add(offset);
            let byte_len = data.len() * std::mem::size_of::<T>();
            std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, dest_ptr, byte_len);
        }
        Ok(())
    }

    fn write<T: Sized>(&mut self, offset: usize, data: &[T]) -> result::Result<(), super::Error> {
        self.update(offset, data)
    }

    fn get_cbv_index(&self) -> Option<usize> {
        self.cbv_index
    }

    fn get_srv_index(&self) -> Option<usize> {
        self.srv_index
    }

    fn get_uav_index(&self) -> Option<usize> {
        self.uav_index
    }

    fn get_vbv(&self) -> Option<VertexBufferView> {
        None
    }

    fn get_ibv(&self) -> Option<IndexBufferView> {
        None
    }

    fn get_counter_offset(&self) -> Option<usize> {
        None
    }

    fn map(&mut self, info: &MapInfo) -> *mut u8 {
        std::ptr::null_mut()
    }

    fn unmap(&mut self, info: &UnmapInfo) {
    }
}

pub struct Shader {
    lib: metal::Library,
    data: *const u8,
    data_size: usize
}

impl super::Shader<Device> for Shader {}

struct MetalSamplerBinding {
    slot: u32,
    sampler: metal::SamplerState
}

/// Push constants binder - uses setVertexBytes/setFragmentBytes for zero-allocation binding
#[derive(Clone)]
struct PushConstantsBinder {
    pub data: Vec<u32>,
    pub num_32_bit_constants: u32,
    pub buffer_index: u32,
    pub dirty: bool,
}

#[derive(Clone, Copy)]
struct ResourceBinding {
    pub heap_ptr: *const Heap,
    pub offset: usize,
}

#[derive(Clone)]
struct ResourceBinder {
    pub buffer_index: u32,
    pub binding_index: u32,
    pub data_type: metal::MTLDataType,
    pub array_length: u64,
    pub bound_resource: Option<ResourceBinding>,
    pub dirty: bool,
}

#[derive(Clone)]
enum PipelineStageBinder {
    PushConstants(PushConstantsBinder),
    Resource(ResourceBinder),
}

/// Key for slot lookup: (register, space, descriptor_type)
type SlotKey = (u32, u32, DescriptorType);

pub struct RenderPipeline {
    pipeline_state: metal::RenderPipelineState,
    slots: Vec<u32>,
    /// Primitive topology for draw calls
    topology: Topology,
    /// Unified slot lookup by (register, space, descriptor_type)
    slot_lookup: HashMap<SlotKey, PipelineSlotInfo>,
    /// Static samplers
    static_samplers: Vec<MetalSamplerBinding>,
    /// Sampler argument buffer
    sampler_argument_buffer: Option<metal::Buffer>,
    /// Vertex stage binders for push constants, keyed by (register, space, descriptor_type)
    vertex_binder: HashMap<SlotKey, PipelineStageBinder>,
    /// Fragment stage binders for push constants, keyed by (register, space, descriptor_type)
    fragment_binder: HashMap<SlotKey, PipelineStageBinder>,
    /// Depth stencil state
    depth_stencil_state: metal::DepthStencilState,
    /// Rasterizer state (applied dynamically on encoder in Metal)
    raster_info: super::RasterInfo,
}

impl super::RenderPipeline<Device> for RenderPipeline {}

impl super::Pipeline for RenderPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        self.slot_lookup.get(&(register, space, descriptor_type))
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Render
    }

    fn get_sub_binding_offset(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> u32 {
        self.slot_lookup
            .get(&(register, space, descriptor_type))
            .map(|s| s.sub_offset)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
pub struct Texture {
    metal_texture: metal::Texture,
    /// Single-sample resolve backing for an MSAA texture (samples > 1); also the texture sampled
    /// when reading a resolvable target normally
    resolved_texture: Option<metal::Texture>,
    /// Bindless index of the resolved / non-MSAA view (returned by `get_srv_index`)
    srv_index: Option<usize>,
    /// Bindless index of the MSAA view, for `Texture2DMS` reads (returned by `get_msaa_srv_index`)
    msaa_srv_index: Option<usize>,
    uav_index: Option<usize>,
    resolvable: bool,
    heap_id: Option<u16>
}

impl super::Texture<Device> for Texture {
    fn get_srv_index(&self) -> Option<usize> {
        self.srv_index
    }

    fn get_subresource_uav_index(&self, subresource: u32) -> Option<usize> {
        None
    }

    fn get_msaa_srv_index(&self) -> Option<usize> {
        self.msaa_srv_index
    }

    fn get_uav_index(&self) -> Option<usize> {
        self.uav_index
    }

    fn clone_inner(&self) -> Texture {
        Texture {
            metal_texture: self.metal_texture.clone(),
            resolved_texture: self.resolved_texture.clone(),
            srv_index: self.srv_index,
            msaa_srv_index: self.msaa_srv_index,
            uav_index: self.uav_index,
            resolvable: self.resolvable,
            heap_id: self.heap_id
        }
    }

    fn is_resolvable(&self) -> bool {
        self.resolvable
    }

    fn get_shader_heap_id(&self) -> Option<u16> {
        self.heap_id
    }
}

#[derive(Clone)]
pub struct Sampler {
    mtl_sampler: metal::SamplerState
}

pub struct ReadBackRequest {

}

impl super::ReadBackRequest<Device> for ReadBackRequest {
    fn is_complete(&self, swap_chain: &SwapChain) -> bool {
        false
    }

    fn map(&self, info: &MapInfo) -> result::Result<ReadBackData, super::Error> {
        Err(super::Error {
            msg: format!(
                "not implemented",
            ),
        })
    }

    fn unmap(&self) {
    }
}

#[derive(Clone)]
pub struct RenderPass {
    desc: metal::RenderPassDescriptor,
    /// Colour attachment formats, one per MRT target (index 0 = SV_Target0)
    pixel_formats: Vec<metal::MTLPixelFormat>,
    depth_format: Option<metal::MTLPixelFormat>,
    /// MSAA sample count shared by all attachments in the pass (1 = no MSAA)
    sample_count: u32,
}

impl super::RenderPass<Device> for RenderPass {
    fn get_format_hash(&self) -> u64 {
        0
    }
}

pub struct ComputePipeline {
    pipeline_state: metal::ComputePipelineState,
    slots: Vec<u32>,
    /// Unified slot lookup by (register, space, descriptor_type)
    slot_lookup: HashMap<SlotKey, PipelineSlotInfo>,
    /// Single-stage binders for push constants and resource bindings
    compute_binder: HashMap<SlotKey, PipelineStageBinder>,
}

impl super::Pipeline for ComputePipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        self.slot_lookup.get(&(register, space, descriptor_type))
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Compute
    }

    fn get_sub_binding_offset(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> u32 {
        self.slot_lookup
            .get(&(register, space, descriptor_type))
            .map(|s| s.sub_offset)
            .unwrap_or(0)
    }
}

#[derive(Clone)]
enum HeapResourceType {
    None,
    Texture,
    Buffer
}

#[derive(Clone)]
pub struct Heap {
    mtl_heap: metal::Heap,
    texture_slots: Vec<Option<metal::Texture>>,
    buffer_slots: Vec<Option<metal::Buffer>>,
    resource_type: Vec<HeapResourceType>,
    offset: usize,
    id: u16,
    /// Argument encoder for bindless texture access (pre-encodes all textures)
    texture_argument_encoder: metal::ArgumentEncoder,
    /// Pre-encoded argument buffer containing all texture references
    texture_argument_buffer: metal::Buffer,
    /// Argument encoder for bindless buffer access
    buffer_argument_encoder: metal::ArgumentEncoder,
    /// Pre-encoded argument buffer containing all buffer references
    buffer_argument_buffer: metal::Buffer,
}

impl Heap {
    fn allocate(&mut self) -> usize {
        let srv = self.offset;
        self.offset += 1;
        unsafe {
            self.texture_slots.resize(self.offset, None);
            self.buffer_slots.resize(self.offset, None);
        }
        self.resource_type.resize(self.offset, HeapResourceType::None);
        srv
    }

    /// Encode a texture into the heap's argument buffer at the given index (for bindless)
    fn encode_texture(&self, index: usize, texture: &metal::Texture) {
        self.texture_argument_encoder.set_argument_buffer(&self.texture_argument_buffer, 0);
        self.texture_argument_encoder.set_texture(index as u64, texture);
    }

    /// Encode a buffer into the heap's argument buffer at the given index (for bindless)
    fn encode_buffer(&self, index: usize, buffer: &metal::Buffer) {
        self.buffer_argument_encoder.set_argument_buffer(&self.buffer_argument_buffer, 0);
        self.buffer_argument_encoder.set_buffer(index as u64, buffer, 0);
    }

    /// Get the pre-encoded texture argument buffer for binding
    pub fn get_texture_argument_buffer(&self) -> &metal::Buffer {
        &self.texture_argument_buffer
    }

    /// Get the pre-encoded buffer argument buffer for binding
    pub fn get_buffer_argument_buffer(&self) -> &metal::Buffer {
        &self.buffer_argument_buffer
    }
}

impl super::Heap<Device> for Heap {
    fn deallocate(&mut self, index: usize) {

    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &SwapChain) {
    }

    fn get_heap_id(&self) -> u16 {
        self.id
    }
}

pub struct QueryHeap {
    heap_type: super::QueryType,
    sample_buffer: Option<metal::CounterSampleBuffer>,
    alloc_index: usize,
    capacity: usize,
}

impl super::QueryHeap<Device> for QueryHeap {
    fn reset(&mut self) {
        self.alloc_index = 0;
    }
}

pub struct CommandSignature {

}

pub struct RaytracingPipeline {

}

pub struct RaytracingShaderBindingTable {

}

pub struct RaytracingBLAS {

}

pub struct RaytracingTLAS {

}

impl Device {
    /// Largest texture sample count <= `requested` that this device supports (always >= 1).
    /// Apple GPUs commonly cap at 4x, so an 8x request is clamped down rather than asserting.
    fn supported_sample_count(&self, requested: u32) -> u32 {
        let mut count = requested.max(1);
        while count > 1 && !self.metal_device.supports_texture_sample_count(count as NSUInteger) {
            count /= 2;
        }
        count
    }

    fn create_render_pass_for_swap_chain(
        &self,
        texture: &Texture,
        clear_col: Option<ClearColour>
    ) -> RenderPass {
        objc::rc::autoreleasepool(|| {
            self.create_render_pass(&RenderPassInfo {
                render_targets: vec![texture],
                rt_clear: clear_col,
                depth_stencil: None,
                ds_clear: None,
                resolve: false,
                discard: false,
                array_slice: 0
            }).unwrap()
        })
    }

    fn create_heap_mtl(mtl_device: &metal::Device, info: &HeapInfo, id: u16) -> Heap {
            // hmm?
            let texture_descriptor = TextureDescriptor::new();
            texture_descriptor.set_width(512);
            texture_descriptor.set_height(512);
            texture_descriptor.set_depth(1);
            texture_descriptor.set_texture_type(metal::MTLTextureType::D2);
            texture_descriptor.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm);
            // Private storage: required for MSAA textures (which can't be Shared) and faster for
            // GPU sampling on Apple Silicon. Texture data is uploaded via a staging buffer + blit.
            texture_descriptor.set_storage_mode(metal::MTLStorageMode::Private);

            // Determine the size required for the heap for the given descriptor
            let size_and_align = mtl_device.heap_texture_size_and_align(&texture_descriptor);
            let texture_size = align_pow2(size_and_align.size, size_and_align.align);

            // The 512x512 RGBA8 reference (~1MB) underestimates real descriptors: 2k material
            // textures, IBL cubemaps and MSAA render targets are far larger. Oversize the heap so
            // the bindless descriptor pool doesn't run out of memory when many/large textures load.
            const HEAP_OVERSIZE_FACTOR: u64 = 2;
            let heap_size = texture_size * info.num_descriptors.max(1) as u64 * HEAP_OVERSIZE_FACTOR;

            let heap_descriptor = metal::HeapDescriptor::new();
            heap_descriptor.set_storage_mode(metal::MTLStorageMode::Private);
            heap_descriptor.set_size(heap_size);

            // Enable hazard tracking so Metal automatically synchronizes heap-allocated
            // textures across command buffers (by default heaps are MTLHazardTrackingModeUntracked)
            unsafe { let _: () = msg_send![&*heap_descriptor, setHazardTrackingMode: metal::MTLHazardTrackingMode::Tracked]; };

            let heap = mtl_device.new_heap(&heap_descriptor);

            // Create texture argument encoder for bindless access
            let max_resources = info.num_descriptors.max(1) as u64;
            let tex_arg_desc = metal::ArgumentDescriptor::new();
            tex_arg_desc.set_index(0);
            tex_arg_desc.set_data_type(metal::MTLDataType::Texture);
            tex_arg_desc.set_array_length(max_resources);
            tex_arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);

            let texture_argument_encoder = mtl_device.new_argument_encoder(
                metal::Array::from_owned_slice(&[tex_arg_desc.to_owned()])
            );
            let texture_argument_buffer = mtl_device.new_buffer(
                texture_argument_encoder.encoded_length(),
                metal::MTLResourceOptions::StorageModeShared
            );

            // Create buffer argument encoder for bindless access
            let buf_arg_desc = metal::ArgumentDescriptor::new();
            buf_arg_desc.set_index(0);
            buf_arg_desc.set_data_type(metal::MTLDataType::Pointer);
            buf_arg_desc.set_array_length(max_resources);
            buf_arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);

            let buffer_argument_encoder = mtl_device.new_argument_encoder(
                metal::Array::from_owned_slice(&[buf_arg_desc.to_owned()])
            );
            let buffer_argument_buffer = mtl_device.new_buffer(
                buffer_argument_encoder.encoded_length(),
                metal::MTLResourceOptions::StorageModeShared
            );

        Heap {
            mtl_heap: heap,
            texture_slots: Vec::new(),
            buffer_slots: Vec::new(),
            resource_type: Vec::new(),
            offset: 0,
            id,
            texture_argument_encoder,
            texture_argument_buffer,
            buffer_argument_encoder,
            buffer_argument_buffer,
        }
    }

    /// Build unified slot lookup
    fn build_slot_lookup(
        &self,
        pipeline_bindings: &Option<Vec<DescriptorBinding>>,
        pipeline_push_constants: &Option<Vec<PushConstantInfo>>,
    ) -> HashMap<SlotKey, PipelineSlotInfo> {
        let mut slot_lookup: HashMap<SlotKey, PipelineSlotInfo> = HashMap::new();

        // hardcoded sampler offsets
        let vertex_samplers_offset: u32 = 2;
        let fragment_samplers_offset: u32 = 0;
        let mut vertex_binding_offset: u32 = vertex_samplers_offset + 1;
        let mut fragment_binding_offset: u32 = fragment_samplers_offset + 1;

        // Add push constant slots first (they come before regular bindings in htwv)
        if let Some(push_constants) = pipeline_push_constants.as_ref() {
            for push_constant in push_constants {
                // Determine stage indices based on visibility, using per-stage offsets
                let (vertex_idx, fragment_idx, canonical_index) = match push_constant.visibility {
                    ShaderVisibility::Vertex => {
                        let idx = vertex_binding_offset;
                        vertex_binding_offset += 1;
                        (Some(idx), None, idx)
                    },
                    ShaderVisibility::Fragment => {
                        let idx = fragment_binding_offset;
                        fragment_binding_offset += 1;
                        (None, Some(idx), idx)
                    },
                    ShaderVisibility::All => {
                        let v_idx = vertex_binding_offset;
                        let f_idx = fragment_binding_offset;
                        vertex_binding_offset += 1;
                        fragment_binding_offset += 1;
                        // Use vertex index as canonical for lookup
                        (Some(v_idx), Some(f_idx), v_idx)
                    },
                    _ => (None, None, 0),
                };

                slot_lookup.insert(
                    (push_constant.shader_register, push_constant.register_space, DescriptorType::PushConstants),
                    PipelineSlotInfo {
                        index: canonical_index,
                        count: Some(push_constant.num_values),
                        sub_offset: 0,
                    },
                );
            }
        }

        // Add regular binding slots, grouped by (register_kind, shader_register) to mirror the
        // descriptor-set layout produced by htwv's MSL codegen. Each (kind, register) becomes its
        // own MSL [[buffer(N)]] slot so the heap's texture and buffer argument buffers never share
        // a slot. sub_offset matches the [[id(N)]] value spirv-cross assigns within the set;
        // callers compensate for the unsized-array-hack offset via get_sub_binding_offset.
        if let Some(bindings) = pipeline_bindings.as_ref() {
            if !bindings.is_empty() {
                // (register_kind, shader_register) -> (buffer_index, next_sub_offset)
                let mut v_groups: HashMap<(char, u32), (u32, u32)> = HashMap::new();
                let mut f_groups: HashMap<(char, u32), (u32, u32)> = HashMap::new();

                for binding in bindings {
                    let key = (descriptor_register_kind(binding.binding_type), binding.shader_register);

                    let v_slot = if matches!(binding.visibility, ShaderVisibility::Vertex | ShaderVisibility::All) {
                        let entry = v_groups.entry(key).or_insert_with(|| {
                            let idx = vertex_binding_offset;
                            vertex_binding_offset += 1;
                            (idx, 0)
                        });
                        let sub = entry.1;
                        entry.1 += 1;
                        Some((entry.0, sub))
                    } else { None };

                    let f_slot = if matches!(binding.visibility, ShaderVisibility::Fragment | ShaderVisibility::All) {
                        let entry = f_groups.entry(key).or_insert_with(|| {
                            let idx = fragment_binding_offset;
                            fragment_binding_offset += 1;
                            (idx, 0)
                        });
                        let sub = entry.1;
                        entry.1 += 1;
                        Some((entry.0, sub))
                    } else { None };

                    let (canonical_index, sub_offset) = v_slot.or(f_slot).unwrap_or((0, 0));
                    slot_lookup.insert(
                        (binding.shader_register, binding.register_space, binding.binding_type),
                        PipelineSlotInfo {
                            index: canonical_index,
                            count: binding.num_descriptors,
                            sub_offset,
                        }
                    );
                }
            }
        }

        slot_lookup
    }

    fn build_stage_binders(
        &self,
        pipeline_bindings: &Option<Vec<DescriptorBinding>>,
        pipeline_push_constants: &Option<Vec<PushConstantInfo>>,
    ) -> (HashMap<SlotKey, PipelineStageBinder>, HashMap<SlotKey, PipelineStageBinder>) {
        const MAX_BINDLESS_TEXTURES: u64 = 1024;

        let mut vertex_binder: HashMap<SlotKey, PipelineStageBinder> = HashMap::new();
        let mut fragment_binder: HashMap<SlotKey, PipelineStageBinder> = HashMap::new();

        let vertex_samplers_offset: u32 = 2;
        let fragment_samplers_offset: u32 = 0;

        let mut vertex_binding_offset: u32 = vertex_samplers_offset + 1;
        let mut fragment_binding_offset: u32 = fragment_samplers_offset + 1;

        // Add push constant binders (no ArgumentEncoder needed - uses setVertexBytes/setFragmentBytes)
        if let Some(push_constants) = pipeline_push_constants.as_ref() {
            for push_constant in push_constants {
                let key: SlotKey = (
                    push_constant.shader_register,
                    push_constant.register_space,
                    DescriptorType::PushConstants
                );

                match push_constant.visibility {
                    ShaderVisibility::Vertex => {
                        let buffer_index = vertex_binding_offset;
                        vertex_binding_offset += 1;

                        vertex_binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                            data: vec![0u32; push_constant.num_values as usize],
                            num_32_bit_constants: push_constant.num_values,
                            buffer_index,
                            dirty: true,
                        }));
                    },
                    ShaderVisibility::Fragment => {
                        let buffer_index = fragment_binding_offset;
                        fragment_binding_offset += 1;

                        fragment_binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                            data: vec![0u32; push_constant.num_values as usize],
                            num_32_bit_constants: push_constant.num_values,
                            buffer_index,
                            dirty: true,
                        }));
                    },
                    ShaderVisibility::All => {
                        let v_buffer_index = vertex_binding_offset;
                        let f_buffer_index = fragment_binding_offset;
                        vertex_binding_offset += 1;
                        fragment_binding_offset += 1;

                        vertex_binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                            data: vec![0u32; push_constant.num_values as usize],
                            num_32_bit_constants: push_constant.num_values,
                            buffer_index: v_buffer_index,
                            dirty: true,
                        }));

                        fragment_binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                            data: vec![0u32; push_constant.num_values as usize],
                            num_32_bit_constants: push_constant.num_values,
                            buffer_index: f_buffer_index,
                            dirty: true,
                        }));
                    },
                    _ => {},
                }
            }
        }

        // Add resource binders, grouped by (register_kind, shader_register). Each (kind, register)
        // pair gets its own [[buffer(N)]] slot per stage so the heap's texture and buffer
        // argument buffers are bound to distinct slots. Within a group, binding_index is the
        // sub_offset that matches the [[id(N)]] value spirv-cross emits in the MSL set struct.
        if let Some(bindings) = pipeline_bindings.as_ref() {
            if !bindings.is_empty() {
                // (register_kind, shader_register) -> (buffer_index, next_sub_offset)
                let mut v_groups: HashMap<(char, u32), (u32, u32)> = HashMap::new();
                let mut f_groups: HashMap<(char, u32), (u32, u32)> = HashMap::new();

                for binding in bindings {
                    let key: SlotKey = (binding.shader_register, binding.register_space, binding.binding_type);
                    let group_key = (descriptor_register_kind(binding.binding_type), binding.shader_register);
                    let data_type = to_mtl_data_type(
                        binding.resource_type.expect("hotline_rs::gfx::mtl: requires resource type for binding")
                    );
                    let array_length = binding.num_descriptors.map(|n| n as u64).unwrap_or(MAX_BINDLESS_TEXTURES);

                    if matches!(binding.visibility, ShaderVisibility::Vertex | ShaderVisibility::All) {
                        let entry = v_groups.entry(group_key).or_insert_with(|| {
                            let idx = vertex_binding_offset;
                            vertex_binding_offset += 1;
                            (idx, 0)
                        });
                        let sub = entry.1;
                        entry.1 += 1;
                        vertex_binder.insert(key, PipelineStageBinder::Resource(ResourceBinder {
                            buffer_index: entry.0,
                            binding_index: sub,
                            data_type,
                            array_length,
                            bound_resource: None,
                            dirty: true,
                        }));
                    }

                    if matches!(binding.visibility, ShaderVisibility::Fragment | ShaderVisibility::All) {
                        let entry = f_groups.entry(group_key).or_insert_with(|| {
                            let idx = fragment_binding_offset;
                            fragment_binding_offset += 1;
                            (idx, 0)
                        });
                        let sub = entry.1;
                        entry.1 += 1;
                        fragment_binder.insert(key, PipelineStageBinder::Resource(ResourceBinder {
                            buffer_index: entry.0,
                            binding_index: sub,
                            data_type,
                            array_length,
                            bound_resource: None,
                            dirty: true,
                        }));
                    }
                }
            }
        }

        (vertex_binder, fragment_binder)
    }

    /// Build a single-stage binder for a compute pipeline. Mirrors `build_stage_binders` but emits
    /// one map: push constants and resource bindings share a single MSL [[buffer(N)]] namespace
    /// (no vertex/fragment split). Buffer indices begin at `COMPUTE_BINDING_BASE` which must match
    /// the [[buffer(N)]] slots htwv emits for the compute kernel.
    fn build_compute_binder(
        &self,
        pipeline_bindings: &Option<Vec<DescriptorBinding>>,
        pipeline_push_constants: &Option<Vec<PushConstantInfo>>,
    ) -> HashMap<SlotKey, PipelineStageBinder> {
        const MAX_BINDLESS_TEXTURES: u64 = 1024;
        // Compute follows the same MSL [[buffer(N)]] layout htwv emits for the fragment stage:
        // buffer(0) is reserved for the sampler descriptor set, push constants take buffer(1), and
        // space0 resource descriptor sets follow at buffer(2)+. So start binding indices at 1.
        const COMPUTE_BINDING_BASE: u32 = 1;

        let mut binder: HashMap<SlotKey, PipelineStageBinder> = HashMap::new();
        let mut binding_offset: u32 = COMPUTE_BINDING_BASE;

        // Push constants (use setBytes - no ArgumentEncoder)
        if let Some(push_constants) = pipeline_push_constants.as_ref() {
            for push_constant in push_constants {
                let key: SlotKey = (
                    push_constant.shader_register,
                    push_constant.register_space,
                    DescriptorType::PushConstants,
                );
                let buffer_index = binding_offset;
                binding_offset += 1;
                binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                    data: vec![0u32; push_constant.num_values as usize],
                    num_32_bit_constants: push_constant.num_values,
                    buffer_index,
                    dirty: true,
                }));
            }
        }

        // Resource bindings grouped by (register_kind, shader_register) - one [[buffer(N)]] per group
        if let Some(bindings) = pipeline_bindings.as_ref() {
            if !bindings.is_empty() {
                let mut groups: HashMap<(char, u32), (u32, u32)> = HashMap::new();
                for binding in bindings {
                    let key: SlotKey = (binding.shader_register, binding.register_space, binding.binding_type);
                    let group_key = (descriptor_register_kind(binding.binding_type), binding.shader_register);
                    let data_type = to_mtl_data_type(
                        binding.resource_type.expect("hotline_rs::gfx::mtl: requires resource type for binding")
                    );
                    let array_length = binding.num_descriptors.map(|n| n as u64).unwrap_or(MAX_BINDLESS_TEXTURES);

                    let entry = groups.entry(group_key).or_insert_with(|| {
                        let idx = binding_offset;
                        binding_offset += 1;
                        (idx, 0)
                    });
                    let sub = entry.1;
                    entry.1 += 1;
                    binder.insert(key, PipelineStageBinder::Resource(ResourceBinder {
                        buffer_index: entry.0,
                        binding_index: sub,
                        data_type,
                        array_length,
                        bound_resource: None,
                        dirty: true,
                    }));
                }
            }
        }

        binder
    }
}

impl super::Device for Device {
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Buffer = Buffer;
    type Shader = Shader;
    type RenderPipeline = RenderPipeline;
    type Texture = Texture;
    type ReadBackRequest = ReadBackRequest;
    type RenderPass = RenderPass;
    type ComputePipeline = ComputePipeline;
    type Heap = Heap;
    type QueryHeap = QueryHeap;
    type CommandSignature = CommandSignature;
    type RaytracingPipeline = RaytracingPipeline;
    type RaytracingShaderBindingTable = RaytracingShaderBindingTable;
    type RaytracingBLAS = RaytracingBLAS;
    type RaytracingTLAS = RaytracingTLAS;

    fn create(info: &super::DeviceInfo) -> Device {
        objc::rc::autoreleasepool(|| {
            let device = metal::Device::system_default()
                .expect("hotline_rs::gfx::mtl: failed to create metal device");
            let command_queue = device.new_command_queue();

            // adapter info
            let adapter_info = AdapterInfo {
                name: device.name().to_string(),
                description: "Metal".to_string(),
                dedicated_video_memory: device.recommended_max_working_set_size() as usize,
                dedicated_system_memory: 0,
                shared_system_memory: 0,
                available: vec![device.name().to_string()]
            };

            // feature info
            let tier = device.argument_buffers_support();
            assert_eq!(metal::MTLArgumentBuffersTier::Tier2, tier);

            // Can the GPU sample timestamp counters at encoder stage boundaries? (Apple Silicon
            // typically can; older/other GPUs may not, in which case we fall back to whole-CB times.)
            let supports_stage_boundary_timestamps: bool = unsafe {
                msg_send![&*device, supportsCounterSampling: MTL_COUNTER_SAMPLING_POINT_AT_STAGE_BOUNDARY]
            };

            Device {
                command_queue: command_queue,
                shader_heap: Self::create_heap_mtl(&device, &HeapInfo{
                    heap_type: HeapType::Shader,
                    num_descriptors: info.shader_heap_size,
                    debug_name: Some("mtl device: shader heap".to_string())
                }, 1),
                adapter_info: adapter_info,
                metal_device: device,
                heap_alloc_id: 2,
                supports_stage_boundary_timestamps,
            }
       })
    }

    fn get_feature_flags(&self) -> &DeviceFeatureFlags {
        unimplemented!()
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Heap {
        let id = self.heap_alloc_id;
        self.heap_alloc_id += 1;
        Self::create_heap_mtl(&self.metal_device, &info, id)
    }

    fn create_query_heap(&self, info: &QueryHeapInfo) -> QueryHeap {
        let sample_buffer = if info.heap_type == super::QueryType::Timestamp
            && self.supports_stage_boundary_timestamps {
            let counter_sets = self.metal_device.counter_sets();
            let ts_set = counter_sets.iter().find(|cs| cs.name().eq_ignore_ascii_case("timestamp"));
            ts_set.and_then(|cs| {
                let desc = metal::CounterSampleBufferDescriptor::new();
                desc.set_counter_set(cs);
                desc.set_sample_count(info.num_queries as _);
                desc.set_storage_mode(metal::MTLStorageMode::Shared);
                self.metal_device.new_counter_sample_buffer_with_descriptor(&desc).ok()
            })
        } else {
            None
        };
        QueryHeap {
            heap_type: info.heap_type,
            sample_buffer,
            alloc_index: 0,
            capacity: info.num_queries,
        }
    }

    fn create_swap_chain<A: os::App>(
        &mut self,
        info: &super::SwapChainInfo,
        win: &A::Window,
    ) -> result::Result<SwapChain, super::Error> {
        unsafe {
            objc::rc::autoreleasepool(|| {
                // layer
                let layer = metal::MetalLayer::new();
                layer.set_device(&self.metal_device);
                layer.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
                layer.set_presents_with_transaction(false);

                // view
                let macos_win = std::mem::transmute::<&A::Window, &os::macos::Window>(win);
                let view = os::macos::nsview_from_window(macos_win);
                view.setWantsLayer(objc::runtime::YES);
                view.setLayer(std::mem::transmute(layer.as_ref()));

                let draw_size = win.get_size();
                layer.set_contents_scale(win.get_dpi_scale() as f64);
                layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

                let drawable = layer.next_drawable()
                    .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

                let backbuffer_texture = Texture {
                    metal_texture: drawable.texture().to_owned(),
                    resolved_texture: None,
                    srv_index: None,
                    msaa_srv_index: None,
                    uav_index: None,
                    resolvable: false,
                    heap_id: None
                };
                let render_pass = self.create_render_pass_for_swap_chain(&backbuffer_texture, info.clear_colour);
                let render_pass_no_clear = self.create_render_pass_for_swap_chain(&backbuffer_texture, None);

                // create swap chain object
                Ok(SwapChain {
                    layer: layer.clone(),
                    view: view,
                    drawable: drawable.to_owned(),
                    backbuffer_clear: info.clear_colour,
                    backbuffer_texture: backbuffer_texture,
                    backbuffer_pass: render_pass,
                    backbuffer_pass_no_clear: render_pass_no_clear,
                    num_buffers: info.num_buffers,
                    frame_event: self.metal_device.new_event(),
                    frame_value: 0,
                    in_flight: std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
                })
            })
        }
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> CmdBuf {
        objc::rc::autoreleasepool(|| {
            let cmd_queue = self.command_queue.clone();
            let cmd = cmd_queue.new_command_buffer().to_owned();

            CmdBuf {
                cmd_queue,
                cmd: Some(cmd),
                render_encoder: None,
                compute_encoder: None,
                bound_index_buffer: None,
                bound_index_stride: 0,
                bound_render_pipeline: None,
                bound_compute_pipeline: None,
                metal_device: self.metal_device.clone(),
                transient_buffers: Vec::new(),
                vertex_binder: HashMap::new(),
                fragment_binder: HashMap::new(),
                compute_binder: HashMap::new(),
                deferred_ops: Vec::new(),
                pending_timestamp: None,
            }
        })
    }

    fn create_render_pipeline(
        &self,
        info: &super::RenderPipelineInfo<Device>,
    ) -> result::Result<RenderPipeline, super::Error> {
        objc::rc::autoreleasepool(|| {
            let pipeline_state_descriptor = metal::RenderPipelineDescriptor::new();

            if let Some(vs) = info.vs {
                unsafe {
                    let lib = self.metal_device.new_library_with_data(std::slice::from_raw_parts(vs.data, vs.data_size))?;
                    let name = &lib.function_names()[0];
                    let vvs = lib.get_function(name, None).unwrap();
                    pipeline_state_descriptor.set_vertex_function(Some(&vvs));
                }
            };
            if let Some(fs) = info.fs {
                unsafe {
                    let lib = self.metal_device.new_library_with_data(std::slice::from_raw_parts(fs.data, fs.data_size))?;
                    let name = &lib.function_names()[0];
                    let pps = lib.get_function(name, None).unwrap();
                    pipeline_state_descriptor.set_fragment_function(Some(&pps));
                }
            };

            // vertex attribs
            let vertex_desc = metal::VertexDescriptor::new();
            let mut attrib_index = 0;

            // track stride, step function, and step rate per slot
            struct SlotLayout {
                stride: u32,
                input_slot_class: super::InputSlotClass,
                step_rate: u32,
            }
            let mut slot_layouts: Vec<Option<SlotLayout>> = Vec::new();
            for element in &info.input_layout {
                let slot = element.input_slot as usize;
                if slot_layouts.len() <= slot {
                    slot_layouts.resize_with(slot + 1, || None);
                }
            }

            // make the individual attributes and track the stride/stepping of each slot
            for element in &info.input_layout {
                let attribute = metal::VertexAttributeDescriptor::new();
                attribute.set_format(to_mtl_vertex_format(element.format));
                attribute.set_buffer_index(element.input_slot as NSUInteger);
                attribute.set_offset(element.aligned_byte_offset as NSUInteger);
                vertex_desc.attributes().set_object_at(attrib_index, Some(&attribute));
                attrib_index += 1;

                let stride = element.aligned_byte_offset + block_size_for_format(element.format);
                let slot = element.input_slot as usize;
                if let Some(ref mut layout) = slot_layouts[slot] {
                    layout.stride = max(layout.stride, stride);
                } else {
                    slot_layouts[slot] = Some(SlotLayout {
                        stride,
                        input_slot_class: element.input_slot_class,
                        step_rate: element.step_rate,
                    });
                }
            }

            // create vertex buffer layouts for each slot
            for (slot, layout_opt) in slot_layouts.iter().enumerate() {
                if let Some(layout) = layout_opt {
                    let layout_desc = metal::VertexBufferLayoutDescriptor::new();
                    layout_desc.set_stride(layout.stride as NSUInteger);
                    match layout.input_slot_class {
                        super::InputSlotClass::PerVertex => {
                            layout_desc.set_step_function(metal::MTLVertexStepFunction::PerVertex);
                            layout_desc.set_step_rate(1);
                        }
                        super::InputSlotClass::PerInstance => {
                            layout_desc.set_step_function(metal::MTLVertexStepFunction::PerInstance);
                            layout_desc.set_step_rate(layout.step_rate as NSUInteger);
                        }
                    }
                    vertex_desc.layouts().set_object_at(slot as NSUInteger, Some(&layout_desc));
                }
            }

            pipeline_state_descriptor.set_vertex_descriptor(Some(&vertex_desc));

            // colour attachments - one per MRT target from the pass (SV_Target0..N). With no pass
            // (eg. depth-only / default) fall back to a single BGRA8 attachment.
            let pixel_formats: Vec<metal::MTLPixelFormat> = info.pass
                .map(|p| p.pixel_formats.clone())
                .filter(|f| !f.is_empty())
                .unwrap_or_else(|| vec![metal::MTLPixelFormat::BGRA8Unorm]);

            for (i, &pixel_format) in pixel_formats.iter().enumerate() {
                let attachment = pipeline_state_descriptor
                    .color_attachments()
                    .object_at(i as u64)
                    .unwrap();
                attachment.set_pixel_format(pixel_format);

                if pixel_format == metal::MTLPixelFormat::Invalid {
                    continue;
                }

                // per-target blend state (falls back to the first / disabled)
                let blend = info.blend_info.render_target.get(i)
                    .or_else(|| info.blend_info.render_target.first());
                if let Some(b) = blend {
                    attachment.set_blending_enabled(b.blend_enabled);
                    attachment.set_rgb_blend_operation(to_mtl_blend_op(&b.blend_op));
                    attachment.set_alpha_blend_operation(to_mtl_blend_op(&b.blend_op_alpha));
                    attachment.set_source_rgb_blend_factor(to_mtl_blend_factor(&b.src_blend));
                    attachment.set_source_alpha_blend_factor(to_mtl_blend_factor(&b.src_blend_alpha));
                    attachment.set_destination_rgb_blend_factor(to_mtl_blend_factor(&b.dst_blend));
                    attachment.set_destination_alpha_blend_factor(to_mtl_blend_factor(&b.dst_blend_alpha));
                    attachment.set_write_mask(to_mtl_write_mask(&b.write_mask));
                } else {
                    attachment.set_blending_enabled(false);
                    attachment.set_write_mask(metal::MTLColorWriteMask::all());
                }
            }

            // Set depth format + MSAA sample count on pipeline descriptor to match the pass
            if let Some(pass) = &info.pass {
                if let Some(depth_format) = pass.depth_format {
                    pipeline_state_descriptor.set_depth_attachment_pixel_format(depth_format);
                    if has_stencil_component(depth_format) {
                        pipeline_state_descriptor.set_stencil_attachment_pixel_format(depth_format);
                    }
                }
                pipeline_state_descriptor.set_sample_count(pass.sample_count as NSUInteger);
            }

            // Create depth stencil state
            let depth_stencil_state = {
                let ds_info = &info.depth_stencil_info;
                let ds_desc = metal::DepthStencilDescriptor::new();

                ds_desc.set_depth_compare_function(to_mtl_compare_func(ds_info.depth_func));
                ds_desc.set_depth_write_enabled(ds_info.depth_write_mask == super::DepthWriteMask::All);

                if ds_info.stencil_enabled {
                    // Front face
                    let front = metal::StencilDescriptor::new();
                    front.set_stencil_compare_function(to_mtl_compare_func(ds_info.front_face.func));
                    front.set_stencil_failure_operation(to_mtl_stencil_op(ds_info.front_face.fail));
                    front.set_depth_failure_operation(to_mtl_stencil_op(ds_info.front_face.depth_fail));
                    front.set_depth_stencil_pass_operation(to_mtl_stencil_op(ds_info.front_face.pass));
                    front.set_read_mask(ds_info.stencil_read_mask as u32);
                    front.set_write_mask(ds_info.stencil_write_mask as u32);
                    ds_desc.set_front_face_stencil(Some(&front));

                    // Back face
                    let back = metal::StencilDescriptor::new();
                    back.set_stencil_compare_function(to_mtl_compare_func(ds_info.back_face.func));
                    back.set_stencil_failure_operation(to_mtl_stencil_op(ds_info.back_face.fail));
                    back.set_depth_failure_operation(to_mtl_stencil_op(ds_info.back_face.depth_fail));
                    back.set_depth_stencil_pass_operation(to_mtl_stencil_op(ds_info.back_face.pass));
                    back.set_read_mask(ds_info.stencil_read_mask as u32);
                    back.set_write_mask(ds_info.stencil_write_mask as u32);
                    ds_desc.set_back_face_stencil(Some(&back));
                }

                self.metal_device.new_depth_stencil_state(&ds_desc)
            };

            // Create static samplers and argument buffer (at buffer(4) per htwv convention)
            let mut pipeline_static_samplers = Vec::new();
            let mut sampler_argument_buffer = None;

            if let Some(static_samplers) = &info.pipeline_layout.static_samplers {
                for sampler in static_samplers {
                    let si = &sampler.sampler_info;
                    let desc = metal::SamplerDescriptor::new();
                    desc.set_address_mode_r(to_mtl_sampler_address_mode(si.address_w));
                    desc.set_address_mode_s(to_mtl_sampler_address_mode(si.address_u));
                    desc.set_address_mode_t(to_mtl_sampler_address_mode(si.address_v));
                    desc.set_min_filter(to_mtl_sampler_min_mag_filter(si.filter));
                    desc.set_mag_filter(to_mtl_sampler_min_mag_filter(si.filter));
                    desc.set_mip_filter(to_mtl_sampler_mip_filter(si.filter));
                    if let Some(func) = si.comparison {
                        desc.set_compare_function(to_mtl_compare_func(func));
                    }
                    desc.set_support_argument_buffers(true);

                    pipeline_static_samplers.push(MetalSamplerBinding {
                        slot: sampler.shader_register,
                        sampler: self.metal_device.new_sampler(&desc)
                    })
                }

                // Create argument buffer for samplers at buffer(4)
                if !pipeline_static_samplers.is_empty() {
                    let arg_desc = metal::ArgumentDescriptor::new();
                    arg_desc.set_index(0);
                    arg_desc.set_data_type(metal::MTLDataType::Sampler);
                    arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);

                    let argument_encoder = self.metal_device.new_argument_encoder(
                        metal::Array::from_owned_slice(&[arg_desc.to_owned()])
                    );
                    let arg_buffer = self.metal_device.new_buffer(
                        argument_encoder.encoded_length(),
                        metal::MTLResourceOptions::StorageModeShared
                    );

                    // Encode sampler into argument buffer
                    argument_encoder.set_argument_buffer(&arg_buffer, 0);
                    argument_encoder.set_sampler_state(0, &pipeline_static_samplers[0].sampler);

                    sampler_argument_buffer = Some(arg_buffer);
                }
            }

            // Build unified slot lookup
            let slot_lookup = self.build_slot_lookup(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
            );

            // Build stage binders for push constants and resource bindings
            let (vertex_binder, fragment_binder) = self.build_stage_binders(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
            );

            let pipeline_state = self.metal_device.new_render_pipeline_state(&pipeline_state_descriptor)?;

            Ok(RenderPipeline {
                pipeline_state,
                slots: Vec::new(),
                static_samplers: pipeline_static_samplers,
                slot_lookup,
                vertex_binder,
                fragment_binder,
                sampler_argument_buffer,
                topology: info.topology,
                depth_stencil_state,
                raster_info: info.raster_info,
            })
        })
    }

    fn create_shader<T: Sized>(
        &self,
        info: &super::ShaderInfo,
        src: &[T],
    ) -> std::result::Result<Shader, super::Error> {
        objc::rc::autoreleasepool(|| {

            let (data, data_size) = unsafe {
                let src = slice_as_u8_slice(src);
                let data = std::alloc::alloc(Layout::from_size_align(src.len() + 1, 8)?);
                std::ptr::write_bytes(data, 0x0, src.len() + 1);
                std::ptr::copy_nonoverlapping(src.as_ptr(), data, src.len());
                (data, src.len())
            };

            let lib = if let Some(compile_info) = info.compile_info.as_ref() {

                let u8slice = slice_as_u8_slice(src);
                println!("{:?}", u8slice);

                let src = std::str::from_utf8(u8slice)?;
                println!("{:?}", src);

                self.metal_device.new_library_with_file(std::path::Path::new(src))?

                /*
                let src = std::str::from_utf8(slice_as_u8_slice(src))?;
                let opt = metal::CompileOptions::new();
                opt.set_fast_math_enabled(true);
                self.metal_device.new_library_with_source(src, &opt)?
                */
            }
            else {
                unsafe {
                    self.metal_device.new_library_with_data(std::slice::from_raw_parts(data, data_size))?
                }
            };

            let names = lib.function_names();
            if names.len() == 1 {
                Ok(Shader{
                    lib: lib.to_owned(),
                    data: data as *const u8,
                    data_size: data_size
                })
            }
            else {
                Err(super::Error {
                    msg: format!(
                        "hotline_rs::gfx::mtl expected a shader with single entry point but shader has {} functions", names.len()
                    ),
                })
            }
        })
    }

    fn create_buffer_with_heap<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
        heap: &mut Heap
    ) -> result::Result<Buffer, super::Error> {
        objc::rc::autoreleasepool(|| {
            // StorageModeShared: CPU and GPU share the same physical memory — no didModifyRange
            // needed and no stale-copy hazard. StorageModeManaged has a separate GPU copy that
            // requires an explicit sync notification after every CPU write; without it the GPU
            // reads stale data, causing tearing
            let opt = metal::MTLResourceOptions::CPUCacheModeDefaultCache |
                metal::MTLResourceOptions::StorageModeShared;

            let byte_len = (info.stride * info.num_elements) as NSUInteger;

            let buf = if let Some(data) = data {
                let bytes = data.as_ptr() as *const std::ffi::c_void;
                self.metal_device.new_buffer_with_data(bytes, byte_len, opt)
            }
            else {
                self.metal_device.new_buffer(byte_len, opt)
            };

            // allocate on the heap
            let alloc_index = heap.allocate();
            heap.buffer_slots[alloc_index] = Some(buf.to_owned());
            heap.encode_buffer(alloc_index, &buf);

            // assign srv or uav
            let srv_index = if info.usage.contains(BufferUsage::SHADER_RESOURCE) {
                Some(alloc_index)
            }
            else {
                None
            };

            let uav_index = if info.usage.contains(BufferUsage::UNORDERED_ACCESS) {
                Some(alloc_index)
            }
            else {
                None
            };

            let cbv_index = if info.usage.contains(BufferUsage::CONSTANT_BUFFER) {
                Some(alloc_index)
            }
            else {
                None
            };

            Ok(Buffer{
                metal_buffer: buf,
                element_stride: info.stride,
                srv_index,
                uav_index,
                cbv_index,
                counter_sample_buffer: None,
                counter_sample_index: 0,
                counter_cmd: None,
            })
        })
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        self.create_buffer_with_heap(
            info,
            data,
            &mut self.shader_heap.clone()
        )
    }

    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> result::Result<Self::Buffer, super::Error> {
        objc::rc::autoreleasepool(|| {
            let opt = metal::MTLResourceOptions::CPUCacheModeDefaultCache |
                metal::MTLResourceOptions::StorageModeManaged;

            // Metal doesn't allow zero-size buffers
            let byte_len = size.max(1) as NSUInteger;
            let buf = self.metal_device.new_buffer(byte_len, opt);

            Ok(Buffer{
                metal_buffer: buf,
                element_stride: size,
                srv_index: None,
                uav_index: None,
                cbv_index: None,
                counter_sample_buffer: None,
                counter_sample_index: 0,
                counter_cmd: None,
            })
        })
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {
        self.create_texture_with_heaps(
            info,
            TextureHeapInfo::default(),
            data,
        )
    }

    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> result::Result<Self::Texture, super::Error> {
        objc::rc::autoreleasepool(|| {
            let desc = TextureDescriptor::new();

            // clamp requested MSAA to what the device supports (eg. 8x -> 4x on most Apple GPUs)
            let sample_count = self.supported_sample_count(info.samples);
            let msaa = sample_count > 1;

            // desc
            desc.set_pixel_format(to_mtl_pixel_format(info.format));
            desc.set_width(info.width as NSUInteger);
            desc.set_height(info.height as NSUInteger);
            desc.set_depth(info.depth as NSUInteger);
            // MSAA textures cannot have a mip chain
            desc.set_mipmap_level_count(if msaa { 1 } else { info.mip_levels as NSUInteger });
            desc.set_usage(to_mtl_texture_usage(info.usage));
            // Must match the (Private) heap the texture is allocated from
            desc.set_storage_mode(metal::MTLStorageMode::Private);
            // MSAA Texture2D uses the D2Multisample type
            desc.set_texture_type(if msaa && matches!(info.tex_type, super::TextureType::Texture2D) {
                metal::MTLTextureType::D2Multisample
            } else {
                to_mtl_texture_type(info.tex_type)
            });

            // For cubemaps, arrayLength must be 1 (6 faces are implicit)
            // For cube arrays, arrayLength is the number of cubemaps (not faces)
            let array_length = match info.tex_type {
                super::TextureType::TextureCube => 1,
                super::TextureType::TextureCubeArray => info.array_layers / 6,
                _ => info.array_layers,
            };
            desc.set_array_length(array_length as NSUInteger);

            desc.set_sample_count(sample_count as NSUInteger);

            // use supplied heap or fallback to the device default
            let shader_heap = if let Some(shader_heap) = heaps.shader {
                shader_heap
            }
            else {
                &mut self.shader_heap
            };

            // heap bindless
            let tex = shader_heap.mtl_heap.new_texture(&desc)
                .expect("hotline_rs::gfx::mtl failed to allocate texture in heap!");

            // upload texture data with support for mips, cubemaps, and array slices.
            // The heap is Private (not CPU-writable), so stage the bytes in a Shared buffer and
            // blit each subresource into the texture on a one-shot command buffer.
            if let Some(data) = data {
                let block_size = super::block_size_for_format(info.format) as u64;
                let tpb = super::texels_per_block_for_format(info.format);

                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        data.as_ptr() as *const u8,
                        std::mem::size_of_val(data)
                    )
                };
                let staging = self.metal_device.new_buffer_with_data(
                    bytes.as_ptr() as *const std::ffi::c_void,
                    bytes.len() as NSUInteger,
                    metal::MTLResourceOptions::StorageModeShared
                );

                let cmd = self.command_queue.new_command_buffer();
                let blit = cmd.new_blit_command_encoder();

                let mut data_offset: u64 = 0;
                for a in 0..info.array_layers {
                    let mut mip_w = info.width;
                    let mut mip_h = info.height;
                    let mut mip_d = info.depth as u64;

                    for mip in 0..info.mip_levels {
                        let pitch = block_size * (mip_w / tpb).max(1);
                        let depth_pitch = pitch * (mip_h / tpb).max(1);

                        blit.copy_from_buffer_to_texture(
                            &staging,
                            data_offset as NSUInteger,
                            pitch as NSUInteger,
                            depth_pitch as NSUInteger,
                            metal::MTLSize { width: mip_w, height: mip_h, depth: mip_d },
                            &tex,
                            a as NSUInteger,
                            mip as NSUInteger,
                            metal::MTLOrigin { x: 0, y: 0, z: 0 },
                            metal::MTLBlitOption::empty(),
                        );

                        data_offset += depth_pitch * mip_d.max(1);

                        // halve dimensions for next mip (non-pot safe)
                        mip_w = (mip_w / 2).max(1);
                        mip_h = (mip_h / 2).max(1);
                        mip_d = (mip_d / 2).max(1);
                    }
                }

                blit.end_encoding();
                cmd.commit();
                cmd.wait_until_completed();
            }

            // allocate on the heap
            let alloc_index = shader_heap.allocate();
            shader_heap.texture_slots[alloc_index] = Some(tex.to_owned());

            // Encode texture into heap's argument buffer for bindless access
            shader_heap.encode_texture(alloc_index, &tex);

            let shader_resource = info.usage.contains(TextureUsage::SHADER_RESOURCE);

            // UAV only applies to the (non-MSAA) texture
            let uav_index = if info.usage.contains(TextureUsage::UNORDERED_ACCESS) {
                Some(alloc_index)
            }
            else {
                None
            };

            if msaa {
                // The primary texture is the MSAA view (read as Texture2DMS via get_msaa_srv_index).
                let msaa_srv_index = if shader_resource { Some(alloc_index) } else { None };

                // Create a single-sample resolve backing so the texture can be read normally and
                // resolved via resolve_texture_subresource (matches the D3D12 resolve concept).
                let mut resolved_texture = None;
                let mut srv_index = None;
                if shader_resource {
                    let rdesc = TextureDescriptor::new();
                    rdesc.set_pixel_format(to_mtl_pixel_format(info.format));
                    rdesc.set_width(info.width as NSUInteger);
                    rdesc.set_height(info.height as NSUInteger);
                    rdesc.set_depth(info.depth as NSUInteger);
                    rdesc.set_mipmap_level_count(info.mip_levels as NSUInteger);
                    rdesc.set_usage(to_mtl_texture_usage(info.usage));
                    rdesc.set_storage_mode(metal::MTLStorageMode::Private);
                    rdesc.set_texture_type(to_mtl_texture_type(info.tex_type));
                    rdesc.set_array_length(array_length as NSUInteger);
                    rdesc.set_sample_count(1);

                    let resolve_tex = shader_heap.mtl_heap.new_texture(&rdesc)
                        .expect("hotline_rs::gfx::mtl failed to allocate resolve texture in heap!");
                    let resolve_index = shader_heap.allocate();
                    shader_heap.texture_slots[resolve_index] = Some(resolve_tex.to_owned());
                    shader_heap.encode_texture(resolve_index, &resolve_tex);
                    srv_index = Some(resolve_index);
                    resolved_texture = Some(resolve_tex);
                }

                Ok(Texture{
                    metal_texture: tex,
                    resolved_texture,
                    srv_index,
                    msaa_srv_index,
                    uav_index,
                    resolvable: shader_resource,
                    heap_id: Some(shader_heap.id)
                })
            }
            else {
                let srv_index = if shader_resource { Some(alloc_index) } else { None };
                Ok(Texture{
                    metal_texture: tex,
                    resolved_texture: None,
                    srv_index,
                    msaa_srv_index: None,
                    uav_index,
                    resolvable: false,
                    heap_id: Some(shader_heap.id)
                })
            }
        })
    }

    fn create_render_pass(
        &self,
        info: &super::RenderPassInfo<Device>,
    ) -> result::Result<RenderPass, super::Error> {
        objc::rc::autoreleasepool(|| {
            // new desc
            let descriptor = metal::RenderPassDescriptor::new();

            // colour attachments - one per MRT target (SV_Target0..N)
            let mut pixel_formats = Vec::new();
            for (i, rt) in info.render_targets.iter().enumerate() {
                let color_attachment = descriptor.color_attachments().object_at(i as u64).unwrap();
                color_attachment.set_texture(Some(&rt.metal_texture));
                color_attachment.set_slice(info.array_slice as u64);

                if let Some(cc) = info.rt_clear {
                    color_attachment.set_load_action(metal::MTLLoadAction::Clear);
                    color_attachment.set_clear_color(metal::MTLClearColor::new(cc.r as f64, cc.g as f64, cc.b as f64, 1.0));
                }
                else {
                    color_attachment.set_load_action(metal::MTLLoadAction::Load);
                }

                // Keep the rendered (MSAA) samples. The MSAA resolve and any mip downsample are
                // driven by the render graph barriers (see resolve_texture_subresource /
                // generate_mip_maps), not baked into every pass, so the barrier can decide when they
                // happen (eg. only after the last of several passes that target the same resource).
                color_attachment.set_store_action(metal::MTLStoreAction::Store);

                pixel_formats.push(rt.metal_texture.pixel_format());
            }

            // sample count shared by all attachments (read from the first colour/depth target)
            let sample_count = info.render_targets.first()
                .map(|rt| rt.metal_texture.sample_count() as u32)
                .or_else(|| info.depth_stencil.map(|ds| ds.metal_texture.sample_count() as u32))
                .unwrap_or(1);

            // Handle depth stencil attachment
            let depth_format = if let Some(ds_texture) = &info.depth_stencil {
                let depth_attachment = descriptor.depth_attachment().unwrap();
                depth_attachment.set_texture(Some(&ds_texture.metal_texture));
                depth_attachment.set_slice(info.array_slice as u64);

                if let Some(ds_clear) = &info.ds_clear {
                    if let Some(depth_val) = ds_clear.depth {
                        depth_attachment.set_load_action(metal::MTLLoadAction::Clear);
                        depth_attachment.set_clear_depth(depth_val as f64);
                    } else {
                        depth_attachment.set_load_action(metal::MTLLoadAction::Load);
                    }
                } else {
                    depth_attachment.set_load_action(metal::MTLLoadAction::Load);
                }
                depth_attachment.set_store_action(metal::MTLStoreAction::Store);

                let format = ds_texture.metal_texture.pixel_format();

                // Handle stencil if format has stencil component
                if has_stencil_component(format) {
                    let stencil_attachment = descriptor.stencil_attachment().unwrap();
                    stencil_attachment.set_texture(Some(&ds_texture.metal_texture));
                    stencil_attachment.set_slice(info.array_slice as u64);

                    if let Some(ds_clear) = &info.ds_clear {
                        if let Some(stencil_val) = ds_clear.stencil {
                            stencil_attachment.set_load_action(metal::MTLLoadAction::Clear);
                            stencil_attachment.set_clear_stencil(stencil_val as u32);
                        } else {
                            stencil_attachment.set_load_action(metal::MTLLoadAction::Load);
                        }
                    } else {
                        stencil_attachment.set_load_action(metal::MTLLoadAction::Load);
                    }
                    stencil_attachment.set_store_action(metal::MTLStoreAction::Store);
                }

                Some(format)
            } else {
                None
            };

            Ok(RenderPass{
                desc: descriptor.to_owned(),
                pixel_formats,
                depth_format,
                sample_count,
            })
        })
    }

    fn create_raytracing_pipeline(
        &self,
        info: &super::RaytracingPipelineInfo<Self>,
    ) -> result::Result<RaytracingPipeline, super::Error> {
        unimplemented!()
    }

    fn create_raytracing_blas(
        &mut self,
        info: &RaytracingBLASInfo<Self>
    ) -> result::Result<RaytracingBLAS, super::Error> {
        unimplemented!()
    }

    fn create_raytracing_shader_binding_table(
        &self,
        info: &super::RaytracingShaderBindingTableInfo<Self>
    ) -> result::Result<RaytracingShaderBindingTable, super::Error> {
        unimplemented!()
    }

    fn create_compute_pipeline(
        &self,
        info: &super::ComputePipelineInfo<Self>,
    ) -> result::Result<ComputePipeline, super::Error> {
        objc::rc::autoreleasepool(|| {
            // load the compute kernel function from the shader library
            let function = unsafe {
                let cs = info.cs;
                let lib = self.metal_device.new_library_with_data(
                    std::slice::from_raw_parts(cs.data, cs.data_size)
                )?;
                let name = &lib.function_names()[0];
                lib.get_function(name, None).unwrap()
            };

            let pipeline_state = self.metal_device.new_compute_pipeline_state_with_function(&function)?;

            // unified slot lookup + single-stage binder, both keyed by (register, space, type)
            let slot_lookup = self.build_slot_lookup(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
            );

            let compute_binder = self.build_compute_binder(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
            );

            Ok(ComputePipeline {
                pipeline_state,
                slots: Vec::new(),
                slot_lookup,
                compute_binder,
            })
        })
    }

    fn create_indirect_render_command<T: Sized>(&mut self,
        arguments: Vec<super::IndirectArgument>,
        pipeline: Option<&RenderPipeline>) -> result::Result<CommandSignature, super::Error> {
        Ok(CommandSignature{

        })
    }

    fn execute(&mut self, cmd: &CmdBuf) {
        // Pass command buffers commit themselves in CmdBuf::close, so there is nothing to submit
        // here for them. Barrier command buffers instead carry deferred ops (transition / resolve /
        // generate mips) which we replay into a fresh command buffer every frame, mirroring how
        // D3D12 re-executes a pre-recorded barrier command list.
        if cmd.deferred_ops.is_empty() {
            return;
        }

        objc::rc::autoreleasepool(|| {
            let metal_cmd = self.command_queue.new_command_buffer();
            for op in &cmd.deferred_ops {
                match op {
                    DeferredBarrierOp::Resolve { msaa, resolve } => {
                        // a load/no-clear pass with a MultisampleResolve store action resolves the
                        // MSAA samples into the single-sample backing without drawing anything.
                        let descriptor = metal::RenderPassDescriptor::new();
                        // depth/stencil targets must resolve through the depth (and stencil)
                        // attachments, not a color attachment - a depth format on color
                        // attachment 0 is "not color renderable" and trips Metal validation,
                        // blocking GPU captures.
                        if is_depth_format(msaa.pixel_format()) {
                            let depth = descriptor.depth_attachment().unwrap();
                            depth.set_texture(Some(msaa));
                            depth.set_resolve_texture(Some(resolve));
                            depth.set_load_action(metal::MTLLoadAction::Load);
                            depth.set_store_action(metal::MTLStoreAction::MultisampleResolve);
                            if has_stencil_component(msaa.pixel_format()) {
                                let stencil = descriptor.stencil_attachment().unwrap();
                                stencil.set_texture(Some(msaa));
                                stencil.set_resolve_texture(Some(resolve));
                                stencil.set_load_action(metal::MTLLoadAction::Load);
                                stencil.set_store_action(metal::MTLStoreAction::MultisampleResolve);
                            }
                        } else {
                            let attachment = descriptor.color_attachments().object_at(0).unwrap();
                            attachment.set_texture(Some(msaa));
                            attachment.set_resolve_texture(Some(resolve));
                            attachment.set_load_action(metal::MTLLoadAction::Load);
                            attachment.set_store_action(metal::MTLStoreAction::StoreAndMultisampleResolve);
                        }
                        let encoder = metal_cmd.new_render_command_encoder(&descriptor);
                        encoder.end_encoding();
                    }
                    DeferredBarrierOp::GenerateMips { texture } => {
                        let blit = metal_cmd.new_blit_command_encoder();
                        blit.generate_mipmaps(texture);
                        blit.end_encoding();
                    }
                }
            }
            metal_cmd.commit();
        });
    }

    fn report_live_objects(&self) -> result::Result<(), super::Error> {
        Ok(())
    }

    fn get_info_queue_messages(&self) -> result::Result<Vec<String>, super::Error> {
        Ok(vec![])
    }

    fn get_shader_heap(&self) -> &Self::Heap {
        &self.shader_heap
    }

    fn get_shader_heap_mut(&mut self) -> &mut Self::Heap {
        &mut self.shader_heap
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &Self::SwapChain) {

    }

    fn get_adapter_info(&self) -> &AdapterInfo {
        &self.adapter_info
    }

    fn read_buffer(&self, swap_chain: &SwapChain, buffer: &Buffer, size: usize, frame_written_fence: u64) -> Option<super::ReadBackData> {
        None
    }

    fn read_timestamps(&self, _swap_chain: &SwapChain, buffer: &Self::Buffer, _size_bytes: usize, _frame_written_fence: u64) -> Vec<f64> {
        // Metal has no GPU-signalled fence; wait for the pass command buffer to finish before reading
        // its timestamps (equivalent to D3D12's GPU fence check).
        if let Some(cmd) = &buffer.counter_cmd {
            cmd.wait_until_completed();
        }

        if let Some(sample_buffer) = &buffer.counter_sample_buffer {
            // counter-sampling path: resolve the one timestamp this buffer points at. The GPU
            // timestamp is in nanoseconds on Apple Silicon; gather_stats wants seconds.
            unsafe {
                let range = metal::NSRange {
                    location: buffer.counter_sample_index as _,
                    length: 1,
                };
                let ns_data: *mut objc::runtime::Object =
                    msg_send![sample_buffer.as_ref(), resolveCounterRange: range];
                if !ns_data.is_null() {
                    let bytes: *const u8 = msg_send![ns_data, bytes];
                    let len: usize = msg_send![ns_data, length];
                    if len >= std::mem::size_of::<u64>() {
                        let nanos = (bytes as *const u64).read_unaligned();
                        // MTLCounterErrorValue marks a sample the GPU could not record - treat as none
                        if nanos != u64::MAX {
                            return vec![nanos as f64 / 1_000_000_000.0];
                        }
                    }
                }
            }
            return vec![];
        }

        // fallback path: whole-CB timing. index 0 = start of pass, index 1 = end of pass.
        if let Some(cmd) = &buffer.counter_cmd {
            let seconds: f64 = unsafe {
                if buffer.counter_sample_index == 0 {
                    msg_send![cmd.as_ref(), GPUStartTime]
                } else {
                    msg_send![cmd.as_ref(), GPUEndTime]
                }
            };
            return vec![seconds];
        }
        vec![]
    }

    fn read_pipeline_statistics(&self, swap_chain: &SwapChain, buffer: &Self::Buffer, frame_written_fence: u64) -> Option<super::PipelineStatistics> {
        None
    }

    fn get_timestamp_size_bytes() -> usize {
        8 // u64; matches D3D12 — Metal uses CounterSampleBuffer, not this backing store
    }

    fn get_pipeline_statistics_size_bytes() -> usize {
        0
    }

    fn get_indirect_command_size(argument_type: IndirectArgumentType) -> usize {
        0
    }

    fn get_counter_alignment() -> usize {
        0
    }

    fn create_upload_buffer<T: Sized>(
        &mut self,
        data: &[T]
    ) -> Result<Buffer, Error> {
        unimplemented!()
    }

    fn create_raytracing_instance_buffer(
        &mut self,
        instances: &Vec<RaytracingInstanceInfo<Self>>
    ) -> Result<Buffer, Error> {
        unimplemented!()
    }

    fn create_raytracing_tlas(
        &mut self,
        info: &RaytracingTLASInfo<Self>
    ) -> Result<Self::RaytracingTLAS, Error> {
        unimplemented!()
    }

    fn create_resource_view(
        &mut self,
        info: &ResourceViewInfo,
        resource: Resource<Device>,
        heap: &mut Heap
    ) -> Result<usize, super::Error> {
        unimplemented!()
    }

    fn create_raytracing_tlas_with_heap(
        &mut self,
        info: &RaytracingTLASInfo<Self>,
        heap: &mut Heap
    ) -> Result<RaytracingTLAS, Error> {
        unimplemented!()
    }

}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}
unsafe impl Send for SwapChain {}
unsafe impl Sync for SwapChain {}
unsafe impl Send for RenderPass {}
unsafe impl Sync for RenderPass {}
unsafe impl Send for RenderPipeline {}
unsafe impl Sync for RenderPipeline {}
unsafe impl Send for ComputePipeline {}
unsafe impl Sync for ComputePipeline {}
unsafe impl Send for Shader {}
unsafe impl Sync for Shader {}
unsafe impl Send for CmdBuf {}
unsafe impl Sync for CmdBuf {}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}
unsafe impl Send for Texture {}
unsafe impl Sync for Texture {}
unsafe impl Send for Heap {}
unsafe impl Sync for Heap {}
unsafe impl Send for QueryHeap {}
unsafe impl Sync for QueryHeap {}
unsafe impl Send for CommandSignature {}
unsafe impl Sync for CommandSignature {}

impl super::ComputePipeline<Device> for ComputePipeline {}
impl super::CommandSignature<Device> for CommandSignature {}

impl super::RaytracingPipeline<Device> for RaytracingPipeline {}
impl super::RaytracingShaderBindingTable<Device> for RaytracingShaderBindingTable {}
impl super::RaytracingBLAS<Device> for RaytracingBLAS {}

impl super::RaytracingTLAS<Device> for RaytracingTLAS {
    fn get_srv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_shader_heap_id(&self) -> u16 {
        unimplemented!()
    }
}
