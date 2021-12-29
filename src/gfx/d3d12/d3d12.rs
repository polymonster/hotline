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

pub struct SwapChain {
    name: String,
    cur_frame_ctx: i32,
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
    command_list: Vec<ID3D12GraphicsCommandList>
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

pub fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
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

            // assign struct
            CmdBuf {
                cur_frame_index: 1,
                command_allocator: command_allocators,
                command_list: command_lists
            }
        }
    }

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = ID3D12CommandList::from(&cmd.command_list[cmd.cur_frame_index]);
            self.command_queue.ExecuteCommandLists(
                1, &mut Some(command_list));
            println!("exec {}", cmd.cur_frame_index);
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
            println!("waiting {} {}", self.cur_frame_ctx, fv);

            if fv != 0 // means no fence was signaled
            {
                fv = 0;
                self.fence.SetEventOnCompletion(fv as u64, self.fence_event);
                waitable[1] = self.fence_event;
                num_waitable = 2;
            }
            
            
            WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
        }
        println!("new_frame");
    }
    fn get_frame_index(&self) -> i32 {
        self.cur_frame_ctx
    }
    fn swap(&mut self, device: &Device) {
        unsafe {
            self.swap_chain.Present(1, 0);
            let fv = self.fence_last_signalled_value + 1;
            println!("signal {}", fv);
            device.command_queue.Signal(&self.fence, fv as u64);
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.cur_frame_ctx as usize] = fv;
        }
        
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
        println!("reset {}", self.cur_frame_index);
    }
    fn clear_debug(&self, queue: &SwapChain) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };

        // Indicate that the back buffer will be used as a render target.
        let barrier = transition_barrier(
            &queue.render_targets[bb],
            D3D12_RESOURCE_STATE_PRESENT,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
        );
        unsafe { self.command_list[bb].ResourceBarrier(1, &barrier) };

        unsafe {
            self.command_list[bb].ClearRenderTargetView(
                queue.rtv_handles[bb], [0.0, 1.0, 0.0, 1.0].as_ptr(), 0, std::ptr::null());
        }

        // Indicate that the back buffer will now be used to present.
        unsafe {
            self.command_list[bb].ResourceBarrier(
                1,
                &transition_barrier(
                    &queue.render_targets[bb],
                    D3D12_RESOURCE_STATE_RENDER_TARGET,
                    D3D12_RESOURCE_STATE_PRESENT,
                ),
            );

            self.command_list[bb].Close();
        }
        println!("clear {}", bb);
    }
}

pub enum Graphics {}
impl gfx::Graphics for Graphics {
    type Device = Device;
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
}