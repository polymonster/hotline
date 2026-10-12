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

/// Setup colour attachments - one per MRT target from the pass (SV_Target0..N), with blend state from `blend_info`.
/// With no pass (eg. depth-only / default) fall back to a single BGRA8 attachment.
fn setup_colour_attachments(
    attachments: &metal::RenderPipelineColorAttachmentDescriptorArrayRef,
    blend_info: &super::BlendInfo,
    pass: Option<&RenderPass>
) {
    let pixel_formats: Vec<metal::MTLPixelFormat> = pass
        .map(|p| p.pixel_formats.clone())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| vec![metal::MTLPixelFormat::BGRA8Unorm]);

    for (i, &pixel_format) in pixel_formats.iter().enumerate() {
        let attachment = attachments.object_at(i as u64).unwrap();
        attachment.set_pixel_format(pixel_format);

        if pixel_format == metal::MTLPixelFormat::Invalid {
            continue;
        }

        // per-target blend state (falls back to the first / disabled)
        let blend = blend_info.render_target.get(i)
            .or_else(|| blend_info.render_target.first());
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
}

/// Copy the attachments of `desc` into a new render pass descriptor which loads them, to resume a render pass in a
/// new encoder. A new descriptor is made rather than a copy so the load actions of `desc` are untouched. If a
/// timestamp is being sampled, only the end of the pass is sampled so the timestamp covers the whole pass
fn resume_render_pass_descriptor(desc: &metal::RenderPassDescriptorRef) -> metal::RenderPassDescriptor {
    fn copy_attachment(src: &metal::RenderPassAttachmentDescriptorRef, dst: &metal::RenderPassAttachmentDescriptorRef) {
        dst.set_texture(src.texture());
        dst.set_level(src.level());
        dst.set_slice(src.slice());
        dst.set_depth_plane(src.depth_plane());
        dst.set_resolve_texture(src.resolve_texture());
        dst.set_resolve_level(src.resolve_level());
        dst.set_resolve_slice(src.resolve_slice());
        dst.set_resolve_depth_plane(src.resolve_depth_plane());
        dst.set_store_action(src.store_action());
        dst.set_load_action(metal::MTLLoadAction::Load);
    }

    let resume = metal::RenderPassDescriptor::new().to_owned();
    for i in 0..8 {
        let src = desc.color_attachments().object_at(i).unwrap();
        if src.texture().is_some() {
            copy_attachment(src, resume.color_attachments().object_at(i).unwrap());
        }
    }
    if let (Some(src), Some(dst)) = (desc.depth_attachment(), resume.depth_attachment()) {
        if src.texture().is_some() {
            copy_attachment(src, dst);
        }
    }
    if let (Some(src), Some(dst)) = (desc.stencil_attachment(), resume.stencil_attachment()) {
        if src.texture().is_some() {
            copy_attachment(src, dst);
        }
    }
    resume.set_render_target_array_length(desc.render_target_array_length());
    resume.set_default_raster_sample_count(desc.default_raster_sample_count());

    if let Some(src) = desc.sample_buffer_attachments().object_at(0) {
        let sample_buffer: *mut objc::runtime::Object = unsafe { msg_send![src, sampleBuffer] };
        if !sample_buffer.is_null() {
            let dst = resume.sample_buffer_attachments().object_at(0).unwrap();
            dst.set_sample_buffer(src.sample_buffer());
            dst.set_start_of_vertex_sample_index(MTL_COUNTER_DONT_SAMPLE);
            dst.set_end_of_vertex_sample_index(MTL_COUNTER_DONT_SAMPLE);
            dst.set_start_of_fragment_sample_index(MTL_COUNTER_DONT_SAMPLE);
            dst.set_end_of_fragment_sample_index(src.end_of_fragment_sample_index());
        }
    }

    resume
}

/// metal-rs does not define the object and mesh stages, they are MTLRenderStageObject (1 << 3) and MTLRenderStageMesh (1 << 4)
fn to_mtl_render_stage(stage: super::ShaderType) -> metal::MTLRenderStages {
    match stage {
        super::ShaderType::Vertex => metal::MTLRenderStages::Vertex,
        super::ShaderType::Fragment => metal::MTLRenderStages::Fragment,
        super::ShaderType::Amplification => metal::MTLRenderStages::from_bits_retain(1 << 3),
        super::ShaderType::Mesh => metal::MTLRenderStages::from_bits_retain(1 << 4),
        _ => unimplemented!(),
    }
}

fn to_mtl_acceleration_structure_vertex_format(format: super::Format) -> result::Result<metal::MTLAttributeFormat, super::Error> {
    match format {
        super::Format::RG32f => Ok(metal::MTLAttributeFormat::Float2),
        super::Format::RGB32f => Ok(metal::MTLAttributeFormat::Float3),
        super::Format::RGBA32f => Ok(metal::MTLAttributeFormat::Float4),
        _ => Err(super::Error {
            msg: "hotline_rs::gfx::mtl: unsupported acceleration structure vertex format".to_string()
        })
    }
}

fn to_mtl_acceleration_structure_index_type(format: super::Format) -> result::Result<metal::MTLIndexType, super::Error> {
    match format {
        super::Format::R16u => Ok(metal::MTLIndexType::UInt16),
        super::Format::R32u => Ok(metal::MTLIndexType::UInt32),
        _ => Err(super::Error {
            msg: "hotline_rs::gfx::mtl: unsupported acceleration structure index format".to_string()
        })
    }
}

/// metal-rs `descriptor()` constructors wrap an autoreleased object as owned, so the owned release on drop and the
/// autorelease pool drain both release it. Retain it once so the owned wrapper is balanced
fn retain_descriptor<T: metal::foreign_types::ForeignType>(descriptor: T) -> T {
    unsafe {
        let _: *mut objc::runtime::Object = msg_send![descriptor.as_ptr() as *mut objc::runtime::Object, retain];
    }
    descriptor
}

/// MTLAccelerationStructureUsage, which metal-rs does not define
const MTL_ACCELERATION_STRUCTURE_USAGE_REFIT: NSUInteger = 1 << 0;
const MTL_ACCELERATION_STRUCTURE_USAGE_PREFER_FAST_BUILD: NSUInteger = 1 << 1;

fn to_mtl_acceleration_structure_usage(flags: super::AccelerationStructureBuildFlags) -> NSUInteger {
    let mut usage = 0;
    if flags.contains(super::AccelerationStructureBuildFlags::ALLOW_UPDATE) {
        usage |= MTL_ACCELERATION_STRUCTURE_USAGE_REFIT;
    }
    if flags.contains(super::AccelerationStructureBuildFlags::PREFER_FAST_BUILD) {
        usage |= MTL_ACCELERATION_STRUCTURE_USAGE_PREFER_FAST_BUILD;
    }
    usage
}

/// Convert hotline instances to metal instance descriptors, metal instances reference their BLAS by index into an
/// array of acceleration structures (returned alongside), where d3d12 uses the BLAS address
fn to_mtl_instance_descriptors(
    instances: &[super::RaytracingInstanceInfo<Device>]
) -> (Vec<metal::MTLAccelerationStructureUserIDInstanceDescriptor>, Vec<metal::AccelerationStructure>) {
    let mut blases: Vec<metal::AccelerationStructure> = Vec::new();
    let descriptors = instances.iter().map(|instance| {
        let blas = &instance.blas.acceleration_structure;
        let index = blases.iter().position(|b| std::ptr::eq::<metal::AccelerationStructureRef>(&**b, &**blas)).unwrap_or_else(|| {
            blases.push(blas.clone());
            blases.len() - 1
        });

        // hotline transforms are row-major 3x4, metal is column-major 4x3
        let m = &instance.transform;
        let mut transformation_matrix = [[0.0; 3]; 4];
        for (c, column) in transformation_matrix.iter_mut().enumerate() {
            for (r, value) in column.iter_mut().enumerate() {
                *value = m[r * 4 + c];
            }
        }

        metal::MTLAccelerationStructureUserIDInstanceDescriptor {
            transformation_matrix,
            // d3d12 instance flags (cull disable, front ccw, force opaque, force non opaque) share the metal bits
            options: metal::MTLAccelerationStructureInstanceOptions::from_bits_truncate(instance.instance_flags),
            mask: instance.instance_mask,
            intersection_function_table_offset: instance.hit_group_index,
            acceleration_structure_index: index as u32,
            user_id: instance.instance_id,
        }
    }).collect();
    (descriptors, blases)
}

/// Create an instance acceleration structure descriptor for `instance_count` instances in `instance_buffer`
fn instance_acceleration_structure_descriptor(
    instance_buffer: &metal::BufferRef,
    instance_count: usize,
    blases: &[metal::AccelerationStructure],
    usage: NSUInteger
) -> metal::InstanceAccelerationStructureDescriptor {
    let desc = retain_descriptor(metal::InstanceAccelerationStructureDescriptor::descriptor());
    let blas_refs: Vec<&metal::AccelerationStructureRef> = blases.iter().map(|b| b.as_ref()).collect();
    desc.set_instanced_acceleration_structures(metal::Array::from_slice(&blas_refs));
    desc.set_instance_descriptor_type(metal::MTLAccelerationStructureInstanceDescriptorType::UserID);
    desc.set_instance_descriptor_buffer(instance_buffer);
    desc.set_instance_descriptor_stride(std::mem::size_of::<metal::MTLAccelerationStructureUserIDInstanceDescriptor>() as NSUInteger);
    desc.set_instance_count(instance_count as NSUInteger);
    unsafe { let _: () = msg_send![&*desc, setUsage: usage]; }
    desc
}

