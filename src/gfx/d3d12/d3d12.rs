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
    device: ID3D12Device,
    dxgi_factory: IDXGIFactory4
}

pub struct Queue {
    name: String,
    command_queue: ID3D12CommandQueue,
    fence: ID3D12Fence,
    fence_value: i32
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
            if cfg!(debug_assertions) {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                    debug.EnableDebugLayer();
                }
            }
    
            let dxgi_factory_flags = if cfg!(debug_assertions) {
                DXGI_CREATE_FACTORY_DEBUG
            } else {
                0
            };
    
            let dxgi_factory: IDXGIFactory4 = CreateDXGIFactory2(dxgi_factory_flags).unwrap();
            let adapter = get_hardware_adapter(&dxgi_factory).unwrap();
            
            let mut d3d12_device: Option<ID3D12Device> = None;
            D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut d3d12_device);
            Device {
                name: String::from("d3d12 device"),
                device: d3d12_device.unwrap(),
                dxgi_factory: dxgi_factory
            }
        }
    }
    fn create_queue(&self) -> Queue {
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
                fence_value: 1
            }
        }
    }
}

impl gfx::Queue<Graphics> for Queue {
    fn create_swap_chain(&self, device: Device, win: win32::Window) {
        unsafe { 
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
            let swap_chain: IDXGISwapChain1 =
            device.dxgi_factory.CreateSwapChainForHwnd(
                &self.command_queue,
                win.get_native_handle(),
                &swap_chain_desc,
                std::ptr::null(),
                None,
            ).unwrap();
        }
    }
}

pub enum Graphics {}
impl gfx::Graphics for Graphics {
    type Device = Device;
    type Queue = Queue;
}

/*
            if (d3d12_device->CreateFence(0, D3D12_FENCE_FLAG_NONE, IID_PPV_ARGS(&ctx->fence)) != S_OK)
                return nullptr;

            ctx->fence_event = CreateEvent(NULL, FALSE, FALSE, NULL);
            if (ctx->fence_event == NULL)
                return nullptr;

            // swap chain
            {
                DXGI_SWAP_CHAIN_DESC1 sd;
                ZeroMemory(&sd, sizeof(sd));
                sd.BufferCount = 3;
                sd.Width = 0;
                sd.Height = 0;
                sd.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
                sd.Flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT;
                sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
                sd.SampleDesc.Count = 1;
                sd.SampleDesc.Quality = 0;
                sd.SwapEffect = DXGI_SWAP_EFFECT_FLIP_DISCARD;
                sd.AlphaMode = DXGI_ALPHA_MODE_UNSPECIFIED;
                sd.Scaling = DXGI_SCALING_STRETCH;
                sd.Stereo = FALSE;

                IDXGIFactory4* dxgi_factory = NULL;
                IDXGISwapChain1* swap_chain1 = NULL;

                if (CreateDXGIFactory1(IID_PPV_ARGS(&dxgi_factory)) != S_OK)
                    return nullptr;

                if (dxgi_factory->CreateSwapChainForHwnd(ctx->command_queue, win32_window->hwnd, &sd, NULL, NULL, &swap_chain1) != S_OK)
                    return nullptr;

                if (swap_chain1->QueryInterface(IID_PPV_ARGS(&ctx->swap_chain)) != S_OK)
                    return nullptr;

                swap_chain1->Release();
                dxgi_factory->Release();

                ctx->swap_chain->SetMaximumFrameLatency(3);
                ctx->swap_chain_wait = ctx->swap_chain->GetFrameLatencyWaitableObject();
            }

            // create backbuffer resources
            for (UINT i = 0; i < 3; i++)
            {
                ID3D12Resource* bb = NULL;
                ctx->swap_chain->GetBuffer(i, IID_PPV_ARGS(&bb));
                d3d12_device->CreateRenderTargetView(bb, NULL, ctx->backbuffer_descriptor[i]);
                ctx->backbuffer[i] = bb;
            }

*/