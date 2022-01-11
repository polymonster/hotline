#[cfg(target_os = "windows")]
use crate::os::win32 as platform;

use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D::Fxc::*, Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
    Win32::System::Threading::*, Win32::System::WindowsProgramming::*,
};

// TODO: from info
const FRAME_COUNT: u32 = 2;

use crate::os::Window;

pub enum Graphics {}
impl super::Graphics for Graphics {
    type Device = Device;
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Buffer = Buffer;
    type Shader = Shader;
    type Pipeline = Pipeline;
    type Texture = Texture;
}

pub struct Device {
    name: String,
    adapter: IDXGIAdapter1,
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    command_queue: ID3D12CommandQueue,
    val: i32,
}

pub struct SwapChain {
    width: i32,
    height: i32,
    frame_index: i32,
    bb_index: i32,
    swap_chain: IDXGISwapChain3,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE>,
    fence: ID3D12Fence,
    fence_last_signalled_value: u64,
    fence_event: HANDLE,
    frame_fence_value: [u64; FRAME_COUNT as usize],
}

pub struct Pipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
}

pub struct CmdBuf {
    bb_index: usize,
    command_allocator: Vec<ID3D12CommandAllocator>,
    command_list: Vec<ID3D12GraphicsCommandList>,
}

pub struct Buffer {
    resource: ID3D12Resource,
    vbv: Option<D3D12_VERTEX_BUFFER_VIEW>,
    ibv: Option<D3D12_INDEX_BUFFER_VIEW>,
}

pub struct Shader {
    blob: ID3DBlob,
}

pub struct Texture {
    resource: ID3D12Resource,
}

impl super::Buffer<Graphics> for Buffer {}
impl super::Shader<Graphics> for Shader {}
impl super::Pipeline<Graphics> for Pipeline {}
impl super::Texture<Graphics> for Texture {}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    let trans = std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: Some(resource.clone()),
        StateBefore: state_before,
        StateAfter: state_after,
        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
    });
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: trans },
    }
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    unsafe {
        for i in 0.. {
            let adapter = factory.EnumAdapters1(i)?;
            let desc = adapter.GetDesc1()?;

            if (DXGI_ADAPTER_FLAG::from(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE)
                != DXGI_ADAPTER_FLAG_NONE
            {
                // Don't select the Basic Render Driver adapter. If you want a
                // software adapter, pass in "/warp" on the command line.
                continue;
            }

            // Check to see whether the adapter supports Direct3D 12, but don't
            // create the actual device yet.
            if D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_12_1,
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
            .is_ok()
            {
                return Ok(adapter);
            }
        }
    }
    unreachable!()
}

fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    let desc = D3D12_ROOT_SIGNATURE_DESC {
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        ..Default::default()
    };

    let mut signature = None;

    let signature = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut signature,
            std::ptr::null_mut(),
        )
    }
    .map(|()| signature.unwrap())?;

    unsafe {
        device.CreateRootSignature(0, signature.GetBufferPointer(), signature.GetBufferSize())
    }
}

impl super::Device<Graphics> for Device {
    fn create() -> Device {
        unsafe {
            // enable debug layer
            let mut dxgi_factory_flags: u32 = 0;
            if cfg!(debug_assertions) {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                    debug.EnableDebugLayer();
                    println!("hotline::gfx::d3d12: enabling debug layer");
                }
                dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
            }

            // create dxgi factory
            let dxgi_factory_result = CreateDXGIFactory2(dxgi_factory_flags);
            if !dxgi_factory_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to create dxgi factory");
            }