fn to_mtl_size(size: super::Size3) -> metal::MTLSize {
    metal::MTLSize::new(size.x as u64, size.y as u64, size.z as u64)
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
        super::ResourceType::AppendStructuredBuffer |
        super::ResourceType::ConsumeStructuredBuffer |
        super::ResourceType::ConstantBuffer |
        super::ResourceType::ByteAddressBuffer |
        super::ResourceType::RWByteAddressBuffer |
        super::ResourceType::Buffer => metal::MTLDataType::Pointer,
        // acceleration structures are bound from the heap's table of resource ids, see `ResourceBinder`
        super::ResourceType::RaytracingAccelerationStructure => metal::MTLDataType::Pointer,
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
    /// Built in kernel which translates indirect arguments, see `INDIRECT_CLAMP_MSL`
    indirect_clamp_pipeline: metal::ComputePipelineState,
    feature_flags: DeviceFeatureFlags,
}

/// MTLCounterSamplingPoint::atStageBoundary — sampling at the boundary between encoder stages.
const MTL_COUNTER_SAMPLING_POINT_AT_STAGE_BOUNDARY: NSUInteger = 0;
/// MTLCounterDontSample sentinel: a stage index that should not record a timestamp.
const MTL_COUNTER_DONT_SAMPLE: NSUInteger = NSUInteger::MAX;

/// Byte stride between per command push constants written by the indirect clamp kernel. macOS requires constant
/// address space buffer offsets to be 256 byte aligned, and push constants are at most 64 x 32bit values
const INDIRECT_PUSH_CONSTANTS_STRIDE: u64 = 256;
/// Max number of push constants arguments in a mesh command signature, matches `IndirectClampInfo`
const MAX_INDIRECT_PUSH_CONSTANTS: usize = 8;

/// Built in kernel run by `execute_indirect` to translate d3d12 style indirect arguments for metal. Each thread
/// handles a command: it writes the dispatch mesh args, zeroed past the count buffer value so those draws are empty,
/// and copies the command's push constants to a 256 byte aligned slot so they can be bound as a constant buffer
const INDIRECT_CLAMP_MSL: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct PushConstantsCopy {
    uint src_offset;
    uint num_values;
};

struct IndirectClampInfo {
    uint stride;
    uint dispatch_offset;
    uint max_count;
    uint has_count;
    uint num_push_constants;
    uint push_constants_stride;
    PushConstantsCopy push_constants[8];
};

kernel void indirect_clamp(
    device const uint* args [[buffer(0)]],
    device const uint* count [[buffer(1)]],
    device uint* dispatch_args [[buffer(2)]],
    device uint* push_constants [[buffer(3)]],
    constant IndirectClampInfo& info [[buffer(4)]],
    uint i [[thread_position_in_grid]])
{
    if (i >= info.max_count) {
        return;
    }

    uint n = info.has_count != 0 ? min(count[0], info.max_count) : info.max_count;
    bool live = i < n;
    uint base = (i * info.stride) / 4;

    for (uint c = 0; c < 3; ++c) {
        dispatch_args[i * 3 + c] = live ? args[base + info.dispatch_offset / 4 + c] : 0;
    }

    if (!live) {
        return;
    }

    for (uint p = 0; p < info.num_push_constants; ++p) {
        uint dst = ((i * info.num_push_constants + p) * info.push_constants_stride) / 4;
        for (uint v = 0; v < info.push_constants[p].num_values; ++v) {
            push_constants[dst + v] = args[base + info.push_constants[p].src_offset / 4 + v];
        }
    }
}
"#;

/// Matches `PushConstantsCopy` in `INDIRECT_CLAMP_MSL`
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PushConstantsCopy {
    src_offset: u32,
    num_values: u32,
}

/// Matches `IndirectClampInfo` in `INDIRECT_CLAMP_MSL`
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct IndirectClampInfo {
    stride: u32,
    dispatch_offset: u32,
    max_count: u32,
    has_count: u32,
    num_push_constants: u32,
    push_constants_stride: u32,
    push_constants: [PushConstantsCopy; MAX_INDIRECT_PUSH_CONSTANTS],
}

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
            let prev_size = self.layer.drawable_size();
            self.layer.set_contents_scale(window.get_dpi_scale() as f64);
            self.layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));
            let resized = prev_size.width != draw_size.x as f64 || prev_size.height != draw_size.y as f64;

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

            resized
        })
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
    bound_mesh_pipeline: Option<*const MeshPipeline>,
    bound_compute_pipeline: Option<*const ComputePipeline>,
    metal_device: metal::Device,
    transient_buffers: Vec<metal::Buffer>,
    vertex_binder: HashMap<SlotKey, PipelineStageBinder>,
    fragment_binder: HashMap<SlotKey, PipelineStageBinder>,
    object_binder: HashMap<SlotKey, PipelineStageBinder>,
    mesh_binder: HashMap<SlotKey, PipelineStageBinder>,
    compute_binder: HashMap<SlotKey, PipelineStageBinder>,
    deferred_ops: Vec<DeferredBarrierOp>,
    pending_timestamp: Option<(metal::CounterSampleBuffer, NSUInteger)>,
    /// Encoder state of the open render pass, so `execute_indirect` can split the pass and restore it
    pass_state: RenderPassState,
    /// Built in kernel which translates indirect arguments, see `INDIRECT_CLAMP_MSL`
    indirect_clamp_pipeline: metal::ComputePipelineState,
}

/// Render encoder state that is not held in the bound pipeline or binders, tracked between `begin_render_pass` and
/// `end_render_pass` so a pass can be split into a new encoder and resumed with the same state
#[derive(Clone, Default)]
struct RenderPassState {
    desc: Option<metal::RenderPassDescriptor>,
    viewport: Option<MTLViewport>,
    scissor: Option<MTLScissorRect>,
    vertex_buffers: Vec<(NSUInteger, metal::Buffer)>,
    heap: Option<*const Heap>,
    debug_groups: Vec<String>,
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
            bound_mesh_pipeline: self.bound_mesh_pipeline,
            bound_compute_pipeline: self.bound_compute_pipeline,
            metal_device: self.metal_device.clone(),
            transient_buffers: self.transient_buffers.clone(),
            vertex_binder: self.vertex_binder.clone(),
            fragment_binder: self.fragment_binder.clone(),
            object_binder: self.object_binder.clone(),
            mesh_binder: self.mesh_binder.clone(),
            compute_binder: self.compute_binder.clone(),
            deferred_ops: self.deferred_ops.clone(),
            pending_timestamp: self.pending_timestamp.clone(),
            pass_state: self.pass_state.clone(),
            indirect_clamp_pipeline: self.indirect_clamp_pipeline.clone(),
        }
    }
}

impl CmdBuf {
    /// Set the encoder state of a render pipeline: pipeline state, depth stencil, raster and static samplers
    fn apply_render_pipeline_state(&self, pipeline: &RenderPipeline) {
        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        encoder.set_render_pipeline_state(&pipeline.pipeline_state);
        encoder.set_depth_stencil_state(&pipeline.depth_stencil_state);

        let raster = &pipeline.raster_info;
        encoder.set_cull_mode(to_mtl_cull_mode(raster.cull_mode));
        encoder.set_front_facing_winding(to_mtl_winding(raster.front_ccw));
        encoder.set_triangle_fill_mode(to_mtl_triangle_fill_mode(raster.fill_mode));
        encoder.set_depth_bias(raster.depth_bias as f32, raster.slope_scaled_depth_bias, raster.depth_bias_clamp);

        // Bind sampler argument buffer at buffer(0) in fragment shader
        if let Some(ref sampler_arg_buffer) = pipeline.sampler_argument_buffer {
            encoder.set_fragment_buffer(0, Some(sampler_arg_buffer), 0);
        }
    }

    /// Set the encoder state of a mesh pipeline: pipeline state, depth stencil, raster and static samplers
    fn apply_mesh_pipeline_state(&self, pipeline: &MeshPipeline) {
        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        encoder.set_render_pipeline_state(&pipeline.pipeline_state);
        encoder.set_depth_stencil_state(&pipeline.depth_stencil_state);

        let raster = &pipeline.raster_info;
        encoder.set_cull_mode(to_mtl_cull_mode(raster.cull_mode));
        encoder.set_front_facing_winding(to_mtl_winding(raster.front_ccw));
        encoder.set_triangle_fill_mode(to_mtl_triangle_fill_mode(raster.fill_mode));
        encoder.set_depth_bias(raster.depth_bias as f32, raster.slope_scaled_depth_bias, raster.depth_bias_clamp);

        // bind sampler argument buffer at buffer(0) of each stage
        if let Some(ref sampler_arg_buffer) = pipeline.sampler_argument_buffer {
            encoder.set_fragment_buffer(0, Some(sampler_arg_buffer), 0);
            encoder.set_mesh_buffer(0, Some(sampler_arg_buffer), 0);
            encoder.set_object_buffer(0, Some(sampler_arg_buffer), 0);
        }
    }

