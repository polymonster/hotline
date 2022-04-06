use crate::gfx;
use gfx::d3d12;
use gfx::Device;

use windows::{
    core::*, Win32::Foundation::*,
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Direct3D::*, Win32::Foundation::HINSTANCE,
    Win32::Media::MediaFoundation::*, Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Win32::System::Com::CoCreateInstance, Win32::System::Com::CoInitialize, Win32::System::Com::CLSCTX_ALL, 
};

pub struct VideoPlayer {
    device: ID3D11Device
}

impl super::VideoPlayer<d3d12::Device> for VideoPlayer {
    fn create(device: &d3d12::Device) {

        let factory = d3d12::get_dxgi_factory(&device);
        let (adapter, _) = d3d12::get_hardware_adapter(factory, &Some(device.get_adapter_info().name.to_string())).unwrap();

        unsafe {
            // create device
            let mut device : Option<ID3D11Device> = None;
            D3D11CreateDevice(
                adapter, 
                D3D_DRIVER_TYPE_UNKNOWN, 
                HINSTANCE(0), 
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                &[],
                D3D11_SDK_VERSION,
                &mut device,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let dev = device.unwrap();

            // make thread safe
            let mt : ID3D11Multithread = dev.cast().unwrap();
            mt.SetMultithreadProtected(BOOL::from(true));

            // setup media engine
            let mut reset_token : u32 = 0;
            let mut dxgi_manager : Option<IMFDXGIDeviceManager> = None;
            MFCreateDXGIDeviceManager(&mut reset_token, &mut dxgi_manager);
            if let Some(dxgi_manager) = &dxgi_manager {
                let d : IUnknown = dev.cast().unwrap();
                dxgi_manager.ResetDevice(d, reset_token);
            }

            // create attributes
            let mut attributes : Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1);

            let attributes = attributes.unwrap();
            let idxgi_manager : IUnknown = dev.cast().unwrap();
            
            attributes.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, idxgi_manager);
            attributes.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0);

            // create event callback
            let idxgi_manager : IUnknown = dev.cast().unwrap();
            attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, idxgi_manager);

            // create media engine
            CoInitialize(std::ptr::null_mut());
            let mf_factory : IMFMediaEngineClassFactory = 
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_ALL).unwrap();
            let media_engine = mf_factory.CreateInstance(0, attributes).unwrap();

            // create media engine ex
            let boobs = 2;

            /*
                // Create MediaEngine.
                ComPtr<IMFMediaEngineClassFactory> mfFactory;
                DX::ThrowIfFailed(
                    CoCreateInstance(CLSID_MFMediaEngineClassFactory,
                    nullptr,
                    CLSCTX_ALL,
                    IID_PPV_ARGS(mfFactory.GetAddressOf())));

                DX::ThrowIfFailed(
                    mfFactory->CreateInstance(0,
                    attributes.Get(),
                    m_mediaEngine.ReleaseAndGetAddressOf()));

                // Create MediaEngineEx
                DX::ThrowIfFailed(m_mediaEngine.As(&m_engineEx));
            */
        }   
    }
}