            // create adapter
            let dxgi_factory = dxgi_factory_result.unwrap();
            let adapter_result = get_hardware_adapter(&dxgi_factory);
            if !adapter_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to get hardware adapter");
            }

            // create device
            let adapter = adapter_result.unwrap();
            let mut d3d12_device: Option<ID3D12Device> = None;
            let device_result =
                D3D12CreateDevice(adapter.clone(), D3D_FEATURE_LEVEL_11_0, &mut d3d12_device);
            if !device_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to create d3d12 device");
            }
            let device = d3d12_device.unwrap();

            // create queue
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                NodeMask: 1,
                ..Default::default()
            };
            let command_queue_result = device.CreateCommandQueue(&desc);
            if !command_queue_result.is_ok() {
                println!("hotline::gfx::d3d12: failed to create command queue");
            }
            let command_queue = command_queue_result.unwrap();

            // construct device and return
            Device {
                name: String::from("d3d12 device"),
                adapter: adapter,
                device: device,
                dxgi_factory: dxgi_factory,
                command_queue: command_queue,
                val: 69,
            }
        }
    }

    fn create_swap_chain(&self, win: &platform::Window) -> SwapChain {
        unsafe {
            // create swap chain desc
            let rect = win.get_rect();
            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                BufferCount: FRAME_COUNT,
                Width: rect.width as u32,
                Height: rect.height as u32,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                Flags: 64,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            // create swap chain itself
            let swap_chain_result = self.dxgi_factory.CreateSwapChainForHwnd(
                &self.command_queue,
                win.get_native_handle(),
                &swap_chain_desc,
                std::ptr::null(),
                None,
            );
            if !swap_chain_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to create swap chain for window");
            }
            let swap_chain: IDXGISwapChain3 = swap_chain_result.unwrap().cast().unwrap();

            // create rtv heap
            let rtv_heap_result = self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                NumDescriptors: FRAME_COUNT,
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                ..Default::default()
            });
            if !rtv_heap_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to create rtv heap for swap chain");
            }

            let rtv_heap: ID3D12DescriptorHeap = rtv_heap_result.unwrap();
            let rtv_descriptor_size =
                self.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
                    as usize;
            let rtv_handle = rtv_heap.GetCPUDescriptorHandleForHeapStart();

            // render targets for the swap chain
            let mut handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = Vec::new();

            for i in 0..FRAME_COUNT {
                let render_target: ID3D12Resource = swap_chain.GetBuffer(i).unwrap();
                let sub_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
                };
                self.device.CreateRenderTargetView(
                    &render_target,
                    std::ptr::null_mut(),
                    &sub_handle,
                );
                handles.push(sub_handle);
            }

            let fence_event = CreateEventA(std::ptr::null_mut(), false, false, None);

            // initialise struct
            SwapChain {
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
                width: rect.width,
                height: rect.height,
                bb_index: 0,
                fence_last_signalled_value: 0,
                fence_event: fence_event,
                swap_chain: swap_chain,
                rtv_heap: rtv_heap,
                rtv_handles: handles,
                frame_index: 0,
                frame_fence_value: [0, 0],
            }
        }
    }

    fn create_cmd_buf(&self) -> CmdBuf {
        unsafe {
            let mut command_allocators: Vec<ID3D12CommandAllocator> = Vec::new();
            let mut command_lists: Vec<ID3D12GraphicsCommandList> = Vec::new();

            for _ in 0..FRAME_COUNT as usize {
                // create command allocator
                let command_allocator =
                    self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT);
                if !command_allocator.is_ok() {
                    panic!("hotline::gfx::d3d12: failed to create command allocator");
                }
                let command_allocator = command_allocator.unwrap();

                // create command list
                let command_list = self.device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &command_allocator,
                    None,
                );
                if !command_list.is_ok() {
                    panic!("hotline::gfx::d3d12: failed to create command list");
                }
                let command_list = command_list.unwrap();

                command_allocators.push(command_allocator);
                command_lists.push(command_list);
            }

            // assign struct
            CmdBuf {
                bb_index: 1,
                command_allocator: command_allocators,
                command_list: command_lists,
            }
        }
    }

    fn create_pipeline(&self, info: super::PipelineInfo<Graphics>) -> Pipeline {
        let mut input_element_descs: [D3D12_INPUT_ELEMENT_DESC; 2] = [
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PSTR(b"POSITION\0".as_ptr() as _),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D12_INPUT_ELEMENT_DESC {
                SemanticName: PSTR(b"COLOR\0".as_ptr() as _),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 12,
                InputSlotClass: D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];

        let root_signature = create_root_signature(&self.device).unwrap();

        let vs = info.vs.unwrap().blob;
        let ps = info.fs.unwrap().blob;

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            InputLayout: D3D12_INPUT_LAYOUT_DESC {
                pInputElementDescs: input_element_descs.as_mut_ptr(),
                NumElements: input_element_descs.len() as u32,
            },
            pRootSignature: Some(root_signature.clone()),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { vs.GetBufferPointer() },
                BytecodeLength: unsafe { vs.GetBufferSize() },
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { ps.GetBufferPointer() },
                BytecodeLength: unsafe { ps.GetBufferSize() },
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                ..Default::default()
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: false.into(),
                IndependentBlendEnable: false.into(),
                RenderTarget: [
                    D3D12_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: false.into(),
                        LogicOpEnable: false.into(),
                        SrcBlend: D3D12_BLEND_ONE,
                        DestBlend: D3D12_BLEND_ZERO,
                        BlendOp: D3D12_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D12_BLEND_ONE,
                        DestBlendAlpha: D3D12_BLEND_ZERO,
                        BlendOpAlpha: D3D12_BLEND_OP_ADD,
                        LogicOp: D3D12_LOGIC_OP_NOOP,
                        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    },
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                ],
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
            SampleMask: u32::max_value(),
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;

        Pipeline {
            pso: unsafe { self.device.CreateGraphicsPipelineState(&desc).unwrap() },
            root_signature: root_signature,
        }
    }

    fn create_shader(&self, info: super::ShaderInfo, data: &[u8]) -> Shader {
        let mut shader_blob = None;
        if info.compile_info.is_some() {
            // TODO: properly use flags
            let compile_flags = if cfg!(debug_assertions) {
                D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
            } else {
                0
            };
            let compile_info = info.compile_info.unwrap();
            unsafe {
                let mut errors = None;
                let result = D3DCompile(
                    data.as_ptr() as *const core::ffi::c_void,
                    data.len(),
                    PSTR(std::ptr::null_mut() as _),
                    std::ptr::null(),
                    None,
                    PSTR(compile_info.entry_point.as_ptr() as _),
                    PSTR(compile_info.target.as_ptr() as _),
                    compile_flags,
                    0,
                    &mut shader_blob,
                    &mut errors,
                );
                if !result.is_ok() {
                    println!("shader compile failed!");
                }
            }
        }
        Shader {
            blob: shader_blob.unwrap(),
        }
    }

    fn create_buffer(&self, info: super::BufferInfo, data: &[u8]) -> Buffer {
        let mut buf: Option<ID3D12Resource> = None;
        unsafe {
            if !self
                .device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_UPLOAD,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Width: data.len() as u64,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    std::ptr::null(),
                    &mut buf,
                )
                .is_ok()
            {
                // TODO: the error should be passed to the user
                panic!("hotline::gfx::d3d12: failed to create buffer!");
            };
        };
        let buf = buf.unwrap();

        // Copy the triangle data to the vertex buffer.
        unsafe {
            let mut map_data = std::ptr::null_mut();
            buf.Map(0, std::ptr::null(), &mut map_data);
            std::ptr::copy_nonoverlapping(data.as_ptr(), map_data as *mut u8, data.len());
            buf.Unmap(0, std::ptr::null());
        }

        let mut vbv: Option<D3D12_VERTEX_BUFFER_VIEW> = None;
        let mut ibv: Option<D3D12_INDEX_BUFFER_VIEW> = None;

        match info.usage {
            super::BufferUsage::Vertex => {
                vbv = Some(D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: unsafe { buf.GetGPUVirtualAddress() },
                    StrideInBytes: info.stride as u32,
                    SizeInBytes: data.len() as u32,
                });
            }
            super::BufferUsage::Index => {
                ibv = Some(D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: unsafe { buf.GetGPUVirtualAddress() },
                    SizeInBytes: data.len() as u32,
                    Format: DXGI_FORMAT_R16_UNORM,
                })
            }
        }

        Buffer {
            resource: buf,
            vbv: vbv,
            ibv: ibv,
        }
    }

    fn create_texture(&self, info: super::TextureInfo, data: &[u8]) -> Texture {
        let mut tex: Option<ID3D12Resource> = None;
        unsafe {
            if !self
                .device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: match info.tex_type {
                            super::TextureType::Texture1D => D3D12_RESOURCE_DIMENSION_TEXTURE1D,
                            super::TextureType::Texture2D => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                            super::TextureType::Texture3D => D3D12_RESOURCE_DIMENSION_TEXTURE3D,
                        },
                        Alignment: 0,
                        Width: info.width as u64,
                        Height: info.height as u32,
                        DepthOrArraySize: info.depth as u16,
                        MipLevels: info.mip_levels as u16,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: info.samples as u32,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                        Flags: D3D12_RESOURCE_FLAG_NONE,
                    },
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    std::ptr::null(),
                    &mut tex,
                )
                .is_ok()
            {
                // TODO: the error should be passed to the user
                panic!("hotline::gfx::d3d12: failed to create texture!");
            };
        };
        let tex = tex.unwrap();
        Texture { resource: tex }
    }

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = ID3D12CommandList::from(&cmd.command_list[cmd.bb_index]);
            self.command_queue.ExecuteCommandLists(1, &mut Some(command_list));
        }
    }
}

