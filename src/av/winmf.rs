use crate::gfx;
use gfx::d3d12;
use gfx::Device;

use crate::os;
use os::win32;

use windows::{
    core::*, Win32::Foundation::*,
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Direct3D::*, Win32::Foundation::HINSTANCE,
    Win32::Media::MediaFoundation::*, Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Win32::System::Com::CoCreateInstance, Win32::System::Com::CoInitialize, Win32::System::Com::CLSCTX_ALL,
};

pub struct VideoPlayer {
    device: ID3D11Device,
    media_engine_ex: IMFMediaEngineEx,
}

#[implement(IMFMediaEngineNotify)]
struct MediaEngineNotify();

#[allow(non_snake_case)]
impl IMFMediaEngineNotify_Impl for MediaEngineNotify {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> ::windows::core::Result<()>  {
        println!("hello world");
        Ok(())
    }
}

impl super::VideoPlayer<d3d12::Device> for VideoPlayer {
    fn create(device: &d3d12::Device) -> VideoPlayer {
        let factory = d3d12::get_dxgi_factory(&device);
        let (adapter, _) = d3d12::get_hardware_adapter(factory, &Some(device.get_adapter_info().name.to_string())).unwrap();
        unsafe {
            MFStartup(MF_SDK_VERSION << 16 | MF_API_VERSION, 0);
            CoInitialize(std::ptr::null_mut());

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
            let device = device.unwrap();

            // make thread safe
            let mt : ID3D11Multithread = device.cast().unwrap();
            mt.SetMultithreadProtected(BOOL::from(true));

            // setup media engine
            let mut reset_token : u32 = 0;
            let mut dxgi_manager : Option<IMFDXGIDeviceManager> = None;
            MFCreateDXGIDeviceManager(&mut reset_token, &mut dxgi_manager);

            // create attributes
            let mut attributes : Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1);
            let attributes = attributes.unwrap();
            
            if let Some(dxgi_manager) = &dxgi_manager {
                let d : IUnknown = device.cast().unwrap();
                dxgi_manager.ResetDevice(d, reset_token);
                let idxgi_manager : IUnknown = device.cast().unwrap();
                attributes.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, idxgi_manager);
            }

            attributes.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0);

            // create event callback
            let notify = MediaEngineNotify {};
            let imn : IMFMediaEngineNotify = notify.into();
            attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, imn);

            // create media engine
            let mf_factory : IMFMediaEngineClassFactory = 
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_ALL).unwrap();
            let media_engine = mf_factory.CreateInstance(0, attributes).unwrap();

            VideoPlayer {
                device: device,
                media_engine_ex: media_engine.cast().unwrap(),
            }
        }
    }

    fn set_source(&self, filepath: String) {
        unsafe {
            let mb = win32::string_to_multibyte(filepath);
            let bstr = SysAllocString(PCWSTR(mb.as_ptr() as _));
            self.media_engine_ex.SetSource(bstr);
        }
    }
}