use crate::gfx;
use gfx::d3d12;
use gfx::Device;

use crate::os;
use os::win32;
use std::result;

use windows::{
    core::*, Win32::Foundation::*,
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Direct3D::*, Win32::Foundation::HINSTANCE,
    Win32::Media::MediaFoundation::*, Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Win32::System::Com::CoCreateInstance, Win32::System::Com::CoInitialize, Win32::System::Com::CLSCTX_ALL,
};

pub struct VideoPlayer {
    device: ID3D11Device,
    media_engine_ex: IMFMediaEngineEx,
    notify: *mut NotifyEvents
}

pub struct NotifyEvents {
    can_play: bool,
    playing: bool,
    ended: bool
}

impl NotifyEvents {
    fn event(&mut self, event: u32) {
        match MF_MEDIA_ENGINE_EVENT(event as i32) {
            MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA => {
                println!("MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA");
            },
            MF_MEDIA_ENGINE_EVENT_CANPLAY => {
                self.can_play = true;
            }
            MF_MEDIA_ENGINE_EVENT_PLAY => {
                self.playing = true;
            }
            MF_MEDIA_ENGINE_EVENT_PAUSE => {
                println!("MF_MEDIA_ENGINE_EVENT_PAUSE");
            }
            MF_MEDIA_ENGINE_EVENT_ENDED => {
                self.ended = true;
                self.playing = false;
            }
            MF_MEDIA_ENGINE_EVENT_TIMEUPDATE => {
                println!("MF_MEDIA_ENGINE_EVENT_TIMEUPDATE");
            }
            MF_MEDIA_ENGINE_EVENT_ERROR => {
                println!("MF_MEDIA_ENGINE_EVENT_ERROR");
/*
                #ifdef _DEBUG
                if (m_mediaEngine)
                {
                    ComPtr<IMFMediaError> error;
                    if (SUCCEEDED(m_mediaEngine->GetError(&error)))
                    {
                        USHORT errorCode = error->GetErrorCode();
                        HRESULT hr = error->GetExtendedErrorCode();
                        char buff[128] = {};
                        sprintf_s(buff, "ERROR: Media Foundation Event Error %u (%08X)\n", errorCode, static_cast<unsigned int>(hr));
                        OutputDebugStringA(buff);
                    }
                    else
                    {
                        OutputDebugStringA("ERROR: Media Foundation Event Error *FAILED GetError*\n");
                    }
                }
                #endif
                break;
*/
            }
            _ => ()
        }
    }
}

#[implement(IMFMediaEngineNotify)]
struct MediaEngineNotify {
    pub notify: *mut NotifyEvents
}

#[allow(non_snake_case)]
impl IMFMediaEngineNotify_Impl for MediaEngineNotify {
    fn EventNotify(&self, event: u32, _param1: usize, _param2: u32) -> ::windows::core::Result<()>  {
        unsafe {
            (*self.notify).event(event);
        }
        Ok(())
    }
}

fn new_notify_events() -> *mut NotifyEvents {
    unsafe {
        let layout =
            std::alloc::Layout::from_size_align(std::mem::size_of::<NotifyEvents>(), 8).unwrap();
        std::alloc::alloc_zeroed(layout) as *mut NotifyEvents
    }
}

impl super::VideoPlayer<d3d12::Device> for VideoPlayer {
    fn create(device: &d3d12::Device) -> result::Result<VideoPlayer, super::Error> {
        let factory = d3d12::get_dxgi_factory(&device);
        let (adapter, _) = d3d12::get_hardware_adapter(factory, &Some(device.get_adapter_info().name.to_string())).unwrap();
        unsafe {
            MFStartup(MF_SDK_VERSION << 16 | MF_API_VERSION, 0)?;
            CoInitialize(std::ptr::null_mut())?;

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
            )?;
            let device = device.unwrap();

            // make thread safe
            let mt : ID3D11Multithread = device.cast().unwrap();
            mt.SetMultithreadProtected(BOOL::from(true));

            // setup media engine
            let mut reset_token : u32 = 0;
            let mut dxgi_manager : Option<IMFDXGIDeviceManager> = None;
            MFCreateDXGIDeviceManager(&mut reset_token, &mut dxgi_manager)?;

            // create attributes
            let mut attributes : Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1)?;

            if let Some(attributes) = &attributes {
                if let Some(dxgi_manager) = &dxgi_manager {
                    let idevice : IUnknown = device.cast()?;
                    dxgi_manager.ResetDevice(idevice, reset_token)?;
                    let idxgi_manager : IUnknown = dxgi_manager.cast()?;
                    attributes.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, idxgi_manager)?;
                }

                attributes.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0)?;

                // create event callback
                let notify = new_notify_events();
                let mn = MediaEngineNotify {
                    notify: notify
                };
                let imn : IMFMediaEngineNotify = mn.into();
                attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, imn)?;
    
                // create media engine
                let mf_factory : IMFMediaEngineClassFactory = 
                    CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_ALL)?;
                let media_engine = mf_factory.CreateInstance(0, attributes)?;
                
                let player = VideoPlayer {
                    device: device,
                    media_engine_ex: media_engine.cast()?,
                    notify: notify
                };

                return Ok(player);
            }

            Err(super::Error {
                error_type: super::ErrorType::InitFailed,
                msg: String::from("hotline::av::wmf failed to initialised, could not create attributes"),
            })
        }
    }

    fn set_source(&self, filepath: String) -> result::Result<(), super::Error> {
        unsafe {
            let mb = win32::string_to_multibyte(filepath);
            let bstr = SysAllocString(PCWSTR(mb.as_ptr() as _));
            self.media_engine_ex.SetSource(bstr)?;
            Ok(())
        }
    }

    fn play(&self) {
        unsafe {
            self.media_engine_ex.Play();
        }
    }

    fn transfer_frame(&self) {
        unsafe {
            let pts = self.media_engine_ex.OnVideoStreamTick().unwrap();
            println!("tick {}", pts);
        }
    }

    fn is_loaded(&self) -> bool {
        unsafe {
            (*self.notify).can_play
        }
    }

    fn is_playing(&self) -> bool {
        unsafe {
            (*self.notify).playing
        }
    }

    fn is_ended(&self) -> bool {
        unsafe {
            (*self.notify).ended
        }
    }
}

impl From<windows::core::Error> for super::Error {
    fn from(err: windows::core::Error) -> super::Error {
        super::Error {
            error_type: super::ErrorType::WindowsMediaFoundation,
            msg: err.message().to_string_lossy(),
        }
    }
}