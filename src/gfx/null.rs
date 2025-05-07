#![allow(warnings)]

#[derive(Clone)]
pub struct Device;

use super::Error;
use super::DeviceInfo;
use super::AdapterInfo;
use super::DeviceFeatureFlags;
use super::SwapChainInfo;
use super::TextureInfo;
use super::TextureHeapInfo;
use super::IndirectArgumentType;
use super::HeapInfo;
use super::QueryHeapInfo;
use super::PipelineStatistics;
use super::ReadBackData;
use super::IndirectArgument;
use super::RaytracingBLASInfo;
use super::RaytracingTLASInfo;
use super::RaytracingPipelineInfo;
use super::ComputePipelineInfo;
use super::RaytracingShaderBindingTableInfo;
use super::RenderPassInfo;
use super::RenderPipelineInfo;
use super::BufferInfo;
use super::ShaderInfo;
use super::QueryType;
use super::TransitionBarrier;
use super::Subresource;
use super::Viewport;
use super::ScissorRect;
use super::Pipeline;
use super::Size3;
use super::Region;
use super::MapInfo;
use super::UnmapInfo;
use super::VertexBufferView;
use super::IndexBufferView;
use super::UavResource;
use super::AccelerationStructureRebuildMode;
use super::Resource;
use super::RaytracingInstanceInfo;
use super::ResourceViewInfo;

use crate::os::Window;
use crate::os::App;

#[derive(Clone)]
pub struct SwapChain;

#[derive(Clone)]
pub struct CmdBuf;

pub struct Shader;
pub struct RenderPipeline;
pub struct Texture;
pub struct Buffer;
pub struct ReadBackRequest;
pub struct RenderPass;
pub struct Heap;
pub struct QueryHeap;
pub struct ComputePipeline;
pub struct RaytracingPipeline;
pub struct CommandSignature;
pub struct RaytracingShaderBindingTable;
pub struct RaytracingBLAS;
pub struct RaytracingTLAS;

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
        unimplemented!()
    }

    fn update<A: App>(&mut self, device: &mut Device, window: &A::Window, cmd: &mut CmdBuf) -> bool {
        unimplemented!()
    }

    fn wait_for_last_frame(&self) {
        unimplemented!()
    }

    fn get_frame_fence_value(&self) -> u64 {
        unimplemented!()
    }

    fn get_num_buffers(&self) -> u32 {
        unimplemented!()
    }

    fn get_backbuffer_index(&self) -> u32 {
        unimplemented!()
    }

    fn get_backbuffer_texture(&self) -> &Texture {
        unimplemented!()
    }

    fn get_backbuffer_pass(&self) -> &RenderPass {
        unimplemented!()
    }

    fn get_backbuffer_pass_mut(&mut self) -> &mut RenderPass {
        unimplemented!()
    }

    fn get_backbuffer_pass_no_clear(&self) -> &RenderPass {
        unimplemented!()
    }

    fn get_backbuffer_pass_no_clear_mut(&mut self) -> &mut RenderPass {
        unimplemented!()
    }

    fn swap(&mut self, device: &Device) {
        unimplemented!()
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        unimplemented!()
    }

    fn close(&mut self) -> Result<(), Error> {
        unimplemented!()
    }

    fn get_backbuffer_index(&self) -> u32 {
        unimplemented!()
    }

    fn begin_render_pass(&self, render_pass: &RenderPass) {
        unimplemented!()
    }

    fn end_render_pass(&self) {
        unimplemented!()
    }

    fn begin_event(&mut self, colour: u32, name: &str) {
        unimplemented!()
    }

    fn end_event(&mut self) {
        unimplemented!()
    }

    fn set_marker(&self, colour: u32, name: &str) {
        unimplemented!()
    }

    fn timestamp_query(&mut self, heap: &mut QueryHeap, resolve_buffer: &mut Buffer) {
        unimplemented!()
    }

    fn begin_query(&mut self, heap: &mut QueryHeap, query_type: QueryType) -> usize {
        unimplemented!()
    }

    fn end_query(&mut self, heap: &mut QueryHeap, query_type: QueryType, index: usize, resolve_buffer: &mut Buffer) {
        unimplemented!()
    }

    fn transition_barrier(&mut self, barrier: &TransitionBarrier<Device>) {
        unimplemented!()
    }

    fn transition_barrier_subresource(&mut self, barrier: &TransitionBarrier<Device>, subresource: Subresource) {
        unimplemented!()
    }

    fn uav_barrier(&mut self, resource: UavResource<Device>) {
        unimplemented!()
    }

    fn set_viewport(&self, viewport: &Viewport) {
        unimplemented!()
    }

    fn set_scissor_rect(&self, scissor_rect: &ScissorRect) {
        unimplemented!()
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
        unimplemented!()
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        unimplemented!()
    }

    fn set_render_pipeline(&self, pipeline: &RenderPipeline) {
        unimplemented!()
    }

    fn set_compute_pipeline(&self, pipeline: &ComputePipeline) {
        unimplemented!()
    }

    fn set_raytracing_pipeline(&self, pipeline: &RaytracingPipeline) {
        unimplemented!()
    }

    fn set_heap<T: Pipeline>(&self, pipeline: &T, heap: &Heap) {
        unimplemented!()
    }

    fn set_binding<T: Pipeline>(&self, pipeline: &T, heap: &Heap, slot: u32, offset: usize) {
        unimplemented!()
    }

    fn push_render_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        unimplemented!()
    }

    fn push_compute_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        unimplemented!()
    }

    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        unimplemented!()
    }

    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        unimplemented!()
    }

    fn dispatch(&self, group_count: Size3, numthreads: Size3) {
        unimplemented!()
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
        unimplemented!()
    }
    
    fn dispatch_rays(&self, sbt: &RaytracingShaderBindingTable, numthreads: Size3) {
        unimplemented!()
    }

    fn update_raytracing_tlas(&mut self, tlas: &RaytracingTLAS, instance_buffer: &Buffer, instance_count: usize, mode: AccelerationStructureRebuildMode) {
        unimplemented!()
    }

    fn resolve_texture_subresource(&self, texture: &Texture, subresource: u32) -> Result<(), Error> {
        unimplemented!()
    }

    fn generate_mip_maps(&mut self, texture: &Texture, device: &Device, heap: &Heap) -> Result<(), Error> {
        unimplemented!()
    }

    fn read_back_backbuffer(&mut self, swap_chain: &SwapChain) -> Result<ReadBackRequest, Error> {
        unimplemented!()
    }

    fn copy_buffer_region(
        &mut self, 
        dst_buffer: &Buffer, 
        dst_offset: usize, 
        src_buffer: &Buffer, 
        src_offset: usize,
        num_bytes: usize
    ) {
        unimplemented!()
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
        unimplemented!()
    }
}