    /// Bind the heap argument buffers to each stage of the currently bound render or mesh pipeline, using the
    /// binders cloned into the command buffer by set_render_pipeline / set_mesh_pipeline
    fn bind_heap_render(&self, heap: &Heap) {
        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        let stages = [
            (super::ShaderType::Vertex, &self.vertex_binder),
            (super::ShaderType::Fragment, &self.fragment_binder),
            (super::ShaderType::Mesh, &self.mesh_binder),
            (super::ShaderType::Amplification, &self.object_binder),
        ];
        for (stage, binder) in stages {
            if binder.is_empty() {
                continue;
            }
            let render_stage = to_mtl_render_stage(stage);

            // Structured buffers are device-allocated (not part of mtl_heap), so use_heap_at does not
            // make them resident - they are reached indirectly through the bindless buffer argument
            // buffer, so without this the GPU can read unmapped memory. Textures live in mtl_heap and
            // are covered by use_heap_at below.
            for buffer in heap.buffer_slots.iter().flatten() {
                encoder.use_resource_at(
                    buffer,
                    metal::MTLResourceUsage::Read | metal::MTLResourceUsage::Write,
                    render_stage,
                );
            }
            encoder.use_heap_at(&heap.mtl_heap, render_stage);
            for acceleration_structure in heap.acceleration_structure_slots.iter().flatten() {
                acceleration_structure.use_resources(|r| encoder.use_resource_at(r, metal::MTLResourceUsage::Read, render_stage));
            }

            for slot in binder.values() {
                if let PipelineStageBinder::Resource(res) = slot {
                    let arg_buffer = match res.data_type {
                        _ if res.acceleration_structure => &heap.acceleration_structure_argument_buffer,
                        metal::MTLDataType::Texture => heap.get_texture_argument_buffer(),
                        metal::MTLDataType::Pointer => heap.get_buffer_argument_buffer(),
                        _ => continue,
                    };
                    let index = res.buffer_index as u64;
                    match stage {
                        super::ShaderType::Vertex => encoder.set_vertex_buffer(index, Some(arg_buffer), 0),
                        super::ShaderType::Fragment => encoder.set_fragment_buffer(index, Some(arg_buffer), 0),
                        super::ShaderType::Mesh => encoder.set_mesh_buffer(index, Some(arg_buffer), 0),
                        _ => encoder.set_object_buffer(index, Some(arg_buffer), 0),
                    }
                }
            }
        }
    }

    /// End the open render encoder, encode compute work with `encode`, then resume the render pass in a new render
    /// encoder which loads the attachments and restores the tracked encoder state. Used for work that must run
    /// outside of a render pass but is issued inside one, such as translating indirect arguments
    fn split_render_pass<F: FnOnce(&metal::ComputeCommandEncoderRef)>(&mut self, encode: F) {
        let render_encoder = self.render_encoder.take()
            .expect("hotline_rs::gfx::mtl expected to be inside a render pass to split it");
        for _ in &self.pass_state.debug_groups {
            render_encoder.pop_debug_group();
        }
        render_encoder.end_encoding();

        let cmd = self.cmd.as_ref()
            .expect("hotline_rs::gfx::mtl expected call to CmdBuf::reset before encoding commands");
        let compute_encoder = cmd.new_compute_command_encoder();
        encode(compute_encoder);
        compute_encoder.end_encoding();

        let desc = resume_render_pass_descriptor(self.pass_state.desc.as_ref()
            .expect("hotline_rs::gfx::mtl expected render pass state to split a render pass"));
        self.render_encoder = Some(cmd.new_render_command_encoder(&desc).to_owned());
        self.restore_render_pass_state();
    }

    /// Re-apply the tracked state of the render pass to a new render encoder, binders are marked dirty so they
    /// are flushed again on the next draw
    fn restore_render_pass_state(&mut self) {
        let state = self.pass_state.clone();
        {
            let encoder = self.render_encoder.as_ref().unwrap();
            for name in &state.debug_groups {
                encoder.push_debug_group(name);
            }
            if let Some(viewport) = state.viewport {
                encoder.set_viewport(viewport);
            }
            if let Some(scissor) = state.scissor {
                encoder.set_scissor_rect(scissor);
            }
            for (slot, buffer) in &state.vertex_buffers {
                encoder.set_vertex_buffer(*slot, Some(buffer), 0);
            }
        }

        if let Some(pipeline) = self.bound_render_pipeline {
            self.apply_render_pipeline_state(unsafe { &*pipeline });
        }
        if let Some(pipeline) = self.bound_mesh_pipeline {
            self.apply_mesh_pipeline_state(unsafe { &*pipeline });
        }
        if let Some(heap) = state.heap {
            self.bind_heap_render(unsafe { &*heap });
        }

        for (_, binder) in self.render_binders_mut() {
            for b in binder.values_mut() {
                match b {
                    PipelineStageBinder::PushConstants(pc) => pc.dirty = true,
                    PipelineStageBinder::Resource(rb) => rb.dirty = true,
                }
            }
        }
    }

    /// The render stage binders, paired with the shader stage they bind to
    fn render_binders_mut(&mut self) -> [(super::ShaderType, &mut HashMap<SlotKey, PipelineStageBinder>); 4] {
        [
            (super::ShaderType::Vertex, &mut self.vertex_binder),
            (super::ShaderType::Fragment, &mut self.fragment_binder),
            (super::ShaderType::Mesh, &mut self.mesh_binder),
            (super::ShaderType::Amplification, &mut self.object_binder),
        ]
    }

