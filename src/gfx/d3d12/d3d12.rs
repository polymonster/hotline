use windows::{
    core::*, 
    Win32::Foundation::*, 
    Win32::Graphics::Direct3D::Fxc::*, 
    Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, 
    Win32::Graphics::Dxgi::Common::*, 
    Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, 
    Win32::System::Threading::*,
    Win32::System::WindowsProgramming::*, 
    Win32::UI::WindowsAndMessaging::*,
    Win32::Graphics::Gdi::ValidateRect
};

const FRAME_COUNT: u32 = 2;

use os::Window;

pub struct Device {
    name: String,
    adapter: IDXGIAdapter1,
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    command_queue: ID3D12CommandQueue,
    val: i32
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

pub struct Sample {
    vertex_buffer: ID3D12Resource,
    root_signature: ID3D12RootSignature,
    pso: ID3D12PipelineState,
    vbv: D3D12_VERTEX_BUFFER_VIEW,
}

pub struct SwapChain {
    name: String,
    cur_frame_ctx: i32,
    width: i32,
    height: i32,
    fence: ID3D12Fence,
    fence_last_signalled_value: i32,
    fence_event: HANDLE,
    swap_chain: IDXGISwapChain3,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE>,
    render_targets: Vec<ID3D12Resource>,
    frame_index: i32,
    frame_fence_value: [i32; FRAME_COUNT as usize]
}

pub struct CmdBuf {
    cur_frame_index: usize,
    command_allocator: Vec<ID3D12CommandAllocator>,
    command_list: Vec<ID3D12GraphicsCommandList>,
    sample: Sample
}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
                pResource: Some(resource.clone()),
                StateBefore: state_before,
                StateAfter: state_after,
                Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
            }),
        },
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
                    D3D_FEATURE_LEVEL_11_0,
                    std::ptr::null_mut::<Option<ID3D12Device>>(),
            ).is_ok()
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

fn create_pipeline_state(
    device: &ID3D12Device,
    root_signature: &ID3D12RootSignature,
) -> Result<ID3D12PipelineState> {
    let compile_flags = if cfg!(debug_assertions) {
        D3DCOMPILE_DEBUG | D3DCOMPILE_SKIP_OPTIMIZATION
    } else {
        0
    };

    let exe_path = std::env::current_exe().ok().unwrap();
    let asset_path = exe_path.parent().unwrap();
    let shaders_hlsl_path = asset_path.join("..\\..\\src\\shaders.hlsl");
    let shaders_hlsl = shaders_hlsl_path.to_str().unwrap();

    println!("shaders: {}", shaders_hlsl);

    let mut vertex_shader = None;
    let vertex_shader = unsafe {
        D3DCompileFromFile(
            shaders_hlsl,
            std::ptr::null_mut(),
            None,
            "VSMain",
            "vs_5_0",
            compile_flags,
            0,
            &mut vertex_shader,
            std::ptr::null_mut(),
        )
    }
    .map(|()| vertex_shader.unwrap())?;

    let mut pixel_shader = None;
    let pixel_shader = unsafe {
        D3DCompileFromFile(
            shaders_hlsl,
            std::ptr::null_mut(),
            None,
            "PSMain",
            "ps_5_0",
            compile_flags,
            0,
            &mut pixel_shader,
            std::ptr::null_mut(),
        )
    }
    .map(|()| pixel_shader.unwrap())?;

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

    let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
        InputLayout: D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: input_element_descs.as_mut_ptr(),
            NumElements: input_element_descs.len() as u32,
        },
        pRootSignature: Some(root_signature.clone()), // << https://github.com/microsoft/windows-rs/discussions/623
        VS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { vertex_shader.GetBufferPointer() },
            BytecodeLength: unsafe { vertex_shader.GetBufferSize() },
        },
        PS: D3D12_SHADER_BYTECODE {
            pShaderBytecode: unsafe { pixel_shader.GetBufferPointer() },
            BytecodeLength: unsafe { pixel_shader.GetBufferSize() },
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

    unsafe { device.CreateGraphicsPipelineState(&desc) }
}

#[repr(C)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 4],
}

