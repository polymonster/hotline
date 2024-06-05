#![cfg(target_os = "macos")]

use crate::os_platform;
use crate::os::Window;

use bevy_ecs::system::lifetimeless::Read;
use metal::TextureDescriptor;

use super::*;
use super::Device as SuperDevice;
use super::ReadBackRequest as SuperReadBackRequest;
use super::Heap as SuperHeap;
use super::Pipeline as SuperPipleline;

use std::result;

use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;

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

impl SwapChain {
    fn create_backbuffer_passes() {

    }
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
        let draw_size = window.get_size();
        self.layer.set_drawable_size(CGSize::new(draw_size.x as f64, draw_size.y as f64));

        let drawable = self.layer.next_drawable()
            .expect("hotline_rs::gfx::mtl failed to get next drawable to create swap chain!");

        self.drawable = drawable.to_owned();

        self.backbuffer_texture = Texture {
            metal_texture: drawable.texture().new_texture_view(drawable.texture().pixel_format())
        };

        self.backbuffer_pass = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, self.backbuffer_clear);
        self.backbuffer_pass_no_clear = device.create_render_pass_for_swap_chain(&self.backbuffer_texture, None);
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

}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
    }

    fn close(&mut self) -> result::Result<(), super::Error> {
        Ok(())
    }

    fn get_backbuffer_index(&self) -> u32 {
        0
    }

    fn begin_render_pass(&self, render_pass: &RenderPass) {
    }

    fn end_render_pass(&self) {
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
    }

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
    }

    fn set_render_pipeline(&self, pipeline: &RenderPipeline) {
    }

    fn set_compute_pipeline(&self, pipeline: &ComputePipeline) {
    }

    fn set_heap<T: SuperPipleline>(&self, pipeline: &T, heap: &Heap) {
    }

    fn set_binding<T: SuperPipleline>(&self, _: &T, heap: &Heap, slot: u32, offset: usize) {
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
    }

    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
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

}

impl super::Shader<Device> for Shader {}

pub struct RenderPipeline {

}

impl super::RenderPipeline<Device> for RenderPipeline {}

#[derive(Clone)]
pub struct Texture {
    metal_texture: metal::Texture
}

impl super::Texture<Device> for Texture {
    fn get_srv_index(&self) -> Option<usize> {
        None
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
            metal_texture: self.metal_texture.clone()
        }
    }

    fn is_resolvable(&self) -> bool {
        false
    }

    fn get_shader_heap_id(&self) -> Option<u16> {
        None
    }
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

}

impl super::RenderPass<Device> for RenderPass {
    fn get_format_hash(&self) -> u64 {
        0
    }
}

pub struct ComputePipeline {

}

#[derive(Clone)]
pub struct Heap {

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
        self.create_render_pass(&RenderPassInfo {
            render_targets: vec![texture],
            rt_clear: clear_col,
            depth_stencil: None,
            ds_clear: None,
            resolve: false,
            discard: false,
            array_slice: 0
        }).unwrap()
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
        let device = metal::Device::system_default()
            .expect("hotline_rs::gfx::mtl: failed to create metal device");
        let command_queue = device.new_command_queue();
        Device {
            metal_device: device,
            command_queue: command_queue,
            shader_heap: Heap {
            },
            adapter_info: AdapterInfo {
                name: "".to_string(),
                description: "".to_string(),
                dedicated_video_memory: 0,
                dedicated_system_memory: 0,
                shared_system_memory: 0,
                available: vec![]
            }
        }
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Heap {
        Heap {

        }
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
                metal_texture: drawable.texture().new_texture_view(drawable.texture().pixel_format())
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
        }
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> CmdBuf {
        CmdBuf {

        }
    }

    fn create_render_pipeline(
        &self,
        info: &super::RenderPipelineInfo<Device>,
    ) -> result::Result<RenderPipeline, super::Error> {
        Ok(RenderPipeline {

        })
    }

    fn create_shader<T: Sized>(
        &self,
        info: &super::ShaderInfo,
        src: &[T],
    ) -> std::result::Result<Shader, super::Error> {
        Ok(Shader{

        })
    }

    fn create_buffer_with_heap<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
        heap: &mut Heap
    ) -> result::Result<Buffer, super::Error> {
        Ok(Buffer{

        })
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &super::BufferInfo,
        data: Option<&[T]>,
    ) -> result::Result<Buffer, super::Error> {
        Ok(Buffer{

        })
    }

    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> result::Result<Self::Buffer, super::Error> {
        Ok(Buffer{

        })
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &super::TextureInfo,
        data: Option<&[T]>,
    ) -> result::Result<Texture, super::Error> {

        let desc = TextureDescriptor::new();
        let tex = self.metal_device.new_texture(&desc);

        Ok(Texture{
            metal_texture: tex
        })
    }

    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> result::Result<Self::Texture, super::Error> {
        let desc = TextureDescriptor::new();
        let tex = self.metal_device.new_texture(&desc);

        Ok(Texture{
            metal_texture: tex
        })
    }

    fn create_render_pass(
        &self,
        info: &super::RenderPassInfo<Device>,
    ) -> result::Result<RenderPass, super::Error> {
        /*
        fn prepare_render_pass_descriptor(descriptor: &RenderPassDescriptorRef, texture: &TextureRef) {
        let color_attachment = descriptor.color_attachments().object_at(0).unwrap();

        color_attachment.set_texture(Some(texture));
        color_attachment.set_load_action(MTLLoadAction::Clear);
        color_attachment.set_clear_color(MTLClearColor::new(0.2, 0.2, 0.25, 1.0));
        color_attachment.set_store_action(MTLStoreAction::Store);
        */

        let descriptor = metal::RenderPassDescriptor::new();
        for rt in &info.render_targets {
            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            // color_attachment.set_texture(Some(texture));

            color_attachment.set_load_action(metal::MTLLoadAction::Clear);
            color_attachment.set_clear_color(metal::MTLClearColor::new(0.2, 0.2, 0.25, 1.0));
            color_attachment.set_store_action(metal::MTLStoreAction::Store);
        }

        Ok(RenderPass{

        })
    }

    fn create_compute_pipeline(
        &self,
        info: &super::ComputePipelineInfo<Self>,
    ) -> result::Result<ComputePipeline, super::Error> {
        Ok(ComputePipeline{

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