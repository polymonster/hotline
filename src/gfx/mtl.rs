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

use std::collections::HashMap;
use std::result;

use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;

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
        super::Format::RGBA8n => MTLVertexFormat::Char4Normalized,
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
        0
    }

    fn get_frame_fence_value(&self) -> u64 {
        0
    }

    fn update<A: os::App>(&mut self, device: &mut Device, window: &A::Window, cmd: &mut CmdBuf) {
        objc::rc::autoreleasepool(|| {
            let draw_size = window.get_size();
            self.layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

            let drawable = self.layer.next_drawable()
                .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

            self.drawable = drawable.to_owned();

            self.backbuffer_texture = Texture {
                metal_texture: drawable.texture().to_owned(),
                srv_index: None
            };

            self.backbuffer_pass = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, self.backbuffer_clear);
            self.backbuffer_pass_no_clear = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, None);
        });
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

    fn swap(&mut self, device: &Device) {
        objc::rc::autoreleasepool(|| {
            let cmd = device.command_queue.new_command_buffer();
            cmd.present_drawable(&self.drawable);
            cmd.commit();
        });
    }
}

#[derive(Clone)]
pub struct CmdBuf {
    cmd_queue: metal::CommandQueue,
    cmd: Option<metal::CommandBuffer>,
    render_encoder: Option<metal::RenderCommandEncoder>,
    compute_encoder: Option<metal::ComputeCommandEncoder>,
    bound_index_buffer: Option<metal::Buffer>,

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

