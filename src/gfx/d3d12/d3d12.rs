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

pub struct Device {
    name: String,
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

impl gfx::Device<GraphicsAPI> for Device {
    fn create() -> Device {
        let mut dev = Device {
            name: String::from("d3d12 device")
        };
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
            D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut d3d12_device).unwrap();
        }
        dev
    }
}

pub enum GraphicsAPI {}
impl gfx::GraphicsAPI for GraphicsAPI {
    type Device = Device;
}

/*
        DeviceD3D12* device = new DeviceD3D12;

        // enable debug interface
#ifdef _DEBUG
        ID3D12Debug* d3d12_debug = nullptr;
        if (SUCCEEDED(D3D12GetDebugInterface(IID_PPV_ARGS(&d3d12_debug))))
            d3d12_debug->EnableDebugLayer();
#endif

        // create device
        D3D_FEATURE_LEVEL lvl = D3D_FEATURE_LEVEL_11_0;
        if (D3D12CreateDevice(NULL, lvl, IID_PPV_ARGS(&device->d3d12_device)) != S_OK)
            return nullptr;

        // shorthand ref to d3d12 device
        auto& d3d12_device = device->d3d12_device;

        // setup debug interface to break on any warnings/errors
#ifdef _DEBUG
        if (d3d12_debug != nullptr)
        {
            ID3D12InfoQueue* info_queue = nullptr;
            d3d12_device->QueryInterface(IID_PPV_ARGS(&info_queue));
            info_queue->SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_ERROR, true);
            info_queue->SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_CORRUPTION, true);
            info_queue->SetBreakOnSeverity(D3D12_MESSAGE_SEVERITY_WARNING, true);
            info_queue->Release();
            d3d12_debug->Release();
        }
#endif

        // all good!
        return (GraphicsDevice)device;

*/