impl SwapChain {
    fn wait_for_frame(&mut self, frame_index: usize) {
        unsafe {
            let mut waitable: [HANDLE; 2] =
                [self.swap_chain.GetFrameLatencyWaitableObject(), HANDLE(0)];
            let mut num_waitable = 1;
            let mut fv = self.frame_fence_value[frame_index as usize];

            if fv != 0
            // means no fence was signaled
            {
                fv = 0;
                if !self.fence.SetEventOnCompletion(fv as u64, self.fence_event).is_ok() {
                    panic!("hotline::gfx::d3d12: failed to set on completion event!");
                }
                waitable[1] = self.fence_event;
                num_waitable = 2;
            }

            WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
        }
    }
}

impl super::SwapChain<Graphics> for SwapChain {
    fn new_frame(&mut self) {
        self.wait_for_frame(self.bb_index as usize);
    }

    fn update(&mut self, device: &Device, window: &platform::Window) {
        let wh = window.get_size();
        if wh.0 != self.width || wh.1 != self.height {
            unsafe {
                self.wait_for_frame(self.bb_index as usize);

                // TODO: how to properly use flags
                let flags = 64;
                let res = self.swap_chain.ResizeBuffers(
                    FRAME_COUNT,
                    wh.0 as u32,
                    wh.1 as u32,
                    DXGI_FORMAT_UNKNOWN,
                    flags,
                );

                if !res.is_ok() {
                    let err = res.err();
                    if err.is_some() {
                        let eee = err.unwrap();
                        println!("swap chain resize failed {}", eee);
                    }
                } else {
                    println!("resize success!");
                }

                // TODO: move into shared function
                let rtv_descriptor_size =
                    device.device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV)
                        as usize;
                let rtv_handle = self.rtv_heap.GetCPUDescriptorHandleForHeapStart();

                let mut handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = Vec::new();
                let mut render_targets: Vec<ID3D12Resource> = Vec::new();
                for i in 0..FRAME_COUNT {
                    let render_target: ID3D12Resource = self.swap_chain.GetBuffer(i).unwrap();
                    let sub_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
                    };
                    device.device.CreateRenderTargetView(
                        &render_target,
                        std::ptr::null_mut(),
                        &sub_handle,
                    );
                    handles.push(sub_handle);
                    render_targets.push(render_target);
                }

                self.rtv_handles = handles;
                self.width = wh.0;
                self.height = wh.1;
            }
        } else {
            self.new_frame();
        }
    }

    fn get_backbuffer_index(&self) -> i32 {
        self.bb_index
    }

    fn swap(&mut self, device: &Device) {
        unsafe {
            self.swap_chain.Present(1, 0);
            let fv = self.fence_last_signalled_value + 1;
            device.command_queue.Signal(&self.fence, fv as u64);
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.bb_index as usize] = fv;

            // swap buffers
            let next_frame_index = self.frame_index + 1;
            self.frame_index = next_frame_index;
            self.bb_index = next_frame_index % FRAME_COUNT as i32;
        }
    }
}