fn create_vertex_buffer(
    device: &ID3D12Device,
    aspect_ratio: f32,
) -> Result<(ID3D12Resource, D3D12_VERTEX_BUFFER_VIEW)> {
    let vertices = [
        Vertex {
            position: [0.0, 0.25 * aspect_ratio, 0.0],
            color: [1.0, 0.0, 0.0, 1.0],
        },
        Vertex {
            position: [0.25, -0.25 * aspect_ratio, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
        },
        Vertex {
            position: [-0.25, -0.25 * aspect_ratio, 0.0],
            color: [0.0, 0.0, 1.0, 1.0],
        },
    ];

    // Note: using upload heaps to transfer static data like vert buffers is
    // not recommended. Every time the GPU needs it, the upload heap will be
    // marshalled over. Please read up on Default Heap usage. An upload heap
    // is used here for code simplicity and because there are very few verts
    // to actually transfer.
    let mut vertex_buffer: Option<ID3D12Resource> = None;
    unsafe {
        device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES {
                Type: D3D12_HEAP_TYPE_UPLOAD,
                ..Default::default()
            },
            D3D12_HEAP_FLAG_NONE,
            &D3D12_RESOURCE_DESC {
                Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                Width: std::mem::size_of_val(&vertices) as u64,
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
            &mut vertex_buffer,
        )?
    };
    let vertex_buffer = vertex_buffer.unwrap();

    // Copy the triangle data to the vertex buffer.
    unsafe {
        let mut data = std::ptr::null_mut();
        vertex_buffer.Map(0, std::ptr::null(), &mut data)?;
        std::ptr::copy_nonoverlapping(
            vertices.as_ptr(),
            data as *mut Vertex,
            std::mem::size_of_val(&vertices),
        );
        vertex_buffer.Unmap(0, std::ptr::null());
    }

    let vbv = D3D12_VERTEX_BUFFER_VIEW {
        BufferLocation: unsafe { vertex_buffer.GetGPUVirtualAddress() },
        StrideInBytes: std::mem::size_of::<Vertex>() as u32,
        SizeInBytes: std::mem::size_of_val(&vertices) as u32,
    };

    Ok((vertex_buffer, vbv))
}

impl gfx::Device<Graphics> for Device {
    fn create() -> Device {
        unsafe {
            // enable debug layer
            let mut dxgi_factory_flags : u32 = 0;
            if cfg!(debug_assertions) {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                    debug.EnableDebugLayer();
                }
                dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
            }
        
            // create dxgi factory
            let dxgi_factory_result = CreateDXGIFactory2(dxgi_factory_flags);
            if !dxgi_factory_result.is_ok() {
                panic!("failed to create dxgi factory");
            }

            // create adapter
            let dxgi_factory = dxgi_factory_result.unwrap();
            let adapter_result = get_hardware_adapter(&dxgi_factory);
            if !adapter_result.is_ok() {
                panic!("failed to get hardware adapter");
            }
            

            // create device
            let adapter = adapter_result.unwrap();
            let mut d3d12_device: Option<ID3D12Device> = None;
            let device_result = D3D12CreateDevice(adapter.clone(), D3D_FEATURE_LEVEL_11_0, &mut d3d12_device);
            if !device_result.is_ok() {
                panic!("failed to create d3d12 device");
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
                println!("failed to create command queue");
            }
            let command_queue = command_queue_result.unwrap();

            // construct device and return
            Device {
                name: String::from("d3d12 device"),
                adapter: adapter,
                device: device,
                dxgi_factory: dxgi_factory,
                command_queue: command_queue,
                val: 69
            }
        }
    }

    fn create_swap_chain(&self, win: &win32::Window) -> SwapChain {
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
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            // create swap chain itself
            let swap_chain_result =
            self.dxgi_factory.CreateSwapChainForHwnd(
                &self.command_queue,
                win.get_native_handle(),
                &swap_chain_desc,
                std::ptr::null(),
                None,
            );
            if !swap_chain_result.is_ok() {
                panic!("failed to create swap chain for window");
            }
            let swap_chain : IDXGISwapChain3 = swap_chain_result.unwrap().cast().unwrap();

            // create rtv heap
            let rtv_heap_result =
                self.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                NumDescriptors: FRAME_COUNT,
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                ..Default::default()
            });
            if !rtv_heap_result.is_ok() {
                panic!("failed to create rtv heap for swap chain");
            }

            let rtv_heap : ID3D12DescriptorHeap = rtv_heap_result.unwrap();
            let rtv_descriptor_size = self.device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE_RTV
            ) as usize;
            let rtv_handle = rtv_heap.GetCPUDescriptorHandleForHeapStart();

            // render targets for the swap chain
            let mut handles : Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = Vec::new();
            let mut render_targets : Vec<ID3D12Resource> = Vec::new();
            
            for i in 0..FRAME_COUNT {
                let render_target: ID3D12Resource = swap_chain.GetBuffer(i).unwrap();
                let sub_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                    ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
                };
                self.device.CreateRenderTargetView(
                    &render_target,
                    std::ptr::null_mut(),
                    &sub_handle
                );
                handles.push(sub_handle);
                render_targets.push(render_target);
            }

            let fence_event = CreateEventA(std::ptr::null_mut(), false, false, None);

            // initialise struct
            SwapChain {
                name: String::from("d3d12 queue"),
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
                width: rect.width,
                height: rect.height,
                cur_frame_ctx: 0,
                fence_last_signalled_value: 0,
                fence_event: fence_event,
                swap_chain: swap_chain,
                rtv_heap: rtv_heap,
                rtv_handles: handles,
                render_targets: render_targets,
                frame_index: 0,
                frame_fence_value: [0, 0]
            }
        }
    }

    fn create_cmd_buf(&self) -> CmdBuf {
        unsafe {
            
            let mut command_allocators: Vec<ID3D12CommandAllocator> = Vec::new();
            let mut command_lists: Vec<ID3D12GraphicsCommandList> = Vec::new();

            for _ in 0..FRAME_COUNT as usize {
                // create command allocator
                let command_allocator = self.device
                    .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT);
                if !command_allocator.is_ok() {
                    panic!("failed to create command allocator");
                }
                let command_allocator = command_allocator.unwrap();

                // create command list
                let command_list =
                    self.device.CreateCommandList(
                        0,
                        D3D12_COMMAND_LIST_TYPE_DIRECT,
                        &command_allocator,
                        None,
                    );
                if !command_list.is_ok() {
                    panic!("failed to create command list");
                }
                let command_list = command_list.unwrap();

                command_allocators.push(command_allocator);
                command_lists.push(command_list);
            }

            let root_signature = create_root_signature(&self.device).unwrap();
            let pso = create_pipeline_state(&self.device, &root_signature).unwrap();
            let (vertex_buffer, vbv) = create_vertex_buffer(&self.device, 16.0/9.0).unwrap();

            let sample = Sample {
                root_signature: root_signature,
                pso: pso,
                vertex_buffer: vertex_buffer,
                vbv: vbv
            };

            // assign struct
            CmdBuf {
                cur_frame_index: 1,
                command_allocator: command_allocators,
                command_list: command_lists,
                sample: sample
            }
        }
    }

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = ID3D12CommandList::from(&cmd.command_list[cmd.cur_frame_index]);
            self.command_queue.ExecuteCommandLists(
                1, &mut Some(command_list));
            // println!("exec {}", cmd.cur_frame_index);
        }
    }

    // tests
    fn test_mutate(&mut self) {
        self.val += 1
    }

    fn print_mutate(&self) {
        println!("mutated value {}", self.val);
    }
}