impl super::Device for Device {
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Shader = Shader;
    type RenderPipeline = RenderPipeline;
    type Texture = Texture;
    type Buffer = Buffer;
    type ReadBackRequest = ReadBackRequest;
    type RenderPass = RenderPass;
    type Heap = Heap;
    type QueryHeap = QueryHeap;
    type ComputePipeline = ComputePipeline;
    type RaytracingPipeline = RaytracingPipeline;
    type CommandSignature = CommandSignature;
    type RaytracingShaderBindingTable = RaytracingShaderBindingTable;
    type RaytracingBLAS = RaytracingBLAS;
    type RaytracingTLAS = RaytracingTLAS;

    fn create(info: &DeviceInfo) -> Self {
        unimplemented!()
    }

    fn create_heap(&mut self, info: &HeapInfo) -> Self::Heap {
        unimplemented!()
    }

    fn create_query_heap(&self, info: &QueryHeapInfo) -> Self::QueryHeap {
        unimplemented!()
    }

    fn create_swap_chain<A: App>(
        &mut self,
        info: &SwapChainInfo,
        window: &A::Window,
    ) -> Result<Self::SwapChain, Error> {
        unimplemented!()
    }

    fn create_cmd_buf(&self, num_buffers: u32) -> Self::CmdBuf {
        unimplemented!()
    }

    fn create_shader<T: Sized>(&self, info: &ShaderInfo, src: &[T]) -> Result<Self::Shader, Error> {
        unimplemented!()
    }

    fn create_buffer<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Buffer, Error> {
        unimplemented!()
    }

    fn create_buffer_with_heap<T: Sized>(
        &mut self,
        info: &BufferInfo,
        data: Option<&[T]>,
        heap: &mut Self::Heap
    ) -> Result<Self::Buffer, Error> {
        unimplemented!()
    }

    fn create_read_back_buffer(
        &mut self,
        size: usize,
    ) -> Result<Self::Buffer, Error> {
        unimplemented!()
    }

    fn create_texture<T: Sized>(
        &mut self,
        info: &TextureInfo,
        data: Option<&[T]>,
    ) -> Result<Self::Texture, Error> {
        unimplemented!()
    }

    fn create_texture_with_heaps<T: Sized>(
        &mut self,
        info: &TextureInfo,
        heaps: TextureHeapInfo<Self>,
        data: Option<&[T]>,
    ) -> Result<Self::Texture, Error> {
        unimplemented!()
    }

    fn create_render_pipeline(
        &self,
        info: &RenderPipelineInfo<Self>,
    ) -> Result<Self::RenderPipeline, Error> {
        unimplemented!()
    }

    fn create_render_pass(&self, info: &RenderPassInfo<Self>) -> Result<Self::RenderPass, Error> {
        unimplemented!()
    }

    fn create_compute_pipeline(
        &self,
        info: &ComputePipelineInfo<Self>,
    ) -> Result<Self::ComputePipeline, Error> {
        unimplemented!()
    }

    fn create_raytracing_pipeline(
        &self,
        info: &RaytracingPipelineInfo<Self>,
    ) -> Result<Self::RaytracingPipeline, Error> {
        unimplemented!()
    }

    fn create_raytracing_shader_binding_table(
        &self,
        info: &RaytracingShaderBindingTableInfo<Self>
    ) -> Result<Self::RaytracingShaderBindingTable, Error> {
        unimplemented!()
    }