impl CmdBuf {
    fn cmd(&self) -> &ID3D12GraphicsCommandList {
        &self.command_list[self.bb_index]
    }
}

impl super::CmdBuf<Graphics> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        self.bb_index = bb;
        if swap_chain.frame_fence_value[bb] != 0 {
            unsafe {
                if !self.command_allocator[bb].Reset().is_ok() {
                    panic!("hotline::gfx::d3d12: failed to reset command_allocator")
                }
                if !self.command_list[bb].Reset(&self.command_allocator[bb], None).is_ok() {
                    panic!("hotline::gfx::d3d12: to reset command_list")
                };
            }
        }
    }

    fn clear_debug(&mut self, queue: &SwapChain, r: f32, g: f32, b: f32, a: f32) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };

        // Indicate that the back buffer will be used as a render target.
        unsafe {
            let barrier = transition_barrier(
                &queue.swap_chain.GetBuffer(bb as u32).unwrap(),
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );

            self.command_list[bb].ResourceBarrier(1, &barrier);
            let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);

            self.command_list[bb].ClearRenderTargetView(
                queue.rtv_handles[bb],
                [r, g, b, a].as_ptr(),
                0,
                std::ptr::null(),
            );
            self.cmd().OMSetRenderTargets(1, &queue.rtv_handles[bb], false, std::ptr::null());
        }
    }

    fn set_viewport(&self, viewport: &super::Viewport) {
        let d3d12_vp = D3D12_VIEWPORT {
            TopLeftX: viewport.x,
            TopLeftY: viewport.y,
            Width: viewport.width,
            Height: viewport.height,
            MinDepth: viewport.min_depth,
            MaxDepth: viewport.max_depth,
        };
        unsafe {
            self.cmd().RSSetViewports(1, &d3d12_vp);
        }
    }

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
        let d3d12_sr = RECT {
            left: scissor_rect.left,
            top: scissor_rect.top,
            right: scissor_rect.right,
            bottom: scissor_rect.bottom,
        };
        unsafe {
            self.cmd().RSSetScissorRects(1, &d3d12_sr);
        }
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        let cmd = self.cmd();
        if buffer.vbv.is_some() {
            unsafe {
                cmd.IASetVertexBuffers(slot, 1, &buffer.vbv.unwrap());
            }
        }
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
        let cmd = self.cmd();
        if buffer.ibv.is_some() {
            unsafe {
                cmd.IASetIndexBuffer(&buffer.ibv.unwrap());
            }
        }
    }

    fn set_pipeline_state(&self, pipeline: &Pipeline) {
        unsafe {
            let cmd = self.cmd();
            cmd.SetGraphicsRootSignature(&pipeline.root_signature);
            cmd.SetPipelineState(&pipeline.pso);
            cmd.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
    }

    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawInstanced(vertex_count, instance_count, start_vertex, start_instance);
        }
    }

    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawIndexedInstanced(
                index_count,
                instance_count,
                start_index,
                base_vertex,
                start_instance,
            );
        }
    }

    fn close(&self, swap_chain: &SwapChain) {
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        let cmd = self.cmd();
        // Indicate that the back buffer will now be used to present.
        unsafe {
            let barrier = transition_barrier(
                &swap_chain.swap_chain.GetBuffer(bb as u32).unwrap(),
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            self.command_list[bb].ResourceBarrier(1, &barrier);
            let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
            if !cmd.Close().is_ok() {
                panic!("hotline: d3d12 failed to close command list.")
            }
        }
    }
}