impl gfx::SwapChain<Graphics> for SwapChain {
    fn new_frame(&mut self) {
        let next_frame_index = self.frame_index + 1;
        self.frame_index = next_frame_index;
        self.cur_frame_ctx = next_frame_index % FRAME_COUNT as i32;
        unsafe {
            let mut waitable : [HANDLE; 2] = [self.swap_chain.GetFrameLatencyWaitableObject(), HANDLE(0)];
            let mut num_waitable = 1;

            let mut fv = self.frame_fence_value[self.cur_frame_ctx as usize];
            //println!("waiting {} {}", self.cur_frame_ctx, fv);

            if fv != 0 // means no fence was signaled
            {
                fv = 0;
                self.fence.SetEventOnCompletion(fv as u64, self.fence_event);
                waitable[1] = self.fence_event;
                num_waitable = 2;
            }

            WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
        }
    }

    fn update(&mut self, device: &Device, window: &win32::Window) {
        let wh = window.get_size();
        if wh.0 != self.width || wh.1 != self.height {
            println!("swap chain resize required!");
            let mut handles : Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = Vec::new();
            let mut render_targets : Vec<ID3D12Resource> = Vec::new();

            self.render_targets.clear();
            self.rtv_handles.clear();

            //self.render_targets[0].into_param()

            std::thread::sleep_ms(1000);

            unsafe {
                let mut waitable : [HANDLE; 2] = [self.swap_chain.GetFrameLatencyWaitableObject(), HANDLE(0)];
                let mut num_waitable = 1;
    
                let mut fv = self.frame_fence_value[self.cur_frame_ctx as usize];
                //println!("waiting {} {}", self.cur_frame_ctx, fv);
    
                if fv != 0 // means no fence was signaled
                {
                    fv = 0;
                    self.fence.SetEventOnCompletion(fv as u64, self.fence_event);
                    waitable[1] = self.fence_event;
                    num_waitable = 2;
                }
    
                WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
            }

            unsafe {
                let res = self.swap_chain.ResizeBuffers(FRAME_COUNT, wh.0 as u32, wh.1 as u32, DXGI_FORMAT_UNKNOWN, 0);
                if !res.is_ok() {
                    let err = res.err();
                    if err.is_some() {
                        let eee = err.unwrap();
                        println!("swap chain resize failed {}", eee);
                    }
                }
                
                let rtv_descriptor_size = device.device.GetDescriptorHandleIncrementSize(
                    D3D12_DESCRIPTOR_HEAP_TYPE_RTV
                ) as usize;
                let rtv_handle = self.rtv_heap.GetCPUDescriptorHandleForHeapStart();
                
                for i in 0..FRAME_COUNT {
                    let render_target: ID3D12Resource = self.swap_chain.GetBuffer(i).unwrap();
                    let sub_handle = D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
                    };
                    device.device.CreateRenderTargetView(
                        &render_target,
                        std::ptr::null_mut(),
                        &sub_handle
                    );
                    handles.push(sub_handle);
                    render_targets.push(render_target);
                }
            }

            self.rtv_handles = handles;
            self.render_targets = render_targets;
            self.width = wh.0;
            self.height = wh.1;
        }
    }

    fn get_frame_index(&self) -> i32 {
        self.cur_frame_ctx
    }

    fn swap(&mut self, device: &Device) {
        unsafe {
            self.swap_chain.Present(1, 0);
            let fv = self.fence_last_signalled_value + 1;
            // println!("signal {}", fv);
            device.command_queue.Signal(&self.fence, fv as u64);
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.cur_frame_ctx as usize] = fv;
        }
    }
}

