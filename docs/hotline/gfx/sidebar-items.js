initSidebarItems({"enum":[["BlendFactor","Controls how the source and destination terms in blend equation are derrived"],["BlendOp","Controls how the source and destination terms are combined: final = src (op) dest"],["BufferUsage","Describes how a buffer will be used on the GPU."],["ComparisonFunc","Used for comparison ops in depth testing, samplers."],["CullMode","Polygon cull mode"],["DepthWriteMask","Write to the depth buffer, or omit writes and just perform depth testing"],["DescriptorType","Describes the type of descriptor binding to create."],["ErrorType","Error types for different gfx backends and FFI calls"],["FillMode","Polygon fillmode"],["Format","Format for resource types (textures / buffers). n = normalised unsigned integer, u = unsigned integer, i = signed integer, f = float"],["HeapType","Options for heap types"],["InputSlotClass","Describes the frequency of which elements are fetched from a vertex input element."],["LogicOp","The logical operation to configure for a render target blend with logic op enabled"],["ResourceState","All possible resource states, some for buffers and some for textures"],["SamplerAddressMode","Address mode for the sampler (controls wrapping and clamping)."],["SamplerFilter","Filtering mode for the sampler (controls bilinear and trilinear interpolation)."],["ShaderType","The stage to which a shader will bind itself."],["ShaderVisibility","Describes the visibility of which shader stages can access a descriptor."],["StencilOp","Stencil operations"],["TextureType","Describes the dimension of a texture"],["Topology","Indicates how the pipeline interprets vertex data at the input assembler stage This will be also used to infer primitive topology types for geometry or hull shaders"]],"fn":[["align","Aligns value to the alignment specified by align. value can be non-power of 2"],["align_pow2","Aligns value to the alignment specified by align. value must be a power of 2"],["as_u8_slice","Take any sized type and return a u8 slice. This can be useful to pass `data` to `Device::create_buffer`."],["block_size_for_format","Returns the ‘block size’ (texel, compressed block of texels or single buffer element) for a given format"],["row_pitch_for_format","Returns the row pitch of an image in bytes: width * block size"],["size_for_format","Return the size in bytes of a 3 dimensional resource: width * height * depth block size"],["slice_as_u8_slice","Take any sized silce and convert to a slice of u8"],["slice_pitch_for_format","Returns the slice pitch of an image in bytes: width * height * block size, a slice is a single 2D image or a single slice of a 3D texture or texture array"]],"mod":[["d3d12","Implemets this interface with a Direct3D12 backend."]],"struct":[["AdapterInfo","Information returned from `Device::get_adapter_info`"],["BlendInfo","Information to control blending operations on render targets"],["BufferInfo","Information to create a buffer through `Device::create_buffer`."],["ClearColour","Values to clear colour render targets at the start of a `RenderPass`"],["ClearDepthStencil","Values to clear depth stencil buffers during a `RenderPass`"],["ComputePipelineInfo","Information to create a compute pipeline through `Device::create_compute_pipeline`"],["CpuAccessFlags","CPU Access flags for buffers or textures"],["DepthStencilInfo","Information to control the depth and stencil testing of primitves when using a `RenderPipeline`"],["DescriptorBinding","Describes a range of resources for access on the GPU."],["DescriptorLayout","Descriptor layout is required to create a pipeline it describes the layout of resources for access on the GPU."],["DeviceInfo","Information to create a device, it contains default heaps for resource views resources will be automatically allocated into these heaps, you can supply custom heaps if necessary"],["Error","Errors passed back from FFI calls to various gfx backends"],["HeapInfo","Information to create a desciptor heap… `Device` will contain default heaps, but you can create your own if required"],["InputElementInfo","Describe a single element of an `InputLayoutInfo`"],["MapInfo","Info to control mapping of resources for read/write access"],["PushConstantInfo","Describes space in the shader to send data to via `CmdBuf::push_constants`."],["RasterInfo","Information to control the rasterisation mode of primitives when using a `RenderPipeline`"],["ReadBackData","Results from an issued ReadBackRequest"],["RenderPassInfo","Information to create a render pass"],["RenderPipelineInfo","Information to create a pipeline through `Device::create_render_pipeline`."],["RenderTargetBlendInfo","Blending operations for a single render target"],["SamplerInfo","Info to create a sampler state object to sample textures in shaders."],["ScissorRect","Structure to specify scissor rect coordinates on a `CmdBuf`."],["ShaderCompileFlags","Shader compilation flags"],["ShaderCompileInfo","Information required to compile a shader from source code."],["ShaderInfo","Information to create a shader through `Device::create_shader`."],["Size3","3-Dimensional struct for compute shader thread count / thread group size"],["StencilInfo","Stencil info for various outcomes of the depth stencil test"],["SwapChainInfo","Information to pass to `Device::create_swap_chain`"],["TextureInfo","Information to create a pipeline through `Device::create_texture`."],["TextureUsage","Textures can be used in one or more of the following ways"],["TransitionBarrier","Transitions are required to be performed to switch resources from reading to writing or into different formats"],["UnmapInfo","Info to control writing of mapped resources"],["Viewport","Structure to specify viewport coordinates on a `CmdBuf`."],["WriteMask","Render target write mask flags"]],"trait":[["Buffer","An opaque Buffer type used for vertex, index, constant or unordered access."],["CmdBuf","Responsible for buffering graphics commands. Internally it will contain a platform specific command list for each buffer in the associated swap chain. At the start of each frame `reset` must be called with an associated swap chain to internally switch which buffer we are writing to. At the end of each frame `close` must be called and finally the `CmdBuf` can be passed to `Device::execute` to be processed on the GPU."],["ComputePipeline","An opaque compute pipeline type.."],["Device","A GPU device is used to create GPU resources, the device also contains a single a single command queue to which all command buffers will submitted and executed each frame."],["Heap","An opaque shader heap type, use to create views of resources for binding and access in shaders"],["ReadBackRequest","Used to readback data from the GPU, once the request is issued `is_complete` needs to be waited on for completion you must poll this every frame and not block so the GPU can flush the request. Once the result is ready the data can be obtained using `get_data`"],["RenderPass","An opaque RenderPass containing an optional set of colour render targets and an optional depth stencil target"],["RenderPipeline","An opaque render pipeline type set blend, depth stencil, raster states on a pipeline, and bind with `CmdBuf::set_pipeline_state`"],["Shader","An opaque Shader type"],["SwapChain","A swap chain is connected to a window, controls fences and signals as we swap buffers."],["Texture","An opaque Texture type"]]});