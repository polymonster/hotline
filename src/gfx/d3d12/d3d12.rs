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

pub struct InternalDevice {
    device: ID3D12Device
}

pub struct Device {
    name: String,
    adapter: IDXGIAdapter1,
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    val: i32
}

unsafe impl Send for Device {}
unsafe impl Sync for Device {}

pub struct Queue {
    name: String,
    command_queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    fence_value: i32,
    swap_chain: Option<IDXGISwapChain1>,
    rtv_heap: Option<ID3D12DescriptorHeap>,
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

            // construct device and return
            Device {
                name: String::from("d3d12 device"),
                adapter: adapter,
                device: device,
                dxgi_factory: dxgi_factory,
                val: 69
            }
        }
    }
    fn create_queue(&self) -> Queue {
        println!("creating queue");
        unsafe {
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                NodeMask: 1, 
                ..Default::default()
            };
            let command_queue = self.device.CreateCommandQueue(&desc).unwrap();
            Queue {
                name: String::from("d3d12 queue"),
                command_queue: command_queue,
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
                fence_value: 1,
                swap_chain: None,
                rtv_heap: None,
            }
        }
    }
    fn test_mutate(&mut self) {
        self.val += 1
    }
    fn print_mutate(&self) {
        println!("mutated value {}", self.val);
    }
}

impl gfx::Queue<Graphics> for Queue {
    fn create_swap_chain(&mut self, device: Device, win: win32::Window) {
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
            device.dxgi_factory.CreateSwapChainForHwnd(
                &self.command_queue,
                win.get_native_handle(),
                &swap_chain_desc,
                std::ptr::null(),
                None,
            );
            if !swap_chain_result.is_ok() {
                panic!("failed to create swap chain for window");
            }
            let swap_chain = swap_chain_result.unwrap();

            // create rtv heap
            let rtv_heap_result =
                device.device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                NumDescriptors: FRAME_COUNT,
                Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                ..Default::default()
            });
            if !rtv_heap_result.is_ok() {
                panic!("failed to create rtv heap for swap chain");
            }

            let rtv_heap : ID3D12DescriptorHeap = rtv_heap_result.unwrap();
            let rtv_descriptor_size = device.device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE_RTV
            ) as usize;
            let rtv_handle = rtv_heap.GetCPUDescriptorHandleForHeapStart();

            // render targets for the swap chain
            for i in 0..FRAME_COUNT {
                let render_target: ID3D12Resource = swap_chain.GetBuffer(i).unwrap();
                device.device.CreateRenderTargetView(
                    &render_target,
                    std::ptr::null_mut(),
                    &D3D12_CPU_DESCRIPTOR_HANDLE {
                        ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
                    }
                );
            }

            self.swap_chain = Some(swap_chain);
            self.rtv_heap = Some(rtv_heap);
        }
    }
}

pub enum Graphics {}
impl gfx::Graphics for Graphics {
    type Device = Device;
    type Queue = Queue;
}