#![cfg(target_os = "windows")]

use crate::gfx;

use gfx::d3d12;
use gfx::Device;

use crate::os;
use os::win32;
use std::result;

// Reference C++ implementation
// https://github.com/microsoft/Xbox-ATG-Samples/blob/master/UWPSamples/Graphics/VideoTextureUWP12

use windows::{
    core::*, Win32::Foundation::*,
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Direct3D::*, Win32::Foundation::HINSTANCE,
    Win32::Media::MediaFoundation::*, Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
    Win32::System::Com::CoCreateInstance, Win32::System::Com::CLSCTX_ALL,
};

pub struct VideoPlayer {
    device: ID3D11Device,
    media_engine_ex: IMFMediaEngineEx,
    notify: *mut NotifyEvents,
    texture: Option<d3d12::Texture>,
    width: u32,
    height: u32,
    cleanup_textures: Vec<d3d12::Texture>
}

pub struct NotifyEvents {
    load_meta_data: bool,
    paused: bool,
    can_play: bool,
    playing: bool,
    ended: bool,
    has_error: bool
}

impl NotifyEvents {
    fn event(&mut self, event: u32) {
        match MF_MEDIA_ENGINE_EVENT(event as i32) {
            MF_MEDIA_ENGINE_EVENT_LOADEDMETADATA => {
                self.load_meta_data = true;
            },
            MF_MEDIA_ENGINE_EVENT_CANPLAY => {
                self.can_play = true;
            }
            MF_MEDIA_ENGINE_EVENT_PLAY => {
                self.playing = true;
            }
            MF_MEDIA_ENGINE_EVENT_PAUSE => {
                self.paused = true;
            }
            MF_MEDIA_ENGINE_EVENT_ENDED => {
                self.ended = true;
                self.playing = false;
            }
            MF_MEDIA_ENGINE_EVENT_TIMEUPDATE => {
                
            }
            MF_MEDIA_ENGINE_EVENT_ERROR => {
                self.has_error = true;
            }
            _ => ()
        }
    }
}

#[allow(unused_must_use)]
mod imp {
    use windows::{core::*, Win32::Media::MediaFoundation::*};
    #[implement(IMFMediaEngineNotify)]
    pub struct MediaEngineNotify {
        pub notify: *mut super::NotifyEvents
    }
}
pub use imp::*;

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

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        if self.texture.is_some() || !self.cleanup_textures.is_empty() {
            panic!("hotline_rs::av::wmf: dropping video player with live textures, call shutdown with d3d12::device to free srv heap memory.");
        };
    }
}

impl VideoPlayer {
    fn handle_error(&self) -> result::Result<(), super::Error> {
        unsafe {
            if (*self.notify).has_error {
                // error
                let err = self.media_engine_ex.GetError()?;
                let code = err.GetErrorCode();
                let msgs = [
                    "MF_MEDIA_ENGINE_ERR_NOERROR",
                    "MF_MEDIA_ENGINE_ERR_ABORTED",
                    "MF_MEDIA_ENGINE_ERR_NETWORK",
                    "MF_MEDIA_ENGINE_ERR_DECODE",
                    "MF_MEDIA_ENGINE_ERR_SRC_NOT_SUPPORTED",
                    "MF_MEDIA_ENGINE_ERR_ENCRYPTED",
                ];

                // extended error
                let mut ext_str = "".to_string();
                let ext = err.GetExtendedErrorCode();
                if let Err(e) = &ext {
                    ext_str = e.message().to_string_lossy();
                }

                // error code with extended info
                return Err(super::Error{
                    msg: format!("hotline_rs::av::wmf: {} : {}", msgs[code as usize], ext_str)
                });
            }
            Ok(())
        }
    }

    fn cleanup_textures(&mut self, device: &mut d3d12::Device) {
        // remove elements from the cleanup_textures array
        while !self.cleanup_textures.is_empty() {
            let tex = self.cleanup_textures.remove(0);
            device.destroy_texture(tex);
        }
    }
}

impl super::VideoPlayer<d3d12::Device> for VideoPlayer {
    fn create(device: &d3d12::Device) -> result::Result<VideoPlayer, super::Error> {
        let factory = d3d12::get_dxgi_factory(device);
        let (adapter, _) = d3d12::get_hardware_adapter(factory, &Some(device.get_adapter_info().name.to_string())).unwrap();
        unsafe {
            MFStartup(MF_SDK_VERSION << 16 | MF_API_VERSION, 0)?;

            // create device
            let mut device : Option<ID3D11Device> = None;
            D3D11CreateDevice(
                &adapter, 
                D3D_DRIVER_TYPE_UNKNOWN,
                HINSTANCE(0), 
                D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                None,
            )?;
            let device = device.unwrap();

            // make thread safe
            let mt : ID3D11Multithread = device.cast()?;
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
                    dxgi_manager.ResetDevice(&idevice, reset_token)?;
                    let idxgi_manager : IUnknown = dxgi_manager.cast()?;
                    attributes.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, &idxgi_manager)?;
                }