    fn allocate_stage_bindings(
        &mut self,
        binder: &HashMap<SlotKey, PipelineStageBinder>,
        stage: super::ShaderType,
    ) {
        let encoder = match self.render_encoder.as_ref() {
            Some(e) => e,
            None => return,
        };

        // Bind push constants using set*Bytes (zero allocations)
        // Skip binders that have not changed since the last draw
        for b in binder.values() {
            if let PipelineStageBinder::PushConstants(pc) = b {
                if !pc.dirty {
                    continue;
                }
                let data_size = (pc.data.len() * 4) as u64;
                let data_ptr = pc.data.as_ptr() as *const std::ffi::c_void;
                let index = pc.buffer_index as u64;

                match stage {
                    super::ShaderType::Vertex => encoder.set_vertex_bytes(index, data_size, data_ptr),
                    super::ShaderType::Fragment => encoder.set_fragment_bytes(index, data_size, data_ptr),
                    super::ShaderType::Mesh => encoder.set_mesh_bytes(index, data_size, data_ptr),
                    super::ShaderType::Amplification => encoder.set_object_bytes(index, data_size, data_ptr),
                    _ => unimplemented!(),
                }
            }
        }

        let render_stage = to_mtl_render_stage(stage);

        // acceleration structures bind the heap's table of resource ids at the slot's offset, which has the same
        // layout as an argument buffer holding a single acceleration structure
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if let (true, true, Some(binding)) = (rb.acceleration_structure, rb.dirty, rb.bound_resource) {
                    let heap = unsafe { &*binding.heap_ptr };
                    if let Some(Some(slot)) = heap.acceleration_structure_slots.get(binding.offset) {
                        slot.use_resources(|r| encoder.use_resource_at(r, metal::MTLResourceUsage::Read, render_stage));
                    }
                    let index = rb.buffer_index as u64;
                    let buffer = Some(heap.acceleration_structure_argument_buffer.as_ref());
                    let offset = (binding.offset * std::mem::size_of::<metal::MTLResourceID>()) as u64;
                    match stage {
                        super::ShaderType::Vertex => encoder.set_vertex_buffer(index, buffer, offset),
                        super::ShaderType::Fragment => encoder.set_fragment_buffer(index, buffer, offset),
                        super::ShaderType::Mesh => encoder.set_mesh_buffer(index, buffer, offset),
                        super::ShaderType::Amplification => encoder.set_object_buffer(index, buffer, offset),
                        _ => unimplemented!(),
                    }
                }
            }
        }

        // Group resource bindings by buffer_index, skipping groups where nothing is dirty
        let mut groups: HashMap<u32, Vec<&ResourceBinder>> = HashMap::new();
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if rb.bound_resource.is_some() && rb.dirty && !rb.acceleration_structure {
                    groups.entry(rb.buffer_index).or_default().push(rb);
                }
            }
        }

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
                                // buffers are device-allocated (not part of mtl_heap), so make them resident
                                encoder.use_resource_at(
                                    buffer, metal::MTLResourceUsage::Read | metal::MTLResourceUsage::Write, render_stage);
                            }
                        }
                        _ => {}
                    }
                }
            }

            let index = buffer_index as u64;
            match stage {
                super::ShaderType::Vertex => encoder.set_vertex_buffer(index, Some(&arg_buffer), 0),
                super::ShaderType::Fragment => encoder.set_fragment_buffer(index, Some(&arg_buffer), 0),
                super::ShaderType::Mesh => encoder.set_mesh_buffer(index, Some(&arg_buffer), 0),
                super::ShaderType::Amplification => encoder.set_object_buffer(index, Some(&arg_buffer), 0),
                _ => unimplemented!(),
            }
            self.transient_buffers.push(arg_buffer);
        }
    }

    fn allocate_stage_resources(&mut self) {
        let binders = [
            (super::ShaderType::Vertex, self.vertex_binder.clone()),
            (super::ShaderType::Fragment, self.fragment_binder.clone()),
            (super::ShaderType::Mesh, self.mesh_binder.clone()),
            (super::ShaderType::Amplification, self.object_binder.clone()),
        ];
        for (stage, binder) in &binders {
            self.allocate_stage_bindings(binder, *stage);
        }

        // Clear dirty flags on originals now that encoding is done
        for (_, binder) in self.render_binders_mut() {
            for b in binder.values_mut() {
                match b {
                    PipelineStageBinder::PushConstants(pc) => pc.dirty = false,
                    PipelineStageBinder::Resource(rb) => rb.dirty = false,
                }
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
                let data_size = (pc.data.len() * 4) as u64;
                let data_ptr = pc.data.as_ptr() as *const std::ffi::c_void;
                encoder.set_bytes(pc.buffer_index as u64, data_size, data_ptr);
            }
        }

        // acceleration structures bind the heap's table of resource ids at the slot's offset, which has the same
        // layout as an argument buffer holding a single acceleration structure
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if let (true, true, Some(binding)) = (rb.acceleration_structure, rb.dirty, rb.bound_resource) {
                    let heap = unsafe { &*binding.heap_ptr };
                    if let Some(Some(slot)) = heap.acceleration_structure_slots.get(binding.offset) {
                        slot.use_resources(|r| encoder.use_resource(r, metal::MTLResourceUsage::Read));
                    }
                    encoder.set_buffer(
                        rb.buffer_index as u64,
                        Some(&heap.acceleration_structure_argument_buffer),
                        (binding.offset * std::mem::size_of::<metal::MTLResourceID>()) as u64
                    );
                }
            }
        }

        // explicitly bound resources, grouped by buffer_index
        let mut groups: HashMap<u32, Vec<&ResourceBinder>> = HashMap::new();
        for b in binder.values() {
            if let PipelineStageBinder::Resource(rb) = b {
                if rb.bound_resource.is_some() && rb.dirty && !rb.acceleration_structure {
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
                                // buffers are device-allocated (not part of mtl_heap), so make them resident
                                encoder.use_resource(buffer, metal::MTLResourceUsage::Read | metal::MTLResourceUsage::Write);
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
            self.pass_state = RenderPassState {
                desc: Some(render_pass.desc.clone()),
                ..Default::default()
            };
        });
    }

    fn end_render_pass(&mut self) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder.as_ref()
                .expect("hotline_rs::gfx::mtl end_render_pass called without matching begin")
                .end_encoding();
            self.render_encoder = None;
            self.pass_state = RenderPassState::default();
        });
    }

    fn begin_event(&mut self, colour: u32, name: &str) {
        if let Some(enc) = self.render_encoder.as_ref() {
            enc.push_debug_group(name);
            self.pass_state.debug_groups.push(name.to_string());
        } else if let Some(enc) = self.compute_encoder.as_ref() {
            enc.push_debug_group(name);
        } else if let Some(cmd) = self.cmd.as_ref() {
            cmd.push_debug_group(name);
        }
    }

    fn end_event(&mut self) {
        if let Some(enc) = self.render_encoder.as_ref() {
            enc.pop_debug_group();
            self.pass_state.debug_groups.pop();
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
        // Metal tracks hazards between encoders for tracked resources, so only writes within the open compute
        // encoder need a barrier to be visible to subsequent dispatches
        if let Some(encoder) = self.compute_encoder.as_ref() {
            match resource {
                UavResource::Buffer(buffer) => encoder.memory_barrier_with_resources(&[&buffer.metal_buffer]),
                UavResource::Texture(texture) => encoder.memory_barrier_with_resources(&[&texture.metal_texture]),
                UavResource::RaytracingTLAS(_) => unimplemented!(),
            }
        }
    }

    fn set_viewport(&mut self, viewport: &super::Viewport) {
        let viewport = MTLViewport {
            originX: viewport.x as f64,
            originY: viewport.y as f64,
            width: viewport.width as f64,
            height: viewport.height as f64,
            znear: viewport.min_depth as f64,
            zfar: viewport.max_depth as f64,
        };
        objc::rc::autoreleasepool(|| {
            self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
            .set_viewport(viewport);
        });
        self.pass_state.viewport = Some(viewport);
    }

    fn set_scissor_rect(&mut self, scissor_rect: &super::ScissorRect) {
        let scissor = MTLScissorRect {
            x: scissor_rect.left as u64,
            y: scissor_rect.top as u64,
            width: (scissor_rect.right - scissor_rect.left) as u64,
            height: (scissor_rect.bottom - scissor_rect.top) as u64,
        };
        objc::rc::autoreleasepool(|| {
            self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
            .set_scissor_rect(scissor);
        });
        self.pass_state.scissor = Some(scissor);
    }

    fn set_vertex_buffer(&mut self, buffer: &Buffer, slot: u32) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .set_vertex_buffer(slot as NSUInteger, Some(&buffer.metal_buffer), 0);
        });
        let vertex_buffers = &mut self.pass_state.vertex_buffers;
        vertex_buffers.retain(|(s, _)| *s != slot as NSUInteger);
        vertex_buffers.push((slot as NSUInteger, buffer.metal_buffer.clone()));
    }

    fn set_index_buffer(&mut self, buffer: &Buffer) {
        self.bound_index_buffer = Some(buffer.metal_buffer.clone());
        self.bound_index_stride = buffer.element_stride;
    }

    fn set_render_pipeline(&mut self, pipeline: &RenderPipeline) {
        objc::rc::autoreleasepool(|| {
            self.apply_render_pipeline_state(pipeline);

            // store pipeline pointer for push_render_constants
            self.bound_render_pipeline = Some(pipeline as *const RenderPipeline);
            self.bound_mesh_pipeline = None;

            // Clone binder templates from pipeline to command buffer
            self.vertex_binder = pipeline.vertex_binder.clone();
            self.fragment_binder = pipeline.fragment_binder.clone();
            self.mesh_binder.clear();
            self.object_binder.clear();
        });
    }

    fn set_mesh_pipeline(&mut self, pipeline: &MeshPipeline) {
        objc::rc::autoreleasepool(|| {
            self.apply_mesh_pipeline_state(pipeline);

            // store pipeline pointer for execute_indirect
            self.bound_mesh_pipeline = Some(pipeline as *const MeshPipeline);
            self.bound_render_pipeline = None;

            // Clone binder templates from pipeline to command buffer
            self.vertex_binder.clear();
            self.fragment_binder = pipeline.fragment_binder.clone();
            self.mesh_binder = pipeline.mesh_binder.clone();
            self.object_binder = pipeline.object_binder.clone();
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
            // Structured buffers are device-allocated (not part of mtl_heap), so use_heap does not
            // make them resident - they are reached indirectly through the bindless buffer argument
            // buffer, so without this the GPU can read unmapped memory. Textures live in mtl_heap
            // and are covered by use_heap above.
            for buffer in heap.buffer_slots.iter().flatten() {
                encoder.use_resource(buffer, metal::MTLResourceUsage::Read | metal::MTLResourceUsage::Write);
            }
            for acceleration_structure in heap.acceleration_structure_slots.iter().flatten() {
                acceleration_structure.use_resources(|r| encoder.use_resource(r, metal::MTLResourceUsage::Read));
            }
            for (_key, slot) in &cp.compute_binder {
                if let PipelineStageBinder::Resource(res) = slot {
                    let arg_buffer = match res.data_type {
                        _ if res.acceleration_structure => &heap.acceleration_structure_argument_buffer,
                        metal::MTLDataType::Texture => heap.get_texture_argument_buffer(),
                        metal::MTLDataType::Pointer => heap.get_buffer_argument_buffer(),
                        _ => continue,
                    };
                    encoder.set_buffer(res.buffer_index as u64, Some(arg_buffer), 0);
                }
            }
            return;
        }

        self.bind_heap_render(heap);
        self.pass_state.heap = Some(heap as *const Heap);
    }

    fn set_binding<T: SuperPipleline>(&mut self, _pipeline: &T, register: u32, space: u32, descriptor_type: super::DescriptorType, heap: &Heap, offset: usize) -> Option<()> {
        let key: SlotKey = (register, space, descriptor_type);
        let heap_ptr = heap as *const Heap;

        // write to each stage binder the slot is visible to
        let binders = [
            &mut self.vertex_binder,
            &mut self.fragment_binder,
            &mut self.mesh_binder,
            &mut self.object_binder,
            &mut self.compute_binder,
        ];
        for binder in binders {
            if let Some(PipelineStageBinder::Resource(ref mut rb)) = binder.get_mut(&key) {
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

        // write to each stage binder the push constants are visible to
        let mut result = None;
        for (_, binder) in self.render_binders_mut() {
            if let Some(PipelineStageBinder::PushConstants(ref mut pc)) = binder.get_mut(&key) {
                let dest_start = dest_offset as usize;
                let dest_end = dest_start + data_size_dwords;
                if dest_end <= pc.data.len() {
                    pc.data[dest_start..dest_end].copy_from_slice(data_u32);
                    pc.dirty = true;
                }
                result = Some(());
            }
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

    fn dispatch_mesh(&mut self, group_count: Size3, ms_numthreads: Size3, as_numthreads: Option<Size3>) {
        objc::rc::autoreleasepool(|| {
            self.allocate_stage_resources();

            // without an object (amplification) stage the object threadgroup size is unused
            let as_numthreads = as_numthreads.unwrap_or(Size3 { x: 1, y: 1, z: 1 });
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .draw_mesh_threadgroups(to_mtl_size(group_count), to_mtl_size(as_numthreads), to_mtl_size(ms_numthreads));
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
        // only mesh dispatch command signatures are implemented on metal so far
        let (ms_threads, as_threads) = match command.dispatch_mesh {
            Some(threads) => threads,
            None => return,
        };
        if max_command_count == 0 {
            return;
        }

        objc::rc::autoreleasepool(|| {
            // metal cannot read the count or push constants from the argument buffer, so a built in kernel
            // translates the arguments first. it must run outside of the render pass, so the pass is split
            let max = max_command_count as u64;
            let num_push_constants = command.push_constants.len() as u64;
            let private = metal::MTLResourceOptions::StorageModePrivate;
            let dispatch_args = self.metal_device.new_buffer(max * 12, private);
            let push_constants_args = self.metal_device.new_buffer(
                (max * num_push_constants * INDIRECT_PUSH_CONSTANTS_STRIDE).max(4), private);

            let mut info = IndirectClampInfo {
                stride: command.stride as u32,
                dispatch_offset: command.dispatch_mesh_offset as u32,
                max_count: max_command_count,
                has_count: counter_buffer.is_some() as u32,
                num_push_constants: num_push_constants as u32,
                push_constants_stride: INDIRECT_PUSH_CONSTANTS_STRIDE as u32,
                ..Default::default()
            };
            for (i, pc) in command.push_constants.iter().enumerate() {
                info.push_constants[i] = PushConstantsCopy {
                    src_offset: pc.offset as u32,
                    num_values: pc.num_values,
                };
            }

            let clamp_pipeline = self.indirect_clamp_pipeline.clone();
            self.split_render_pass(|encoder| {
                encoder.set_compute_pipeline_state(&clamp_pipeline);
                encoder.set_buffer(0, Some(&argument_buffer.metal_buffer), argument_buffer_offset as u64);
                if let Some(counter_buffer) = counter_buffer {
                    encoder.set_buffer(1, Some(&counter_buffer.metal_buffer), counter_buffer_offset as u64);
                }
                else {
                    encoder.set_buffer(1, Some(&argument_buffer.metal_buffer), 0);
                }
                encoder.set_buffer(2, Some(&dispatch_args), 0);
                encoder.set_buffer(3, Some(&push_constants_args), 0);
                encoder.set_bytes(
                    4,
                    std::mem::size_of::<IndirectClampInfo>() as u64,
                    &info as *const IndirectClampInfo as *const std::ffi::c_void
                );
                encoder.dispatch_thread_groups(
                    metal::MTLSize::new((max + 63) / 64, 1, 1),
                    metal::MTLSize::new(64, 1, 1)
                );
            });

            // flush the bindings into the resumed render encoder
            self.allocate_stage_resources();

            let encoder = self.render_encoder.as_ref().unwrap();
            for i in 0..max {
                // bind each command's push constants as a constant buffer, in place of the binder's bytes
                for (p, pc) in command.push_constants.iter().enumerate() {
                    let offset = (i * num_push_constants + p as u64) * INDIRECT_PUSH_CONSTANTS_STRIDE;
                    let stages = [
                        (super::ShaderType::Mesh, &self.mesh_binder),
                        (super::ShaderType::Amplification, &self.object_binder),
                        (super::ShaderType::Fragment, &self.fragment_binder),
                    ];
                    for (stage, binder) in stages {
                        if let Some(PipelineStageBinder::PushConstants(b)) = binder.get(&pc.key) {
                            let index = b.buffer_index as u64;
                            match stage {
                                super::ShaderType::Mesh => encoder.set_mesh_buffer(index, Some(&push_constants_args), offset),
                                super::ShaderType::Amplification => encoder.set_object_buffer(index, Some(&push_constants_args), offset),
                                _ => encoder.set_fragment_buffer(index, Some(&push_constants_args), offset),
                            }
                        }
                    }
                }
                encoder.draw_mesh_threadgroups_with_indirect_buffer(
                    &dispatch_args,
                    i * 12,
                    to_mtl_size(as_threads.unwrap_or(Size3 { x: 1, y: 1, z: 1 })),
                    to_mtl_size(ms_threads)
                );
            }

            // the push constant slots now hold the argument buffer, so rebind the cpu values on the next draw
            for (_, binder) in self.render_binders_mut() {
                for pc in &command.push_constants {
                    if let Some(PipelineStageBinder::PushConstants(b)) = binder.get_mut(&pc.key) {
                        b.dirty = true;
                    }
                }
            }

            self.transient_buffers.push(dispatch_args);
            self.transient_buffers.push(push_constants_args);
        });
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
        objc::rc::autoreleasepool(|| {
            assert!(self.render_encoder.is_none(),
                "hotline_rs::gfx::mtl update_raytracing_tlas cannot be called inside a render pass");

            // close any open compute encoder - Metal forbids two live encoders on one cmd buffer
            if let Some(enc) = self.compute_encoder.take() {
                enc.end_encoding();
            }

            let blases = instance_buffer.instance_acceleration_structures.clone();
            let desc = instance_acceleration_structure_descriptor(
                &instance_buffer.metal_buffer, instance_count, &blases, tlas.usage);

            let encoder = self.cmd.as_ref()
                .expect("hotline_rs::gfx::mtl expected call to CmdBuf::reset before update_raytracing_tlas")
                .new_acceleration_structure_command_encoder();

            // refit in place when the tlas was built to allow it, otherwise rebuild
            let refit = matches!(mode, AccelerationStructureRebuildMode::Refit)
                && tlas.usage & MTL_ACCELERATION_STRUCTURE_USAGE_REFIT != 0;
            if refit {
                let destination: *const metal::AccelerationStructureRef = std::ptr::null();
                unsafe {
                    let _: () = msg_send![encoder,
                        refitAccelerationStructure: &*tlas.acceleration_structure
                        descriptor: &*desc
                        destination: destination
                        scratchBuffer: &*tlas.scratch_buffer
                        scratchBufferOffset: 0 as NSUInteger];
                }
            }
            else {
                encoder.build_acceleration_structure(&tlas.acceleration_structure, &desc, &tlas.scratch_buffer, 0);
            }
            encoder.end_encoding();

            *tlas.blases.lock().unwrap() = blases;
        });
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
    /// For instance buffers from `create_raytracing_instance_buffer`, the BLASes the instances index
    instance_acceleration_structures: Vec<metal::AccelerationStructure>,
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
        // buffers are StorageModeShared so are always cpu visible, return the base address like d3d12
        self.metal_buffer.contents() as *mut u8
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
    /// Acceleration structures are not encoded into an argument buffer, they are bound from the heap's table of
    /// acceleration structure resource ids, see `Heap::acceleration_structure_argument_buffer`
    pub acceleration_structure: bool,
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

pub struct MeshPipeline {
    pipeline_state: metal::RenderPipelineState,
    slots: Vec<u32>,
    /// Unified slot lookup by (register, space, descriptor_type)
    slot_lookup: HashMap<SlotKey, PipelineSlotInfo>,
    /// Static samplers
    static_samplers: Vec<MetalSamplerBinding>,
    /// Sampler argument buffer, bound at buffer(0) of each stage
    sampler_argument_buffer: Option<metal::Buffer>,
    /// Object (amplification) stage binders for push constants and resources, keyed by (register, space, descriptor_type)
    object_binder: HashMap<SlotKey, PipelineStageBinder>,
    /// Mesh stage binders for push constants and resources, keyed by (register, space, descriptor_type)
    mesh_binder: HashMap<SlotKey, PipelineStageBinder>,
    /// Fragment stage binders for push constants and resources, keyed by (register, space, descriptor_type)
    fragment_binder: HashMap<SlotKey, PipelineStageBinder>,
    /// Depth stencil state
    depth_stencil_state: metal::DepthStencilState,
    /// Rasterizer state (applied dynamically on encoder in Metal)
    raster_info: super::RasterInfo,
}

impl super::MeshPipeline<Device> for MeshPipeline {}

unsafe impl Send for MeshPipeline {}
unsafe impl Sync for MeshPipeline {}

impl super::Pipeline for MeshPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        self.slot_lookup.get(&(register, space, descriptor_type))
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Render
    }
}

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
}

/// The BLASes instanced by a TLAS. metal does not follow the references inside an acceleration structure, so they
/// must be made resident wherever the TLAS is used. Shared between the TLAS and its heap slot so
/// `update_raytracing_tlas` can change them
type SharedAccelerationStructures = std::sync::Arc<std::sync::Mutex<Vec<metal::AccelerationStructure>>>;

/// A TLAS allocated in a heap with the BLASes it instances
#[derive(Clone)]
struct HeapAccelerationStructure {
    tlas: metal::AccelerationStructure,
    blases: SharedAccelerationStructures,
}

impl HeapAccelerationStructure {
    /// Call `use_resource` with the TLAS and each of its BLASes to make them resident
    fn use_resources<F: Fn(&metal::ResourceRef)>(&self, use_resource: F) {
        use_resource(&self.tlas);
        for blas in self.blases.lock().unwrap().iter() {
            use_resource(blas);
        }
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
    /// TLASes allocated in the heap
    acceleration_structure_slots: Vec<Option<HeapAccelerationStructure>>,
    /// gpuResourceIDs of the TLASes in the heap, which is the layout of an argument buffer array of acceleration
    /// structures. Bound whole for bindless access, or at an offset of `slot * 8` for a single acceleration structure
    acceleration_structure_argument_buffer: metal::Buffer,
}

impl Heap {
    fn allocate(&mut self) -> usize {
        let srv = self.offset;
        self.offset += 1;
        unsafe {
            self.texture_slots.resize(self.offset, None);
            self.buffer_slots.resize(self.offset, None);
        }
        self.acceleration_structure_slots.resize(self.offset, None);
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

    /// Store a TLAS at the given index and write its gpuResourceID into the acceleration structure argument buffer
    fn encode_acceleration_structure(&mut self, index: usize, acceleration_structure: HeapAccelerationStructure) {
        let capacity = self.acceleration_structure_argument_buffer.length() as usize / std::mem::size_of::<metal::MTLResourceID>();
        assert!(index < capacity, "hotline_rs::gfx::mtl: heap is full, cannot allocate acceleration structure at index {}", index);
        let id: metal::MTLResourceID = unsafe { msg_send![&*acceleration_structure.tlas, gpuResourceID] };
        unsafe {
            let ids = self.acceleration_structure_argument_buffer.contents() as *mut metal::MTLResourceID;
            *ids.add(index) = id;
        }
        self.acceleration_structure_slots[index] = Some(acceleration_structure);
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

/// Push constants written by an indirect command, `offset` is the byte offset in each command's arguments
struct IndirectPushConstants {
    key: SlotKey,
    offset: usize,
    num_values: u32,
}

#[derive(Default)]
pub struct CommandSignature {
    /// Byte stride of each command in the argument buffer
    stride: usize,
    /// Byte offset of the dispatch mesh arguments in each command
    dispatch_mesh_offset: usize,
    /// Mesh and amplification threadgroup sizes for mesh dispatch signatures, None for other signatures
    dispatch_mesh: Option<(Size3, Option<Size3>)>,
    push_constants: Vec<IndirectPushConstants>,
}

/// Raytracing pipelines are not supported on metal, which only has inline raytracing, see `create_raytracing_pipeline`
pub struct RaytracingPipeline {
    slots: Vec<u32>,
}

impl super::Pipeline for RaytracingPipeline {
    fn get_pipeline_slot(&self, _register: u32, _space: u32, _descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        None
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Compute
    }
}

pub struct RaytracingShaderBindingTable {

}

pub struct RaytracingBLAS {
    acceleration_structure: metal::AccelerationStructure,
}

pub struct RaytracingTLAS {
    acceleration_structure: metal::AccelerationStructure,
    blases: SharedAccelerationStructures,
    /// Scratch buffer for rebuilding or refitting in `update_raytracing_tlas`
    scratch_buffer: metal::Buffer,
    /// MTLAccelerationStructureUsage the TLAS was built with
    usage: NSUInteger,
    srv_index: Option<usize>,
    heap_id: Option<u16>,
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

            /*
            // newHeapWithDescriptor: returns nil on allocation failure (e.g. requested size
            // exceeds what the GPU can back). metal-rs wraps the result without checking, so
            // every later deref of a nil heap would trip foreign-types' from_ptr assert with
            // no useful context. Probe first via raw msg_send so we can report what was
            // actually requested before falling back to metal-rs's wrapper.
            let probe_ptr: *mut objc::runtime::Object = unsafe {
                msg_send![&*mtl_device, newHeapWithDescriptor: &*heap_descriptor]
            };
            assert!(
                !probe_ptr.is_null(),
                "hotline_rs::gfx::mtl: failed to allocate MTLHeap ({:.1} MB, {} descriptors, storage Private). \
                 Requested size = texture_size({} B) * num_descriptors({}) * HEAP_OVERSIZE_FACTOR({}). \
                 Reduce num_descriptors in HeapInfo, or use a smaller per-batch heap for buffer-only allocations.",
                heap_size as f64 / (1024.0 * 1024.0),
                info.num_descriptors,
                texture_size,
                info.num_descriptors.max(1),
                HEAP_OVERSIZE_FACTOR
            );
            // probe_ptr is a +1 retained heap; release it and let metal-rs allocate again so
            // we keep using its wrapper type without juggling raw pointer ownership.
            unsafe { let _: () = msg_send![probe_ptr, release]; }
            */

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

            // table of acceleration structure gpuResourceIDs for bindless access
            let acceleration_structure_argument_buffer = mtl_device.new_buffer(
                max_resources * std::mem::size_of::<metal::MTLResourceID>() as u64,
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
            acceleration_structure_slots: Vec::new(),
            acceleration_structure_argument_buffer,
        }
    }

    /// Create a buffer and register it in `heap` for bindless and `set_binding` access. Takes the metal device
    /// rather than self so `create_buffer` can pass the device shader heap
    fn create_buffer_mtl<T: Sized>(
        metal_device: &metal::Device,
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
                metal_device.new_buffer_with_data(bytes, byte_len, opt)
            }
            else {
                metal_device.new_buffer(byte_len, opt)
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
                instance_acceleration_structures: Vec::new(),
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

    /// Build an acceleration structure on the device queue and wait for it to complete. Returns the acceleration
    /// structure and a scratch buffer big enough to rebuild or refit it later
    fn build_acceleration_structure_mtl(
        metal_device: &metal::Device,
        command_queue: &metal::CommandQueue,
        desc: &metal::AccelerationStructureDescriptorRef
    ) -> (metal::AccelerationStructure, metal::Buffer) {
        let sizes = metal_device.acceleration_structure_sizes_with_descriptor(desc);
        let acceleration_structure = metal_device.new_acceleration_structure_with_size(sizes.acceleration_structure_size);
        let scratch_buffer = metal_device.new_buffer(
            sizes.build_scratch_buffer_size.max(sizes.refit_scratch_buffer_size).max(1),
            metal::MTLResourceOptions::StorageModePrivate
        );

        let cmd = command_queue.new_command_buffer();
        let encoder = cmd.new_acceleration_structure_command_encoder();
        encoder.build_acceleration_structure(&acceleration_structure, desc, &scratch_buffer, 0);
        encoder.end_encoding();
        cmd.commit();
        cmd.wait_until_completed();

        (acceleration_structure, scratch_buffer)
    }

    /// Create a TLAS and allocate it in `heap`. Takes the metal device and queue rather than self so
    /// `create_raytracing_tlas` can pass the device shader heap
    fn create_raytracing_tlas_mtl(
        metal_device: &metal::Device,
        command_queue: &metal::CommandQueue,
        info: &RaytracingTLASInfo<Device>,
        heap: &mut Heap
    ) -> result::Result<RaytracingTLAS, super::Error> {
        objc::rc::autoreleasepool(|| {
            let (descriptors, blases) = to_mtl_instance_descriptors(info.instances);
            let instance_buffer = metal_device.new_buffer_with_data(
                descriptors.as_ptr() as *const std::ffi::c_void,
                (descriptors.len().max(1) * std::mem::size_of::<metal::MTLAccelerationStructureUserIDInstanceDescriptor>()) as u64,
                metal::MTLResourceOptions::StorageModeShared
            );

            let usage = to_mtl_acceleration_structure_usage(info.build_flags);
            let desc = instance_acceleration_structure_descriptor(&instance_buffer, descriptors.len(), &blases, usage);
            let (acceleration_structure, scratch_buffer) = Self::build_acceleration_structure_mtl(metal_device, command_queue, &desc);

            let blases = std::sync::Arc::new(std::sync::Mutex::new(blases));
            let srv_index = heap.allocate();
            heap.encode_acceleration_structure(srv_index, HeapAccelerationStructure {
                tlas: acceleration_structure.clone(),
                blases: blases.clone(),
            });

            Ok(RaytracingTLAS {
                acceleration_structure,
                blases,
                scratch_buffer,
                usage,
                srv_index: Some(srv_index),
                heap_id: Some(heap.id),
            })
        })
    }

    fn create_depth_stencil_state(&self, ds_info: &super::DepthStencilInfo) -> metal::DepthStencilState {
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
    }

    /// Create the static samplers of a pipeline layout and an argument buffer holding them, to bind at buffer(0) of
    /// the stages that use them. SPIRV-Cross repacks the samplers actually used by a shader into
    /// spvDescriptorSetBuffer0 with sequential ids starting at 0 (it does NOT preserve the HLSL register, eg.
    /// sampler_wrap_linear at s1 becomes [[id(0)]]). So encode each sampler at its position in the static_samplers
    /// list, which matches that packing order.
    fn create_static_samplers(&self, static_samplers: &Option<Vec<super::SamplerBinding>>) -> (Vec<MetalSamplerBinding>, Option<metal::Buffer>) {
        let mut pipeline_static_samplers = Vec::new();
        for sampler in static_samplers.iter().flatten() {
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

        if pipeline_static_samplers.is_empty() {
            return (pipeline_static_samplers, None);
        }

        let arg_desc = metal::ArgumentDescriptor::new();
        arg_desc.set_index(0);
        arg_desc.set_data_type(metal::MTLDataType::Sampler);
        arg_desc.set_array_length(pipeline_static_samplers.len() as u64);
        arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);

        let argument_encoder = self.metal_device.new_argument_encoder(
            metal::Array::from_owned_slice(&[arg_desc.to_owned()])
        );
        let arg_buffer = self.metal_device.new_buffer(
            argument_encoder.encoded_length(),
            metal::MTLResourceOptions::StorageModeShared
        );

        // Encode each sampler at its packed id (list position)
        argument_encoder.set_argument_buffer(&arg_buffer, 0);
        for (id, s) in pipeline_static_samplers.iter().enumerate() {
            argument_encoder.set_sampler_state(id as u64, &s.sampler);
        }

        (pipeline_static_samplers, Some(arg_buffer))
    }

    /// Build the per-stage binders and the unified slot lookup for a pipeline. `stages` lists each shader stage
    /// with its own metal [[buffer(N)]] table as (visibility, samplers_offset). Each stage's table holds the static
    /// sampler descriptor set at `samplers_offset`, then the push constants visible to the stage in layout order, then
    /// one descriptor set per (register_kind, register, space) group, which must mirror the descriptor set allocation
    /// htwv uses when compiling the stage. Each group holds exactly one binding so it always sits at id(0).
    /// The slot lookup reports the buffer index in the first stage the slot is visible to.
    fn build_stage_binders<const N: usize>(
        &self,
        pipeline_bindings: &Option<Vec<DescriptorBinding>>,
        pipeline_push_constants: &Option<Vec<PushConstantInfo>>,
        stages: [(ShaderVisibility, u32); N],
    ) -> ([HashMap<SlotKey, PipelineStageBinder>; N], HashMap<SlotKey, PipelineSlotInfo>) {
        const MAX_BINDLESS_TEXTURES: u64 = 1024;

        let mut slot_lookup: HashMap<SlotKey, PipelineSlotInfo> = HashMap::new();

        let binders = stages.map(|(stage, samplers_offset)| {
            let visible = |v: ShaderVisibility| v == stage || v == ShaderVisibility::All;
            let mut binder: HashMap<SlotKey, PipelineStageBinder> = HashMap::new();
            let mut binding_offset = samplers_offset + 1;

            // push constants use setBytes, no argument encoder
            for push_constant in pipeline_push_constants.iter().flatten().filter(|pc| visible(pc.visibility)) {
                let key: SlotKey = (push_constant.shader_register, push_constant.register_space, DescriptorType::PushConstants);
                let buffer_index = binding_offset;
                binding_offset += 1;
                binder.insert(key, PipelineStageBinder::PushConstants(PushConstantsBinder {
                    // msl pads constant buffer structs to 16 bytes, and metal validates the bound size covers them
                    data: vec![0u32; (push_constant.num_values as usize + 3) & !3],
                    num_32_bit_constants: push_constant.num_values,
                    buffer_index,
                    dirty: true,
                }));
                slot_lookup.entry(key).or_insert(PipelineSlotInfo {
                    index: buffer_index,
                    count: Some(push_constant.num_values),
                });
            }

            // resource bindings grouped by (register_kind, shader_register, register_space), space is part of the
            // key so bindless arrays sharing a register (eg. textures t1/space7, cubemaps t1/space9) get their own set
            let mut groups: HashMap<(char, u32, u32), u32> = HashMap::new();
            for binding in pipeline_bindings.iter().flatten().filter(|b| visible(b.visibility)) {
                let key: SlotKey = (binding.shader_register, binding.register_space, binding.binding_type);
                let group_key = (descriptor_register_kind(binding.binding_type), binding.shader_register, binding.register_space);
                let buffer_index = *groups.entry(group_key).or_insert_with(|| {
                    let idx = binding_offset;
                    binding_offset += 1;
                    idx
                });
                let resource_type = binding.resource_type.expect("hotline_rs::gfx::mtl: requires resource type for binding");
                binder.insert(key, PipelineStageBinder::Resource(ResourceBinder {
                    buffer_index,
                    binding_index: 0,
                    data_type: to_mtl_data_type(resource_type),
                    acceleration_structure: matches!(resource_type, super::ResourceType::RaytracingAccelerationStructure),
                    array_length: binding.num_descriptors.map(|n| n as u64).unwrap_or(MAX_BINDLESS_TEXTURES),
                    bound_resource: None,
                    dirty: true,
                }));
                slot_lookup.entry(key).or_insert(PipelineSlotInfo {
                    index: buffer_index,
                    count: binding.num_descriptors,
                });
            }

            binder
        });

        (binders, slot_lookup)
    }
}

impl super::Device for Device {
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Buffer = Buffer;
    type Shader = Shader;
    type RenderPipeline = RenderPipeline;
    type MeshPipeline = MeshPipeline;
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

            let indirect_clamp_pipeline = {
                let lib = device.new_library_with_source(INDIRECT_CLAMP_MSL, &metal::CompileOptions::new())
                    .expect("hotline_rs::gfx::mtl: failed to compile indirect clamp kernel");
                let function = lib.get_function("indirect_clamp", None)
                    .expect("hotline_rs::gfx::mtl: failed to find indirect clamp kernel");
                device.new_compute_pipeline_state_with_function(&function)
                    .expect("hotline_rs::gfx::mtl: failed to create indirect clamp pipeline")
            };

            let mut feature_flags = DeviceFeatureFlags::NONE;
            if device.supports_raytracing() {
                feature_flags |= DeviceFeatureFlags::RAYTRACING;
            }
            if device.supports_family(metal::MTLGPUFamily::Metal3) {
                feature_flags |= DeviceFeatureFlags::MESH_SAHDER;
            }

            Device {
                command_queue: command_queue,
                indirect_clamp_pipeline,
                feature_flags,
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
        &self.feature_flags
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
                bound_mesh_pipeline: None,
                bound_compute_pipeline: None,
                metal_device: self.metal_device.clone(),
                transient_buffers: Vec::new(),
                vertex_binder: HashMap::new(),
                fragment_binder: HashMap::new(),
                object_binder: HashMap::new(),
                mesh_binder: HashMap::new(),
                compute_binder: HashMap::new(),
                deferred_ops: Vec::new(),
                pending_timestamp: None,
                pass_state: RenderPassState::default(),
                indirect_clamp_pipeline: self.indirect_clamp_pipeline.clone(),
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

            setup_colour_attachments(pipeline_state_descriptor.color_attachments(), &info.blend_info, info.pass);

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
            let depth_stencil_state = self.create_depth_stencil_state(&info.depth_stencil_info);

            // Create static samplers and argument buffer (bound at fragment buffer(0))
            let (pipeline_static_samplers, sampler_argument_buffer) = self.create_static_samplers(&info.pipeline_layout.static_samplers);

            // Build stage binders for push constants and resource bindings, vertex buffers occupy buffer(0..1)
            let ([vertex_binder, fragment_binder], slot_lookup) = self.build_stage_binders(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
                [(ShaderVisibility::Vertex, 2), (ShaderVisibility::Fragment, 0)],
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

    fn create_mesh_pipeline(
        &self,
        info: &super::MeshPipelineInfo<Device>,
    ) -> std::result::Result<MeshPipeline, super::Error> {
        objc::rc::autoreleasepool(|| {
            if !self.metal_device.supports_family(metal::MTLGPUFamily::Metal3) {
                return Err(super::Error {
                    msg: "hotline_rs::gfx::mtl: mesh pipelines require a Metal3 capable device".to_string()
                });
            }

            let get_function = |shader: &Shader| -> result::Result<metal::Function, super::Error> {
                let name = &shader.lib.function_names()[0];
                Ok(shader.lib.get_function(name, None)?)
            };

            let desc = metal::MeshRenderPipelineDescriptor::new();

            let ms = info.ms.ok_or(super::Error {
                msg: "hotline_rs::gfx::mtl: mesh pipeline requires a mesh shader".to_string()
            })?;
            let ms_function = get_function(ms)?;
            desc.set_mesh_function(Some(&ms_function));

            if let Some(amps) = info.amps {
                let as_function = get_function(amps)?;
                desc.set_object_function(Some(&as_function));
            }

            if let Some(fs) = info.fs {
                let fs_function = get_function(fs)?;
                desc.set_fragment_function(Some(&fs_function));
            }

            setup_colour_attachments(desc.color_attachments(), &info.blend_info, info.pass);
            desc.set_alpha_to_coverage_enabled(info.blend_info.alpha_to_coverage_enabled);

            // depth format + MSAA sample count to match the pass
            if let Some(pass) = &info.pass {
                if let Some(depth_format) = pass.depth_format {
                    desc.set_depth_attachment_pixel_format(depth_format);
                    if has_stencil_component(depth_format) {
                        desc.set_stencil_attachment_pixel_format(depth_format);
                    }
                }
                desc.set_raster_sample_count(pass.sample_count as NSUInteger);
            }

            let pipeline_state = self.metal_device.new_mesh_render_pipeline_state(&desc)?;

            let (static_samplers, sampler_argument_buffer) = self.create_static_samplers(&info.pipeline_layout.static_samplers);

            // mesh first so the slot lookup reports mesh stage buffer indices, which execute_indirect binds
            let ([mesh_binder, object_binder, fragment_binder], slot_lookup) = self.build_stage_binders(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
                [(ShaderVisibility::Mesh, 0), (ShaderVisibility::Amplification, 0), (ShaderVisibility::Fragment, 0)],
            );

            Ok(MeshPipeline {
                pipeline_state,
                slots: Vec::new(),
                slot_lookup,
                static_samplers,
                sampler_argument_buffer,
                object_binder,
                mesh_binder,
                fragment_binder,
                depth_stencil_state: self.create_depth_stencil_state(&info.depth_stencil_info),
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
        Self::create_buffer_mtl(&self.metal_device, info, data, heap)
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        Self::create_buffer_mtl(&self.metal_device, info, data, &mut self.shader_heap)
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
                instance_acceleration_structures: Vec::new(),
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
        objc::rc::autoreleasepool(|| {
            let geometry: metal::AccelerationStructureGeometryDescriptor = match &info.geometry {
                super::RaytracingGeometryInfo::Triangles(tris) => {
                    if tris.transform3x4.is_some() {
                        return Err(super::Error {
                            msg: "hotline_rs::gfx::mtl: blas transform3x4 is not yet supported".to_string()
                        });
                    }
                    let desc = retain_descriptor(metal::AccelerationStructureTriangleGeometryDescriptor::descriptor());
                    desc.set_vertex_buffer(Some(&tris.vertex_buffer.metal_buffer));
                    desc.set_vertex_stride(tris.vertex_stride as NSUInteger);
                    desc.set_vertex_format(to_mtl_acceleration_structure_vertex_format(tris.vertex_format)?);
                    desc.set_index_buffer(Some(&tris.index_buffer.metal_buffer));
                    desc.set_index_type(to_mtl_acceleration_structure_index_type(tris.index_format)?);
                    desc.set_triangle_count((tris.index_count / 3) as NSUInteger);
                    desc.set_opaque(info.geometry_flags.contains(super::RaytracingGeometryFlags::OPAQUE));
                    let parent: &metal::AccelerationStructureGeometryDescriptorRef = &desc;
                    parent.to_owned()
                }
                super::RaytracingGeometryInfo::AABBs(aabbs) => {
                    let buffer = aabbs.aabbs.ok_or(super::Error {
                        msg: "hotline_rs::gfx::mtl: blas aabbs requires an aabb buffer".to_string()
                    })?;
                    // d3d12 and metal aabbs are both 6 floats (min xyz, max xyz)
                    let desc = retain_descriptor(metal::AccelerationStructureBoundingBoxGeometryDescriptor::descriptor());
                    desc.set_bounding_box_buffer(Some(&buffer.metal_buffer));
                    desc.set_bounding_box_count(aabbs.aabb_count as NSUInteger);
                    desc.set_opaque(info.geometry_flags.contains(super::RaytracingGeometryFlags::OPAQUE));
                    let parent: &metal::AccelerationStructureGeometryDescriptorRef = &desc;
                    parent.to_owned()
                }
            };

            let desc = retain_descriptor(metal::PrimitiveAccelerationStructureDescriptor::descriptor());
            desc.set_geometry_descriptors(metal::Array::from_slice(&[geometry.as_ref()]));
            let usage = to_mtl_acceleration_structure_usage(info.build_flags);
            unsafe { let _: () = msg_send![&*desc, setUsage: usage]; }

            let (acceleration_structure, _) = Self::build_acceleration_structure_mtl(
                &self.metal_device, &self.command_queue, &desc);

            Ok(RaytracingBLAS {
                acceleration_structure
            })
        })
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
            let ([compute_binder], slot_lookup) = self.build_stage_binders(
                &info.pipeline_layout.bindings,
                &info.pipeline_layout.push_constants,
                [(ShaderVisibility::Compute, 0)],
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
        Ok(CommandSignature::default())
    }

    fn create_indirect_mesh_command<T: Sized>(&mut self,
        arguments: Vec<super::IndirectArgument>,
        pipeline: Option<&MeshPipeline>,
        ms_numthreads: Size3,
        as_numthreads: Option<Size3>) -> result::Result<CommandSignature, super::Error> {
        let err = |msg: &str| super::Error { msg: format!("hotline_rs::gfx::mtl: create_indirect_mesh_command: {}", msg) };

        let mut signature = CommandSignature {
            stride: std::mem::size_of::<T>(),
            dispatch_mesh: Some((ms_numthreads, as_numthreads)),
            ..Default::default()
        };
        if signature.stride % 4 != 0 {
            return Err(err("argument stride must be a multiple of 4 bytes"));
        }

        // arguments are tightly packed in the order supplied, same as d3d12
        let mut offset = 0;
        let mut has_dispatch = false;
        for argument in arguments {
            match argument.argument_type {
                super::IndirectArgumentType::PushConstants => {
                    let pc = unsafe { argument.arguments.as_ref().ok_or(err("push constants requires arguments"))?.push_constants };
                    let pipeline = pipeline.ok_or(err("push constants requires a pipeline"))?;
                    // find the push constants for the slot, slots are mesh stage buffer indices where the stage has them
                    let key = [&pipeline.mesh_binder, &pipeline.object_binder, &pipeline.fragment_binder].iter()
                        .flat_map(|binder| binder.iter())
                        .find_map(|(key, binder)| match binder {
                            PipelineStageBinder::PushConstants(b) if b.buffer_index == pc.slot => Some(*key),
                            _ => None
                        })
                        .ok_or(err(&format!("no push constants found for slot {}", pc.slot)))?;
                    // the whole constant buffer is bound from the arguments, so they must supply all of it
                    let size = pipeline.slot_lookup.get(&key).and_then(|s| s.count).unwrap_or(0);
                    if pc.offset != 0 || pc.num_values != size {
                        return Err(err("push constants must supply all values of the constant buffer"));
                    }
                    if signature.push_constants.len() == MAX_INDIRECT_PUSH_CONSTANTS {
                        return Err(err("too many push constants arguments"));
                    }
                    signature.push_constants.push(IndirectPushConstants {
                        key,
                        offset,
                        num_values: pc.num_values,
                    });
                    offset += pc.num_values as usize * 4;
                }
                super::IndirectArgumentType::DispatchMesh => {
                    signature.dispatch_mesh_offset = offset;
                    has_dispatch = true;
                    offset += std::mem::size_of::<super::DispatchArguments>();
                }
                _ => {
                    return Err(err("only PushConstants and DispatchMesh arguments are supported"));
                }
            }
        }

        if !has_dispatch {
            return Err(err("arguments require a DispatchMesh argument"));
        }

        Ok(signature)
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
        let (descriptors, blases) = to_mtl_instance_descriptors(instances);
        let mut buffer = self.create_buffer::<metal::MTLAccelerationStructureUserIDInstanceDescriptor>(&BufferInfo {
            usage: BufferUsage::NONE,
            cpu_access: CpuAccessFlags::WRITE,
            format: super::Format::Unknown,
            stride: std::mem::size_of::<metal::MTLAccelerationStructureUserIDInstanceDescriptor>(),
            num_elements: descriptors.len().max(1),
            initial_state: ResourceState::GenericRead,
        }, Some(&descriptors))?;
        buffer.instance_acceleration_structures = blases;
        Ok(buffer)
    }

    fn create_raytracing_tlas(
        &mut self,
        info: &RaytracingTLASInfo<Self>
    ) -> Result<Self::RaytracingTLAS, Error> {
        Self::create_raytracing_tlas_mtl(&self.metal_device, &self.command_queue, info, &mut self.shader_heap)
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
        Self::create_raytracing_tlas_mtl(&self.metal_device, &self.command_queue, info, heap)
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
        self.srv_index
    }

    fn get_shader_heap_id(&self) -> u16 {
        self.heap_id.expect("hotline_rs::gfx::mtl: expected tlas to be allocated in a heap")
    }
}