impl CmdBuf {
    fn cmd(&self) -> &ID3D12GraphicsCommandList {
        &self.command_list[self.cur_frame_index]
    }
}

impl gfx::CmdBuf<Graphics> for CmdBuf {
    fn reset(&mut self, queue: &SwapChain) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };
        unsafe { 
            self.command_allocator[bb].Reset();
            self.command_list[bb].Reset(&self.command_allocator[bb], None);
        }
        self.cur_frame_index = bb;
        // println!("reset {}", self.cur_frame_index);
    }
    fn clear_debug(&self, queue: &SwapChain, r: f32, g: f32, b: f32, a: f32) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };

        // Indicate that the back buffer will be used as a render target.
        let barrier = transition_barrier(
            &queue.render_targets[bb],
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );

        unsafe 
        {
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.command_list[bb].ClearRenderTargetView(
                queue.rtv_handles[bb], [r, g, b, a].as_ptr(), 0, std::ptr::null());
            self.cmd().OMSetRenderTargets(1, &queue.rtv_handles[bb], false, std::ptr::null());
        }
    }
    fn set_state_debug(&self) {
        let cmd = self.cmd();
        unsafe { 
            cmd.SetGraphicsRootSignature(&self.sample.root_signature);
            cmd.SetPipelineState(&self.sample.pso);
            cmd.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            cmd.IASetVertexBuffers(0, 1, &self.sample.vbv);
            cmd.DrawInstanced(3, 1, 0, 0);
        };
    }
    fn set_viewport(&self, viewport: &gfx::Viewport) {
        let d3d12_vp = D3D12_VIEWPORT {
            TopLeftX: viewport.x,
            TopLeftY: viewport.y,
            Width: viewport.width,
            Height: viewport.height,
            MinDepth: viewport.min_depth,
            MaxDepth: viewport.max_depth
        };
        unsafe {
            self.cmd().RSSetViewports(1, &d3d12_vp);
        }     
    }
    fn set_scissor_rect(&self, scissor_rect: &gfx::ScissorRect) {
        let d3d12_sr = RECT {
            left: scissor_rect.left,
            top: scissor_rect.top,
            right: scissor_rect.right,
            bottom: scissor_rect.bottom
        };
        unsafe {
            self.cmd().RSSetScissorRects(1, &d3d12_sr);
        }
    }
    fn draw_instanced(&self, vertex_count: u32, instance_count: u32, start_vertex: u32, start_instance: u32) {
        unsafe {
            self.cmd().DrawInstanced(vertex_count, instance_count, start_vertex, start_instance);
        }
    }
    fn close_debug(&self, queue: &SwapChain) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };
        let cmd = self.cmd();
        // Indicate that the back buffer will now be used to present.
        unsafe {
            cmd.ResourceBarrier(
                1,
                &transition_barrier(
                    &queue.render_targets[bb],
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                    D3D12_RESOURCE_STATE_PRESENT,
                ),
            );
            cmd.Close();
        }
    }
}

pub enum Graphics {}
impl gfx::Graphics for Graphics {
    type Device = Device;
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
}