                attributes.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0)?;

                // create event callback
                let notify = new_notify_events();
                let mn = MediaEngineNotify {
                    notify
                };
                let imn : IMFMediaEngineNotify = mn.into();
                attributes.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &imn)?;
    
                // create media engine
                let mf_factory : IMFMediaEngineClassFactory = 
                    CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_ALL)?;
                let media_engine = mf_factory.CreateInstance(0, attributes)?;
                
                let player = VideoPlayer {
                    device,
                    media_engine_ex: media_engine.cast()?,
                    notify,
                    texture: None,
                    cleanup_textures: vec![],
                    width: 0,
                    height: 0
                };

                return Ok(player);
            }

            Err(super::Error {
                msg: String::from("hotline_rs::av::wmf:: failed to initialise, could not create attributes"),
            })
        }
    }

    fn shutdown(&mut self, device: &mut d3d12::Device) {
        // clean up any old lingering textures
        self.cleanup_textures(device);
        // take ownership of the player texture
        let mut none_tex = None;
        std::mem::swap(&mut none_tex, &mut self.texture);
        if let Some(tex) = none_tex {
            device.destroy_texture(tex);
        }
    }

    fn set_source(&mut self, filepath: String) -> result::Result<(), super::Error> {
        unsafe {
            // set texture to none and push the old tex for cleanup
            let mut none_tex = None;
            std::mem::swap(&mut none_tex, &mut self.texture);
            if let Some(tex) = none_tex {
                self.cleanup_textures.push(tex);
            }

            // reset state
            (*self.notify).can_play = false;
            (*self.notify).ended = false;
            (*self.notify).playing = false;

            let wp = win32::string_to_wide(filepath);
            let bstr = SysAllocString(PCWSTR(wp.as_ptr() as _));
            self.media_engine_ex.SetSource(&bstr)?;
            Ok(())
        }
    }

    fn play(&self) -> result::Result<(), super::Error> {
        unsafe {
            self.media_engine_ex.Play()?;
        }
        Ok(())
    }

    fn pause(&self) -> result::Result<(), super::Error> {
        unsafe {
            self.media_engine_ex.Pause()?;
        }
        Ok(())
    }

    fn update(&mut self, device: &mut d3d12::Device) -> result::Result<(), super::Error> {
        unsafe {
            // handle errors and return early
            self.handle_error()?;

            // return early if not loaded
            if !self.is_loaded() {
                return Ok(());
            }

            // clean up old textures that have now been dropped, requires a device
            self.cleanup_textures(device);

            // create texture
            if self.texture.is_none() && self.is_loaded() {
                let mut x : u32 = 0;
                let mut y : u32 = 0;
                self.media_engine_ex.GetNativeVideoSize(Some(&mut x), Some(&mut y))?;

                let info = gfx::TextureInfo {
                    tex_type: gfx::TextureType::Texture2D,
                    format: gfx::Format::BGRA8n,
                    width: x as u64,
                    height: y as u64,
                    depth: 1,
                    array_layers: 1,
                    mip_levels: 1,
                    samples: 1,
                    usage: gfx::TextureUsage::VIDEO_DECODE_TARGET | gfx::TextureUsage::SHADER_RESOURCE,
                    initial_state: gfx::ResourceState::ShaderResource
                };

                self.texture = Some(device.create_texture::<u8>(&info, None)?);
                self.width = x;
                self.height = y;
            }

            // update
            if self.is_loaded() {
                let pts = self.media_engine_ex.OnVideoStreamTick();
                if pts.is_ok() {
                    if let Some(tex) = &self.texture {
                        let sh = d3d12::get_texture_shared_handle(tex);
                        if let Some(handle) = sh {
                            let dev1 : ID3D11Device1 = self.device.cast()?;
                            let media_texture : ID3D11Texture2D = dev1.OpenSharedResource1(*handle)?;
    
                            let mf_rect = MFVideoNormalizedRect {
                                left: 0.0,
                                top: 0.0,
                                right: 1.0,
                                bottom: 1.0
                            };
    
                            let rect = RECT {
                                left: 0,
                                top: 0,
                                right: self.width as i32,
                                bottom: self.height as i32
                            };
    
                            self.media_engine_ex.TransferVideoFrame(
                                &media_texture, Some(&mf_rect), &rect, None)?;
                        } 
                    }
                }
            }

            Ok(())
        }
    }

    fn get_size(&self) -> os::Size<u32> {
        os::Size {
            x: self.width,
            y: self.height
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

    fn get_texture(&self) -> &Option<d3d12::Texture> {
        &self.texture
    }
}