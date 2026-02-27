#![cfg(target_os = "macos")]

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

#[derive(Clone)]
pub struct Device {
    metal_device: metal::Device,
    command_queue: metal::CommandQueue,
    shader_heap: Heap,
    adapter_info: AdapterInfo
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
}

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
    }

    fn wait_for_last_frame(&self) {
    }

    fn get_num_buffers(&self) -> u32 {
        3
    }

    fn get_frame_fence_value(&self) -> u64 {
        0
    }

    fn update<A: os::App>(&mut self, device: &mut Device, window: &A::Window, cmd: &mut CmdBuf) -> bool {
        objc::rc::autoreleasepool(|| {
            let draw_size = window.get_size();
            self.layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

            let drawable = self.layer.next_drawable()
                .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

            self.drawable = drawable.to_owned();

            self.backbuffer_texture = Texture {
                metal_texture: drawable.texture().to_owned(),
                srv_index: None,
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
            let cmd = device.command_queue.new_command_buffer();
            cmd.present_drawable(&self.drawable);
            cmd.commit();
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
    /// Raw pointer to bound render pipeline (valid during render pass)
    bound_render_pipeline: Option<*const RenderPipeline>,
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
        }
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        objc::rc::autoreleasepool(|| {
            self.cmd = Some(self.cmd_queue.new_command_buffer().to_owned());
        });
    }

    fn close(&mut self) -> result::Result<(), super::Error> {
        objc::rc::autoreleasepool(|| {
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
    }

    fn end_event(&mut self) {
    }

    fn timestamp_query(&mut self, heap: &mut QueryHeap, resolve_buffer: &mut Buffer) {
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
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .set_render_pipeline_state(&pipeline.pipeline_state);

            // Bind sampler argument buffer at buffer(0) in fragment shader
            if let Some(ref sampler_arg_buffer) = pipeline.sampler_argument_buffer {
                self.render_encoder.as_ref().unwrap().set_fragment_buffer(
                    0,
                    Some(sampler_arg_buffer),
                    0
                );
            }

            // Store pipeline pointer for push_render_constants
            self.bound_render_pipeline = Some(pipeline as *const RenderPipeline);
        });
    }

    fn set_compute_pipeline(&mut self, pipeline: &ComputePipeline) {

    }

    fn set_raytracing_pipeline(&mut self, pipeline: &RaytracingPipeline) {
        unimplemented!()
    }

    fn set_heap<T: SuperPipleline>(&mut self, pipeline: &T, heap: &Heap) {
        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        // Make the heap accessible to shaders
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Fragment);
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Vertex);

        // Cast pipeline to RenderPipeline to access slot_lookup
        let rp: &RenderPipeline = unsafe { std::mem::transmute(pipeline) };

        // Track which argument buffers we've already bound (they're shared across texture slots)
        let mut bound_vertex_buffers: std::collections::HashSet<u64> = std::collections::HashSet::new();
        let mut bound_fragment_buffers: std::collections::HashSet<u64> = std::collections::HashSet::new();

        // Encode textures and bind shared argument buffer
        for ((_register, _space, descriptor_type), slot) in &rp.slot_lookup {
            // Only process ShaderResource bindings (textures)
            if *descriptor_type != DescriptorType::ShaderResource {
                continue;
            }
            // Skip push constants (they have data_buffer)
            if slot.data_buffer.is_some() {
                continue;
            }

            // Set up the argument buffer for encoding
            slot.argument_encoder.set_argument_buffer(&slot.argument_buffer, 0);

            // Check if this is an array binding (bindless) or single texture (bindful)
            let count = slot.info.count.unwrap_or(1) as usize;
            if count > 1 {
                // Array binding (bindless) - encode ALL textures from heap
                for i in 0..count.min(heap.texture_slots.len()) {
                    if let Some(texture) = heap.texture_slots.get(i).and_then(|t| t.as_ref()) {
                        slot.argument_encoder.set_texture(i as u64, texture);
                    }
                }
            } else {
                // Single texture binding (bindful) - encode at binding_index
                if let Some(texture) = heap.texture_slots.get(slot.binding_index as usize).and_then(|t| t.as_ref()) {
                    slot.argument_encoder.set_texture(slot.binding_index as u64, texture);
                }
            }

            // Bind the argument buffer only once per stage (it's shared across all texture slots)
            if let Some(vertex_idx) = slot.vertex_buffer_index {
                if bound_vertex_buffers.insert(vertex_idx as u64) {
                    encoder.set_vertex_buffer(vertex_idx as u64, Some(&slot.argument_buffer), 0);
                }
            }
            if let Some(fragment_idx) = slot.fragment_buffer_index {
                if bound_fragment_buffers.insert(fragment_idx as u64) {
                    encoder.set_fragment_buffer(fragment_idx as u64, Some(&slot.argument_buffer), 0);
                }
            }
        }
    }

    fn set_binding<T: SuperPipleline>(&mut self, pipeline: &T, register: u32, space: u32, descriptor_type: super::DescriptorType, heap: &Heap, offset: usize) -> Option<()> {
        let encoder = self.render_encoder.as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        // Make the heap accessible to shaders
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Fragment);
        encoder.use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Vertex);

        let rp: &RenderPipeline = unsafe { std::mem::transmute(pipeline) };

        // Look up the slot by (register, space, descriptor_type)
        if let Some(slot) = rp.slot_lookup.get(&(register, space, descriptor_type)) {
            slot.argument_encoder.set_argument_buffer(&slot.argument_buffer, 0);

            // Set texture from heap at offset, using binding_index for position in shared buffer
            if let Some(texture) = heap.texture_slots.get(offset).and_then(|t| t.as_ref()) {
                slot.argument_encoder.set_texture(slot.binding_index as u64, texture);
            }

            // Bind to appropriate stage(s)
            if let Some(vertex_idx) = slot.vertex_buffer_index {
                encoder.set_vertex_buffer(vertex_idx as u64, Some(&slot.argument_buffer), 0);
            }
            if let Some(fragment_idx) = slot.fragment_buffer_index {
                encoder.set_fragment_buffer(fragment_idx as u64, Some(&slot.argument_buffer), 0);
            }

            Some(())
        } else {
            None
        }
    }

    fn set_marker(&mut self, colour: u32, name: &str) {
    }

    fn push_render_constants<T: Sized>(&mut self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        let encoder = self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands");

        // Find the pipeline slot by buffer index
        if let Some(pipeline_ptr) = self.bound_render_pipeline {
            let pipeline = unsafe { &*pipeline_ptr };

            // Find slot with matching buffer index
            for pipeline_slot in pipeline.slot_lookup.values() {
                if pipeline_slot.info.index == slot && pipeline_slot.data_buffer.is_some() {
                    // Copy data to the data buffer
                    if let Some(ref data_buffer) = pipeline_slot.data_buffer {
                        let data_bytes = unsafe {
                            std::slice::from_raw_parts(
                                data.as_ptr() as *const u8,
                                std::mem::size_of_val(data)
                            )
                        };
                        let dest_ptr = data_buffer.contents() as *mut u8;
                        let dest_offset_bytes = dest_offset as usize * 4;
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                data_bytes.as_ptr(),
                                dest_ptr.add(dest_offset_bytes),
                                data_bytes.len()
                            );
                        }

                        // Re-encode the buffer pointer into the argument buffer
                        pipeline_slot.argument_encoder.set_argument_buffer(&pipeline_slot.argument_buffer, 0);
                        pipeline_slot.argument_encoder.set_buffer(0, data_buffer, 0);

                        // Bind the argument buffer to the appropriate stage(s)
                        if let Some(vertex_idx) = pipeline_slot.vertex_buffer_index {
                            encoder.set_vertex_buffer(
                                vertex_idx as u64,
                                Some(&pipeline_slot.argument_buffer),
                                0
                            );
                        }
                        if let Some(fragment_idx) = pipeline_slot.fragment_buffer_index {
                            encoder.set_fragment_buffer(
                                fragment_idx as u64,
                                Some(&pipeline_slot.argument_buffer),
                                0
                            );
                        }
                    }
                    return;
                }
            }
        }
    }

    fn push_compute_constants<T: Sized>(&mut self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
    }

    fn draw_instanced(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .draw_primitives_instanced_base_instance(
                    metal::MTLPrimitiveType::TriangleStrip,
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
                    metal::MTLIndexType::UInt16,
                    &self.bound_index_buffer.as_ref().unwrap(),
                    start_index as u64 * self.bound_index_stride as u64,
                    instance_count as u64,
                    base_vertex as i64,
                    start_instance as u64
                );
        })
    }

    fn dispatch(&mut self, group_count: Size3, _numthreads: Size3) {
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

    fn resolve_texture_subresource(&mut self, texture: &Texture, subresource: u32) -> result::Result<(), super::Error> {
        Ok(())
    }

    fn generate_mip_maps(&mut self, texture: &Texture, device: &Device, heap: &Heap) -> result::Result<(), super::Error> {
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

pub struct Buffer {
    metal_buffer: metal::Buffer,
    element_stride: usize
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
        Ok(())
    }

    fn get_cbv_index(&self) -> Option<usize> {
        None
    }

    fn get_srv_index(&self) -> Option<usize> {
        None
    }

    fn get_uav_index(&self) -> Option<usize> {
        None
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

/// Unified pipeline slot for both descriptors and push constants
/// Supports per-stage buffer indices for Metal argument buffers
pub struct PipelineSlot {
    /// Metal buffer index for vertex stage (None if not visible to vertex)
    pub vertex_buffer_index: Option<u32>,
    /// Metal buffer index for fragment stage (None if not visible to fragment)
    pub fragment_buffer_index: Option<u32>,
    /// Argument encoder for encoding resources into argument buffer
    pub argument_encoder: metal::ArgumentEncoder,
    /// Argument buffer containing encoded resource pointers
    pub argument_buffer: metal::Buffer,
    /// Index within the shared argument buffer (for texture bindings)
    pub binding_index: u32,
    /// Data buffer for push constants (None for regular descriptors)
    pub data_buffer: Option<metal::Buffer>,
    /// Slot info for API compatibility
    pub info: PipelineSlotInfo,
    /// Visibility for this slot
    pub visibility: ShaderVisibility,
}

/// Key for slot lookup: (register, space, descriptor_type)
type SlotKey = (u32, u32, DescriptorType);

pub struct RenderPipeline {
    pipeline_state: metal::RenderPipelineState,
    static_samplers: Vec<MetalSamplerBinding>,
    slots: Vec<u32>,
    /// Unified slot lookup by (register, space, descriptor_type)
    slot_lookup: HashMap<SlotKey, PipelineSlot>,
    /// Sampler argument buffer (at buffer(4) per htwv convention)
    sampler_argument_buffer: Option<metal::Buffer>,
    /// Primitive topology for draw calls
    topology: Topology,
}

impl super::RenderPipeline<Device> for RenderPipeline {}

impl super::Pipeline for RenderPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        self.slot_lookup.get(&(register, space, descriptor_type)).map(|slot| &slot.info)
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
    srv_index: Option<usize>,
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
        None
    }

    fn get_uav_index(&self) -> Option<usize> {
        None
    }

    fn clone_inner(&self) -> Texture {
        Texture {
            metal_texture: self.metal_texture.clone(),
            srv_index: self.srv_index,
            heap_id: self.heap_id
        }
    }

    fn is_resolvable(&self) -> bool {
        false
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
    pixel_format: metal::MTLPixelFormat,
}

impl super::RenderPass<Device> for RenderPass {
    fn get_format_hash(&self) -> u64 {
        0
    }
}

pub struct ComputePipeline {
    slots: Vec<u32>
}

impl super::Pipeline for ComputePipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        None
    }

    fn get_pipeline_slots(&self) -> &Vec<u32> {
        &self.slots
    }

    fn get_pipeline_type() -> PipelineType {
        super::PipelineType::Compute
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
    id: u16
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

}

impl super::QueryHeap<Device> for QueryHeap {
    fn reset(&mut self) {
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
            texture_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);

            // Determine the size required for the heap for the given descriptor
            let size_and_align = mtl_device.heap_texture_size_and_align(&texture_descriptor);
            let texture_size = align_pow2(size_and_align.size, size_and_align.align);

            let heap_size = texture_size * info.num_descriptors.max(1) as u64;

            let heap_descriptor = metal::HeapDescriptor::new();
            heap_descriptor.set_storage_mode(metal::MTLStorageMode::Shared);
            heap_descriptor.set_size(heap_size);

            let heap = mtl_device.new_heap(&heap_descriptor);

        Heap {
            mtl_heap: heap,
            texture_slots: Vec::new(),
            buffer_slots: Vec::new(),
            resource_type: Vec::new(),
            offset: 0,
            id
        }
    }

    /// Build unified slot lookup
    fn build_slot_lookup(
        &self,
        pipeline_bindings: &Option<Vec<DescriptorBinding>>,
        pipeline_push_constants: &Option<Vec<PushConstantInfo>>,
    ) -> HashMap<SlotKey, PipelineSlot> {
        let mut slot_lookup: HashMap<SlotKey, PipelineSlot> = HashMap::new();

        // htwv convention: samplers at 2 on vs
        // samplers at 0 on ps
        // Track binding offsets separately per stage since different numbers of
        // bindings and push constants might be active on each stage
        let vertex_samplers_offset: u32 = 2;
        let fragment_samplers_offset: u32 = 0;
        let mut vertex_binding_offset: u32 = vertex_samplers_offset + 1;
        let mut fragment_binding_offset: u32 = fragment_samplers_offset + 1;

        // Add push constant slots first (they come before regular bindings in htwv)
        if let Some(push_constants) = pipeline_push_constants.as_ref() {
            for push_constant in push_constants {

                // Create argument descriptor for pointer type (push constants use pointers in argument buffers)
                let arg_desc = metal::ArgumentDescriptor::new();
                arg_desc.set_index(0);
                arg_desc.set_data_type(metal::MTLDataType::Pointer);
                arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);

                let argument_encoder = self.metal_device.new_argument_encoder(
                    metal::Array::from_owned_slice(&[arg_desc.to_owned()])
                );
                let argument_buffer = self.metal_device.new_buffer(
                    argument_encoder.encoded_length(),
                    metal::MTLResourceOptions::StorageModeShared
                );

                // Data buffer holds the actual push constant values
                let data_buffer = self.metal_device.new_buffer(
                    push_constant.num_values as u64 * 4,
                    metal::MTLResourceOptions::StorageModeShared
                );

                // Encode the data buffer pointer into the argument buffer
                argument_encoder.set_argument_buffer(&argument_buffer, 0);
                argument_encoder.set_buffer(0, &data_buffer, 0);

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
                    PipelineSlot {
                        vertex_buffer_index: vertex_idx,
                        fragment_buffer_index: fragment_idx,
                        argument_encoder,
                        argument_buffer,
                        binding_index: 0, // Not used for push constants
                        data_buffer: Some(data_buffer),
                        info: PipelineSlotInfo {
                            index: canonical_index,
                            count: Some(push_constant.num_values),
                        },
                        visibility: push_constant.visibility,
                    },
                );
            }
        }

        // Add regular binding slots - ALL share ONE argument buffer
        const MAX_BINDLESS_TEXTURES: u64 = 1024;
        if let Some(bindings) = pipeline_bindings.as_ref() {
            if !bindings.is_empty() {
                // Build argument descriptors - one per binding with unique indices
                let arg_descs: Vec<metal::ArgumentDescriptor> = bindings.iter().enumerate()
                    .map(|(i, binding)| {
                        let arg_desc = metal::ArgumentDescriptor::new();
                        arg_desc.set_index(i as u64);  // Each binding gets unique index
                        let array_len = binding.num_descriptors.map(|n| n as u64).unwrap_or(MAX_BINDLESS_TEXTURES);
                        arg_desc.set_array_length(array_len);
                        arg_desc.set_data_type(metal::MTLDataType::Texture);
                        arg_desc.set_access(metal::MTLArgumentAccess::ReadOnly);
                        arg_desc.to_owned()
                    })
                    .collect();

                // Create SINGLE encoder/buffer for ALL bindings
                let argument_encoder = self.metal_device.new_argument_encoder(
                    metal::Array::from_owned_slice(&arg_descs)
                );
                let argument_buffer = self.metal_device.new_buffer(
                    argument_encoder.encoded_length(),
                    metal::MTLResourceOptions::StorageModeShared
                );

                // Determine if any binding needs vertex or fragment visibility
                let needs_vertex = bindings.iter().any(|b|
                    matches!(b.visibility, ShaderVisibility::Vertex | ShaderVisibility::All));
                let needs_fragment = bindings.iter().any(|b|
                    matches!(b.visibility, ShaderVisibility::Fragment | ShaderVisibility::All));

                // Single buffer index per stage (only increment once, not per binding!)
                let vertex_idx = if needs_vertex {
                    let idx = vertex_binding_offset;
                    vertex_binding_offset += 1;
                    Some(idx)
                } else {
                    None
                };
                let fragment_idx = if needs_fragment {
                    let idx = fragment_binding_offset;
                    fragment_binding_offset += 1;
                    Some(idx)
                } else {
                    None
                };
                let canonical_index = vertex_idx.or(fragment_idx).unwrap_or(0);

                // Each binding shares buffer but has unique binding_index
                for (i, binding) in bindings.iter().enumerate() {
                    // Per-slot visibility based on the binding's visibility
                    let slot_vertex_idx = match binding.visibility {
                        ShaderVisibility::Vertex | ShaderVisibility::All => vertex_idx,
                        _ => None,
                    };
                    let slot_fragment_idx = match binding.visibility {
                        ShaderVisibility::Fragment | ShaderVisibility::All => fragment_idx,
                        _ => None,
                    };

                    slot_lookup.insert(
                        (binding.shader_register, binding.register_space, binding.binding_type),
                        PipelineSlot {
                            vertex_buffer_index: slot_vertex_idx,
                            fragment_buffer_index: slot_fragment_idx,
                            argument_encoder: argument_encoder.clone(),
                            argument_buffer: argument_buffer.clone(),
                            binding_index: i as u32,  // 0, 1, 2, 3 matching shader [[id()]]
                            data_buffer: None,
                            info: PipelineSlotInfo {
                                index: canonical_index,
                                count: binding.num_descriptors,
                            },
                            visibility: binding.visibility,
                        },
                    );
                }
            }
        }

        slot_lookup
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
            assert_eq!(metal::MTLArgumentBuffersTier::Tier2, tier); //TODO: message

            Device {
                command_queue: command_queue,
                shader_heap: Self::create_heap_mtl(&device, &HeapInfo{
                    heap_type: HeapType::Shader,
                    num_descriptors: info.shader_heap_size,
                    debug_name: Some("mtl device: shader heap".to_string())
                }, 1),
                adapter_info: adapter_info,
                metal_device: device
            }
       })
    }

    fn get_feature_flags(&self) -> &DeviceFeatureFlags {
        unimplemented!()
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Heap {
        Self::create_heap_mtl(&self.metal_device, &info, 2)
    }

    fn create_query_heap(&self, info: &QueryHeapInfo) -> QueryHeap {
        QueryHeap {

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
                layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

                let drawable = layer.next_drawable()
                    .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

                let backbuffer_texture = Texture {
                    metal_texture: drawable.texture().to_owned(),
                    srv_index: None,
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
                })
            })
        }
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> CmdBuf {
        objc::rc::autoreleasepool(|| {
            CmdBuf {
                cmd_queue: self.command_queue.clone(),
                cmd: None,
                render_encoder: None,
                compute_encoder: None,
                bound_index_buffer: None,
                bound_index_stride: 0,
                bound_render_pipeline: None,
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
                    let vvs = lib.get_function("vs_main", None).unwrap();
                    pipeline_state_descriptor.set_vertex_function(Some(&vvs));
                }
            };
            if let Some(fs) = info.fs {
                unsafe {
                    let lib = self.metal_device.new_library_with_data(std::slice::from_raw_parts(fs.data, fs.data_size))?;
                    let pps = lib.get_function("ps_main", None).unwrap();
                    pipeline_state_descriptor.set_fragment_function(Some(&pps));
                }
            };

            // vertex attribs
            let vertex_desc = metal::VertexDescriptor::new();
            let mut attrib_index = 0;

            // make spaces for slots to calculate the stride from offsets + size
            let mut slot_strides = Vec::new();
            for element in &info.input_layout {
                if slot_strides.len() < (element.input_slot + 1) as usize {
                    slot_strides.resize((element.input_slot + 1) as usize, 0);
                }
            }

            // make the idividual attributes and track the stride of each slot
            for element in &info.input_layout {
                let attribute = metal::VertexAttributeDescriptor::new();
                attribute.set_format(to_mtl_vertex_format(element.format));
                attribute.set_buffer_index(element.input_slot as NSUInteger);
                attribute.set_offset(element.aligned_byte_offset as NSUInteger);
                vertex_desc.attributes().set_object_at(attrib_index, Some(&attribute));
                attrib_index += 1;

                let stride = element.aligned_byte_offset + block_size_for_format(element.format);
                slot_strides[element.input_slot as usize] = max(slot_strides[element.input_slot as usize], stride);
            }

            // vertex layouts; TODO: work out MTLVertexStepFunction
            let layout_desc = metal::VertexBufferLayoutDescriptor::new();
            layout_desc.set_step_function(metal::MTLVertexStepFunction::PerVertex);
            layout_desc.set_stride(slot_strides[0] as NSUInteger);
            vertex_desc.layouts().set_object_at(0, Some(&layout_desc));

            pipeline_state_descriptor.set_vertex_descriptor(Some(&vertex_desc));

            // TODO: attachments
            let attachment = pipeline_state_descriptor
                .color_attachments()
                .object_at(0)
                .unwrap();

            // Get pixel format from pass, or default to BGRA8Unorm
            let pixel_format = info.pass
                .map(|p| p.pixel_format)
                .unwrap_or(metal::MTLPixelFormat::BGRA8Unorm);
            attachment.set_pixel_format(pixel_format);

            // Apply blend state from pipeline info
            if let Some(b) = info.blend_info.render_target.first() {
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
            }

            // TODO: depth stencil

            // TODO: raster

            // Create static samplers and argument buffer (at buffer(4) per htwv convention)
            let mut pipeline_static_samplers = Vec::new();
            let mut sampler_argument_buffer = None;

            if let Some(static_samplers) = &info.pipeline_layout.static_samplers {
                for sampler in static_samplers {
                    let desc = metal::SamplerDescriptor::new();
                    desc.set_address_mode_r(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_address_mode_s(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_address_mode_t(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
                    desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
                    desc.set_mip_filter(metal::MTLSamplerMipFilter::Linear);
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

            let pipeline_state = self.metal_device.new_render_pipeline_state(&pipeline_state_descriptor)?;

            Ok(RenderPipeline {
                pipeline_state,
                slots: Vec::new(),
                static_samplers: pipeline_static_samplers,
                slot_lookup,
                sampler_argument_buffer,
                topology: info.topology,
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
            let opt = metal::MTLResourceOptions::CPUCacheModeDefaultCache |
                metal::MTLResourceOptions::StorageModeManaged;

            let byte_len = (info.stride * info.num_elements) as NSUInteger;

            let buf = if let Some(data) = data {
                let bytes = data.as_ptr() as *const std::ffi::c_void;
                self.metal_device.new_buffer_with_data(bytes, byte_len, opt)
            }
            else {
                self.metal_device.new_buffer(byte_len, opt)
            };

            Ok(Buffer{
                metal_buffer: buf,
                element_stride: info.stride
            })
        })
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        objc::rc::autoreleasepool(|| {
            let opt = metal::MTLResourceOptions::CPUCacheModeDefaultCache |
                metal::MTLResourceOptions::StorageModeManaged;

            let byte_len = (info.stride * info.num_elements) as NSUInteger;

            let buf = if let Some(data) = data {
                let bytes = data.as_ptr() as *const std::ffi::c_void;
                self.metal_device.new_buffer_with_data(bytes, byte_len, opt)
            }
            else {
                self.metal_device.new_buffer(byte_len, opt)
            };

            Ok(Buffer{
                metal_buffer: buf,
                element_stride: info.stride
            })
        })
    }

    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> result::Result<Self::Buffer, super::Error> {
        objc::rc::autoreleasepool(|| {
            let opt = metal::MTLResourceOptions::CPUCacheModeDefaultCache |
                metal::MTLResourceOptions::StorageModeManaged;

            let byte_len = size as NSUInteger;
            let buf = self.metal_device.new_buffer(byte_len, opt);

            Ok(Buffer{
                metal_buffer: buf,
                element_stride: size
            })
        })
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {
        objc::rc::autoreleasepool(|| {
            let desc = TextureDescriptor::new();

            // TODO:
            // tex_type
            // format
            // initial_state

            // desc
            desc.set_pixel_format(metal::MTLPixelFormat::RGBA8Unorm); // TODO: format

            desc.set_width(info.width as NSUInteger);
            desc.set_height(info.height as NSUInteger);
            desc.set_depth(info.depth as NSUInteger);
            desc.set_array_length(info.array_layers as NSUInteger);
            desc.set_mipmap_level_count(info.mip_levels as NSUInteger);
            desc.set_sample_count(info.samples as NSUInteger);
            desc.set_usage(to_mtl_texture_usage(info.usage));
            desc.set_storage_mode(metal::MTLStorageMode::Shared);
            desc.set_texture_type(metal::MTLTextureType::D2);

            // heap bindless
            let tex = self.shader_heap.mtl_heap.new_texture(&desc)
                .expect("hotline_rs::gfx::mtl failed to allocate texture in heap!");

            // data
            if let Some(data) = data {
                tex.replace_region(
                    metal::MTLRegion {
                        origin: metal::MTLOrigin { x: 0, y: 0, z: 0 },
                        size: metal::MTLSize {
                            width: info.width,
                            height: info.height,
                            depth: info.depth as u64,
                        },
                    },
                    0,
                    data.as_ptr() as _,
                    info.width * 4, // TODO size from format
                );
            }

            // srv
            let srv_index = self.shader_heap.allocate();
            self.shader_heap.texture_slots[srv_index] = Some(tex.to_owned());

            Ok(Texture{
                metal_texture: tex,
                srv_index: Some(srv_index),
                heap_id: Some(self.shader_heap.id)
            })
        })
    }

    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> result::Result<Self::Texture, super::Error> {
        objc::rc::autoreleasepool(|| {
            let desc = TextureDescriptor::new();
            let tex = self.metal_device.new_texture(&desc);
            Ok(Texture{
                metal_texture: tex,
                srv_index: None,
                heap_id: Some(self.shader_heap.id)
            })
        })
    }

    fn create_render_pass(
        &self,
        info: &super::RenderPassInfo<Device>,
    ) -> result::Result<RenderPass, super::Error> {
        objc::rc::autoreleasepool(|| {
            // new desc
            let descriptor = metal::RenderPassDescriptor::new();

            // colour attachments
            for rt in &info.render_targets {
                let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
                color_attachment.set_texture(Some(&rt.metal_texture));

                if let Some(cc) = info.rt_clear {
                    color_attachment.set_load_action(metal::MTLLoadAction::Clear);
                    color_attachment.set_clear_color(metal::MTLClearColor::new(cc.r as f64, cc.g as f64, cc.b as f64, 1.0));
                    color_attachment.set_store_action(metal::MTLStoreAction::Store);
                }
                else {
                    color_attachment.set_load_action(metal::MTLLoadAction::Load);
                    color_attachment.set_store_action(metal::MTLStoreAction::Store);
                }
            }

            // Get pixel format from first render target
            let pixel_format = info.render_targets.first()
                .map(|rt| rt.metal_texture.pixel_format())
                .unwrap_or(metal::MTLPixelFormat::BGRA8Unorm);

            Ok(RenderPass{
                desc: descriptor.to_owned(),
                pixel_format,
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
        Ok(ComputePipeline{
            slots: Vec::new()
        })
    }

    fn create_indirect_render_command<T: Sized>(&mut self,
        arguments: Vec<super::IndirectArgument>,
        pipeline: Option<&RenderPipeline>) -> result::Result<CommandSignature, super::Error> {
        Ok(CommandSignature{

        })
    }

    fn execute(&mut self, cmd: &CmdBuf) {

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

    fn read_timestamps(&self, swap_chain: &SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Vec<f64> {
        vec![]
    }

    fn read_pipeline_statistics(&self, swap_chain: &SwapChain, buffer: &Self::Buffer, frame_written_fence: u64) -> Option<super::PipelineStatistics> {
        None
    }

    fn get_timestamp_size_bytes() -> usize {
        0
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