    fn set_viewport(&self, viewport: &super::Viewport) {
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

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
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

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .set_vertex_buffer(slot as NSUInteger, Some(&buffer.metal_buffer), 0);
        });
    }

    fn set_index_buffer(&mut self, buffer: &Buffer) {
        self.bound_index_buffer = Some(buffer.metal_buffer.clone());
    }

    fn set_render_pipeline(&self, pipeline: &RenderPipeline) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .set_render_pipeline_state(&pipeline.pipeline_state);

            // TODO: temp samplers
            for sampler in &pipeline.static_samplers {
                self.render_encoder.as_ref().unwrap().set_fragment_sampler_state(
                    sampler.slot as u64, Some(&sampler.sampler))
            }
        });
    }

    fn set_compute_pipeline(&self, pipeline: &ComputePipeline) {

    }

    fn set_heap<T: SuperPipleline>(&self, pipeline: &T, heap: &Heap) {

    }

    fn set_heap_render(&self, pipeline: &RenderPipeline, heap: &Heap) {
        // TODO: new
        self.render_encoder
            .as_ref()
            .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
            .use_heap_at(&heap.mtl_heap, metal::MTLRenderStages::Fragment);

        // TODO: loop
        pipeline.descriptor_slots.iter().enumerate().for_each(|(slot_index, slot)| {
            if let Some(slot) = slot {
                // TODO: how to know which reg to bind on
                slot.argument_encoder.set_argument_buffer(&slot.argument_buffer, 0);

                // TODO: need to know data types (Texture, Buffer)
                // assign textures to slots
                if slot_index == 0 {
                    heap.tex_slots.iter().enumerate().for_each(|(index, texture)| {
                        slot.argument_encoder.set_texture(index as u64, texture);
                    });
                }

                if slot_index == 0 {
                    self.render_encoder.as_ref().unwrap().set_fragment_buffer(slot_index as u64, Some(&slot.argument_buffer), 0);
                }
            }
        });
    }

    fn set_binding<T: SuperPipleline>(&self, _: &T, heap: &Heap, slot: u32, offset: usize) {
        // TODO: how to know the type?
    }

    fn set_texture(&mut self, texture: &Texture, slot: u32) {
        self.render_encoder.as_ref().unwrap().set_fragment_texture(slot as u64, Some(&texture.metal_texture));
    }

    fn set_marker(&self, colour: u32, name: &str) {
    }

    fn push_render_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
    }

    fn push_compute_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
    }

    fn draw_instanced(
        &self,
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
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        objc::rc::autoreleasepool(|| {
            self.render_encoder
                .as_ref()
                .expect("hotline_rs::gfx::metal expected a call to begin render pass before using render commands")
                .draw_indexed_primitives_instanced_base_instance(
                    metal::MTLPrimitiveType::TriangleStrip,
                    index_count as u64,
                    metal::MTLIndexType::UInt16,
                    &self.bound_index_buffer.as_ref().unwrap(),
                    start_index as u64,
                    instance_count as u64,
                    base_vertex as i64,
                    start_instance as u64
                );
        })
    }

    fn dispatch(&self, group_count: Size3, _numthreads: Size3) {
    }

    fn execute_indirect(
        &self,
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

    fn resolve_texture_subresource(&self, texture: &Texture, subresource: u32) -> result::Result<(), super::Error> {
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
}

pub struct Buffer {
    metal_buffer: metal::Buffer
}

impl super::Buffer<Device> for Buffer {
    fn update<T: Sized>(&mut self, offset: usize, data: &[T]) -> result::Result<(), super::Error> {
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
    function: metal::Function,
    lib: metal::Library,
    data: Vec<u8>
}

impl super::Shader<Device> for Shader {}

struct MetalSamplerBinding {
    slot: u32,
    sampler: metal::SamplerState
}

#[derive(Clone)]
pub struct DescriptorMember {
    offset: u32,
    num: u32
}
type DescriptorMemberArray = Vec<Option<DescriptorMember>>;

#[derive(Clone)]
pub struct DescriptorSlot {
    argument_buffer: metal::Buffer,
    argument_encoder: metal::ArgumentEncoder,
    members: Vec<Option<DescriptorMember>>
}
type DescriptorSlotArray = Vec<Option<DescriptorSlot>>;

pub struct RenderPipeline {
    pipeline_state: metal::RenderPipelineState,
    static_samplers: Vec<MetalSamplerBinding>,
    slots: Vec<u32>,
    descriptor_slots: DescriptorSlotArray
}

impl super::RenderPipeline<Device> for RenderPipeline {}


impl super::Pipeline for RenderPipeline {
    fn get_pipeline_slot(&self, register: u32, space: u32, descriptor_type: DescriptorType) -> Option<&super::PipelineSlotInfo> {
        None
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
    srv_index: Option<usize>
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
            srv_index: self.srv_index
        }
    }

    fn is_resolvable(&self) -> bool {
        false
    }

    fn get_shader_heap_id(&self) -> Option<u16> {
        None
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
    desc: metal::RenderPassDescriptor
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
pub struct Heap {
    mtl_heap: metal::Heap,
    tex_slots: Vec<metal::Texture>
}

impl super::Heap<Device> for Heap {
    fn deallocate(&mut self, index: usize) {
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &SwapChain) {
    }

    fn get_heap_id(&self) -> u16 {
        0
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
    fn create_heap_mtl(mtl_device: &metal::Device, info: &HeapInfo) -> Heap {
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
            println!("heap: {:?}", heap);

        Heap {
            mtl_heap: heap,
            tex_slots: Vec::new()
        }
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

    fn create(info: &super::DeviceInfo) -> Device {
        //objc::rc::autoreleasepool(|| {
            let device = metal::Device::system_default()
                .expect("hotline_rs::gfx::mtl: failed to create metal device");
            let command_queue = device.new_command_queue();

            // adapter info
            let adapter_info = AdapterInfo {
                name: device.name().to_string(),
                description: "".to_string(),
                dedicated_video_memory: device.recommended_max_working_set_size() as usize,
                dedicated_system_memory: 0,
                shared_system_memory: 0,
                available: vec![]
            };

            // feature info
            let tier = device.argument_buffers_support();
            println!("Argument buffer support: {:?}", tier);
            assert_eq!(metal::MTLArgumentBuffersTier::Tier2, tier);

            Device {
                command_queue: command_queue,
                shader_heap: Self::create_heap_mtl(&device, &HeapInfo{
                    heap_type: HeapType::Shader,
                    num_descriptors: info.shader_heap_size
                }),
                adapter_info: adapter_info,
                metal_device: device
            }
       //})
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Heap {
        Self::create_heap_mtl(&self.metal_device, &info)
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
                    srv_index: None
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
                bound_index_buffer: None
            }
        })
    }

    fn create_render_pipeline(
        &self,
        info: &super::RenderPipelineInfo<Device>,
    ) -> result::Result<RenderPipeline, super::Error> {
        // objc::rc::autoreleasepool(|| {
            let pipeline_state_descriptor = metal::RenderPipelineDescriptor::new();

            //println!("{:?}", info.pipeline_layout);
            let vs_data = info.vs.unwrap().data.to_vec();
            let lib = self.metal_device.new_library_with_data(vs_data.as_slice())?;
            let vvs = lib.get_function("vs_main", None).unwrap();

            pipeline_state_descriptor.set_vertex_function(Some(&vvs));

            let ps_data = info.fs.unwrap().data.to_vec();
            let lib = self.metal_device.new_library_with_data(ps_data.as_slice())?;
            let pps = lib.get_function("ps_main", None).unwrap();

            pipeline_state_descriptor.set_fragment_function(Some(&pps));

            /*
            if let Some(vs) = info.vs {
                let lib = self.metal_device.new_library_with_data(vs.data.as_slice())?;
                let vvs = lib.get_function("vs_main", None).unwrap();

                pipeline_state_descriptor.set_vertex_function(Some(&vvs));

                //println!("vs {:?}", vs.function);
                //pipeline_state_descriptor.set_vertex_function(Some(&vs.function));
            };
            if let Some(fs) = info.fs {
                let lib = self.metal_device.new_library_with_data(fs.data.as_slice())?;
                let pps = lib.get_function("ps_main", None).unwrap();

                pipeline_state_descriptor.set_fragment_function(Some(&pps));

                //println!("fs {:?}", fs.function);
                //pipeline_state_descriptor.set_fragment_function(Some(&fs.function));
            };
            */

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
            attachment.set_pixel_format(metal::MTLPixelFormat::BGRA8Unorm);
            attachment.set_blending_enabled(false);
            attachment.set_rgb_blend_operation(metal::MTLBlendOperation::Add);
            attachment.set_alpha_blend_operation(metal::MTLBlendOperation::Add);
            attachment.set_source_rgb_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            attachment.set_source_alpha_blend_factor(metal::MTLBlendFactor::SourceAlpha);
            attachment.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
            attachment.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);

            // TODO: depth stencil

            // TODO: raster

            // TODO: samplers?
            let mut pipeline_static_samplers = Vec::new();
            if let Some(static_samplers) = &info.pipeline_layout.static_samplers {
                for sampler in static_samplers {
                    println!("sampler: {}", sampler.shader_register);

                    let desc = metal::SamplerDescriptor::new();
                    desc.set_address_mode_r(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_address_mode_s(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_address_mode_t(metal::MTLSamplerAddressMode::Repeat);
                    desc.set_min_filter(metal::MTLSamplerMinMagFilter::Linear);
                    desc.set_mag_filter(metal::MTLSamplerMinMagFilter::Linear);
                    desc.set_mip_filter(metal::MTLSamplerMipFilter::Linear);
                    desc.set_support_argument_buffers(true);

                    // TODO:
                    pipeline_static_samplers.push(MetalSamplerBinding {
                        slot: sampler.shader_register,
                        sampler: self.metal_device.new_sampler(&desc)
                    })
                }
            }

            // argument buffer to descriptor slot style
            let mut descriptor_slots : DescriptorSlotArray = Vec::new();

            // register spaces, and shader registers may not be ordered and may not be sequential or have gaps
            if let Some(bindings) = info.pipeline_layout.bindings.as_ref() {
                // make space for enough shader register spaces
                let mut space_count = 0;
                for binding in bindings {
                    space_count = binding.register_space.max(space_count);
                }
                descriptor_slots.resize((space_count + 1) as usize, None);

                // iterate over descriptor slots and find members
                descriptor_slots.iter_mut().enumerate().for_each(|(space, descriptor_slot)| {
                    let mut members : DescriptorMemberArray = Vec::new();
                    for binding in bindings {
                        if binding.register_space == space as u32 {
                            if members.len() < (binding.shader_register + 1) as usize {
                                members.resize((binding.shader_register + 1) as usize, None);
                            }

                            // get num
                            let num = if let Some(num) = binding.num_descriptors {
                                num
                            }
                            else {
                                1
                            };

                            // assign member info
                            members[binding.shader_register as usize] = Some(
                                DescriptorMember {
                                    offset: 0,
                                    num: num
                                }
                            );
                        }
                    }

                    // now work out the offsets of the members within the space
                    let mut offset = 0;
                    for member in &mut members {
                        if let Some(member) = member {
                            member.offset = offset;
                            offset += member.num;
                        }
                    }

                    // finally if we have members and not an empty space
                    // create an argument buffer
                    if members.len() > 0 {
                        let mut member_descriptors = Vec::new();

                        let mut total_num = 0;
                        for member in &members {
                            if let Some(member) = member {
                                let descriptor = metal::ArgumentDescriptor::new();
                                descriptor.set_index(member.offset as u64);
                                descriptor.set_array_length(member.num as u64);

                                // TODO: types / access
                                descriptor.set_data_type(metal::MTLDataType::Texture);
                                descriptor.set_access(metal::MTLArgumentAccess::ReadOnly);

                                // push metal argument descriptor
                                member_descriptors.push(descriptor.to_owned());

                                total_num += member.num;
                            }
                        }

                        // create encoder and argument buffer
                        let argument_encoder = self.metal_device.new_argument_encoder(metal::Array::from_owned_slice(member_descriptors.as_slice()));
                        let argument_buffer_size = argument_encoder.encoded_length() * total_num as u64;
                        let argument_buffer = self.metal_device.new_buffer(argument_buffer_size, metal::MTLResourceOptions::empty());

                        *descriptor_slot = Some(
                            DescriptorSlot {
                                argument_encoder,
                                argument_buffer,
                                members
                            }
                        )
                    }
                });
            }

            /*
            // Argument Buffer
            let descriptor = metal::ArgumentDescriptor::new();
            descriptor.set_index(0);
            descriptor.set_array_length(11);
            descriptor.set_data_type(metal::MTLDataType::Texture);
            descriptor.set_access(metal::MTLArgumentAccess::ReadOnly);
            println!("Argument descriptor: {:?}", descriptor);
            */

            //let encoder = self.metal_device.new_argument_encoder(metal::Array::from_slice(&[descriptor]));
            //println!("encoder: {:?}", encoder);

            // TODO: heap size
            //let argument_buffer_size = encoder.encoded_length();
            //let argument_buffer = self.metal_device.new_buffer(argument_buffer_size, metal::MTLResourceOptions::empty());
            //println!("buffer: {:?}", argument_buffer);

            let pipeline_state = self.metal_device.new_render_pipeline_state(&pipeline_state_descriptor)?;

            Ok(RenderPipeline {
                pipeline_state,
                slots: Vec::new(),
                static_samplers: pipeline_static_samplers,
                descriptor_slots
            })
        //})
    }

    fn create_shader<T: Sized>(
        &self,
        info: &super::ShaderInfo,
        src: &[T],
    ) -> std::result::Result<Shader, super::Error> {
        //objc::rc::autoreleasepool(|| {
            let mut data_copy = Vec::<u8>::new();
            let data = slice_as_u8_slice(src);

            let lib = self.metal_device.new_library_with_data(data)?;

            let names = lib.function_names();
            if names.len() == 1 {
                Ok(Shader{
                    function: lib.get_function(names[0].as_str(), None)?.to_owned(),
                    lib: lib.to_owned(),
                    data: data.to_vec()
                })
            }
            else {
                Err(super::Error {
                    msg: format!(
                        "hotline_rs::gfx::mtl expected a shader with single entry point but shader has {} functions", names.len()
                    ),
                })
            }
        //})
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
                metal_buffer: buf
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
                metal_buffer: buf
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
                metal_buffer: buf
            })
        })
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {
        // objc::rc::autoreleasepool(|| {
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

            // bindful
            // let tex = self.metal_device.new_texture(&desc);

            // heap bindless
            println!("heap: {:?}", self.shader_heap.mtl_heap);
            let tex = self.shader_heap.mtl_heap.new_texture(&desc)
                .expect("hotline_rs::gfx::mtl failed to allocate texture in heap!");

            self.shader_heap.tex_slots.push(tex.to_owned());

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

            Ok(Texture{
                metal_texture: tex,
                srv_index: Some(self.shader_heap.tex_slots.len())
            })
        // })
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
                srv_index: None
            })
        })
    }

    fn create_render_pass(
        &self,
        info: &super::RenderPassInfo<Device>,
    ) -> result::Result<RenderPass, super::Error> {
        // objc::rc::autoreleasepool(|| {
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

            Ok(RenderPass{
                desc: descriptor.to_owned()
            })
        //})
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

    fn execute(&self, cmd: &CmdBuf) {

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