    fn create_raytracing_blas(
        &mut self,
        info: &RaytracingBLASInfo<Self>
    ) -> Result<Self::RaytracingBLAS, Error> {
        unimplemented!()
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

    fn create_indirect_render_command<T: Sized>(
        &mut self, 
        arguments: Vec<IndirectArgument>,
        pipeline: Option<&Self::RenderPipeline>
    ) -> Result<Self::CommandSignature, super::Error> {
        unimplemented!()
    }

    fn execute(&self, cmd: &Self::CmdBuf) {
        unimplemented!()
    }

    fn get_shader_heap(&self) -> &Self::Heap {
        unimplemented!()
    }

    fn get_shader_heap_mut(&mut self) -> &mut Self::Heap {
        unimplemented!()
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &Self::SwapChain) {
        unimplemented!()
    }

    fn get_adapter_info(&self) -> &AdapterInfo {
        unimplemented!()
    }

    fn get_feature_flags(&self) -> &DeviceFeatureFlags {
        unimplemented!()
    }

    fn read_buffer(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Option<ReadBackData> {
        unimplemented!()
    }

    fn read_timestamps(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, size_bytes: usize, frame_written_fence: u64) -> Vec<f64> {
        unimplemented!()
    }

    fn read_pipeline_statistics(&self, swap_chain: &Self::SwapChain, buffer: &Self::Buffer, frame_written_fence: u64) -> Option<PipelineStatistics> {
        unimplemented!()
    }

    fn report_live_objects(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn get_info_queue_messages(&self) -> Result<Vec<String>, Error> {
        unimplemented!()
    }

    fn get_timestamp_size_bytes() -> usize {
        unimplemented!()
    }

    fn get_pipeline_statistics_size_bytes() -> usize {
        unimplemented!()
    }

    fn get_indirect_command_size(argument_type: IndirectArgumentType) -> usize {
        unimplemented!()
    }

    fn get_counter_alignment() -> usize {
        unimplemented!()
    }
}

impl super::Texture<Device> for Texture {
    fn get_srv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_uav_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_subresource_uav_index(&self, subresource: u32) -> Option<usize> {
        unimplemented!()
    }

    fn get_msaa_srv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn clone_inner(&self) -> Self {
        unimplemented!()
    }

    fn is_resolvable(&self) -> bool {
        unimplemented!()
    }

    fn get_shader_heap_id(&self) -> Option<u16> {
        unimplemented!()
    }
}

impl super::Buffer<Device> for Buffer {
    fn update<T: Sized>(&mut self, offset: usize, data: &[T]) -> Result<(), Error> {
        unimplemented!()
    }

    fn write<T: Sized>(&mut self, offset: usize, data: &[T]) -> Result<(), Error> {
        unimplemented!()
    }

    fn map(&mut self, info: &MapInfo) -> *mut u8 {
        unimplemented!()
    }

    fn unmap(&mut self, info: &UnmapInfo) {
        unimplemented!()
    }

    fn get_srv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_cbv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_uav_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_vbv(&self) -> Option<VertexBufferView> {
        unimplemented!()
    }

    fn get_ibv(&self) -> Option<IndexBufferView> {
        unimplemented!()
    }

    fn get_counter_offset(&self) -> Option<usize> {
        unimplemented!()
    }
}

impl super::Heap<Device> for Heap {
    fn deallocate(&mut self, index: usize) {
        unimplemented!()
    }

    fn cleanup_dropped_resources(&mut self, swap_chain: &SwapChain) {
        unimplemented!()
    }

    fn get_heap_id(&self) -> u16 {
        unimplemented!()
    }
}

impl super::QueryHeap<Device> for QueryHeap {
    fn reset(&mut self) {
        unimplemented!()
    }
}

impl super::ReadBackRequest<Device> for ReadBackRequest {
    fn is_complete(&self, swap_chain: &SwapChain) -> bool {
        unimplemented!()
    }

    fn map(&self, info: &MapInfo) -> Result<ReadBackData, Error> {
        unimplemented!()
    }

    fn unmap(&self) {
        unimplemented!()
    }
}

impl super::RenderPass<Device> for RenderPass {
    fn get_format_hash(&self) -> u64 {
        unimplemented!()
    }
}

impl super::RaytracingTLAS<Device> for RaytracingTLAS {
    fn get_srv_index(&self) -> Option<usize> {
        unimplemented!()
    }

    fn get_shader_heap_id(&self) -> u16 {
        unimplemented!()
    }
}


impl super::Shader<Device> for Shader {}
impl super::RenderPipeline<Device> for RenderPipeline {}
impl super::ComputePipeline<Device> for ComputePipeline {}
impl super::RaytracingPipeline<Device> for RaytracingPipeline {}
impl super::CommandSignature<Device> for CommandSignature {}
impl super::RaytracingShaderBindingTable<Device> for RaytracingShaderBindingTable {}
impl super::RaytracingBLAS<Device> for RaytracingBLAS {}