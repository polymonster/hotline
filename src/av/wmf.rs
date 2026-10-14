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
    height: u32
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
    use windows::Win32::Media::MediaFoundation::*;

    #[windows_core::implement(IMFMediaEngineNotify)]
    pub struct MediaEngineNotify {
        pub notify: *mut super::NotifyEvents
    }

    #[allow(non_snake_case)]
    impl IMFMediaEngineNotify_Impl for MediaEngineNotify_Impl {
        fn EventNotify(&self, event: u32, _param1: usize, _param2: u32) -> ::windows::core::Result<()>  {
            unsafe {
                (*self.notify).event(event);
            }
            Ok(())
        }
    }
}
pub use imp::*;



fn new_notify_events() -> *mut NotifyEvents {
    unsafe {
        let layout =
            std::alloc::Layout::from_size_align(std::mem::size_of::<NotifyEvents>(), 8).unwrap();
        std::alloc::alloc_zeroed(layout) as *mut NotifyEvents
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
                    ext_str = e.message().to_string()
                }

                // error code with extended info
                return Err(super::Error{
                    msg: format!("hotline_rs::av::wmf: {} : {}", msgs[code as usize], ext_str)
                });
            }
            Ok(())
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
                HINSTANCE(std::ptr::null_mut() as *mut _), 
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
            let _ = mt.SetMultithreadProtected(BOOL::from(true));

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

                attributes.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32)?;

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


    fn set_source(&mut self, filepath: String) -> result::Result<(), super::Error> {
        unsafe {
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

/// H.264 .mp4 recorder using the media foundation sink writer, which inserts the BGRX to NV12 colour conversion and a
/// hardware encoder when available
pub struct VideoRecorder {
    writer: Option<IMFSinkWriter>,
    stream: u32,
    width: u32,
    height: u32
}

/// Media foundation times are in 100ns units
fn to_mf_time(seconds: f64) -> i64 {
    (seconds * 10_000_000.0) as i64
}

impl super::VideoRecorder for VideoRecorder {
    fn create(filepath: &str, width: u32, height: u32, fps: u32) -> result::Result<VideoRecorder, super::Error> {
        let (width, height) = (width & !1, height & !1);
        unsafe {
            // the sink writer needs com, this fails harmlessly when the thread is already initialised
            let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
            MFStartup(MF_SDK_VERSION << 16 | MF_API_VERSION, 0)?;

            let mut attributes : Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attributes, 1)?;
            let attributes = attributes.unwrap();
            attributes.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;
            let writer = MFCreateSinkWriterFromURL(&HSTRING::from(filepath), None, &attributes)?;

            let frame_size = (width as u64) << 32 | height as u64;
            let frame_rate = (fps as u64) << 32 | 1;
            let set_video_type = |subtype: &GUID| -> result::Result<IMFMediaType, windows::core::Error> {
                let media_type = MFCreateMediaType()?;
                media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
                media_type.SetGUID(&MF_MT_SUBTYPE, subtype)?;
                media_type.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
                media_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size)?;
                media_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate)?;
                media_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, 1 << 32 | 1)?;
                Ok(media_type)
            };

            // encoded output, roughly 0.1 bits per pixel per frame
            let output = set_video_type(&MFVideoFormat_H264)?;
            output.SetUINT32(&MF_MT_AVG_BITRATE, (width as f64 * height as f64 * fps as f64 * 0.1) as u32)?;
            let stream = writer.AddStream(&output)?;

            // raw input, a positive stride is top down rows
            let input = set_video_type(&MFVideoFormat_RGB32)?;
            input.SetUINT32(&MF_MT_DEFAULT_STRIDE, width * 4)?;
            writer.SetInputMediaType(stream, &input, None)?;

            writer.BeginWriting()?;
            Ok(VideoRecorder {
                writer: Some(writer),
                stream,
                width,
                height
            })
        }
    }

    fn write_frame(&mut self, frame: &gfx::ReadBackData, time: f64, duration: f64) -> result::Result<(), super::Error> {
        let row_bytes = self.width * 4;
        if frame.row_pitch < row_bytes as usize || frame.data.len() < frame.row_pitch * self.height as usize {
            return Err(super::Error {
                msg: "hotline_rs::av::wmf: frame is smaller than the recorder".to_string()
            });
        }
        unsafe {
            let size = row_bytes * self.height;
            let buffer = MFCreateMemoryBuffer(size)?;
            let mut dst = std::ptr::null_mut();
            buffer.Lock(&mut dst, None, None)?;
            // crops the gpu row padding and any odd column / row
            let copied = MFCopyImage(dst, row_bytes as i32, frame.data.as_ptr(), frame.row_pitch as i32, row_bytes, self.height);
            buffer.Unlock()?;
            copied?;
            buffer.SetCurrentLength(size)?;

            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(to_mf_time(time))?;
            sample.SetSampleDuration(to_mf_time(duration))?;
            self.writer.as_ref().unwrap().WriteSample(self.stream, &sample)?;
        }
        Ok(())
    }

    fn get_size(&self) -> os::Size<u32> {
        os::Size {
            x: self.width,
            y: self.height
        }
    }

    fn finish(mut self) -> result::Result<(), super::Error> {
        if let Some(writer) = self.writer.take() {
            unsafe {
                writer.Finalize()?;
            }
        }
        Ok(())
    }
}

impl Drop for VideoRecorder {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            unsafe {
                let _ = writer.Finalize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::av::VideoRecorder as _;

    /// Encodes a moving gradient to target/test_output/video_recorder.mp4. Rows are padded like gpu read backs and the
    /// height is odd, which the recorder crops
    #[test]
    fn video_recorder() -> result::Result<(), crate::Error> {
        let (width, height, row_pitch) = (320, 241, 1536);
        let output_dir = "target/test_output";
        std::fs::create_dir_all(output_dir).unwrap();
        let filepath = format!("{}/video_recorder.mp4", output_dir);

        let mut recorder = VideoRecorder::create(&filepath, width, height, 60)?;
        assert_eq!((recorder.get_size().x, recorder.get_size().y), (320, 240));

        for frame in 0..60 {
            // bgrx: red ramps across, green ramps down, blue pulses over time, padding is white
            let mut data = vec![255u8; row_pitch * height as usize];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let i = y * row_pitch + x * 4;
                    data[i] = (frame * 4) as u8;
                    data[i + 1] = (y * 255 / height as usize) as u8;
                    data[i + 2] = (x * 255 / width as usize) as u8;
                }
            }
            let data: &'static [u8] = Box::leak(data.into_boxed_slice());
            recorder.write_frame(&gfx::ReadBackData {
                data,
                format: gfx::Format::BGRX8n,
                size: data.len(),
                row_pitch,
                slice_pitch: data.len()
            }, frame as f64 / 60.0, 1.0 / 60.0)?;
        }
        recorder.finish()?;

        assert!(std::fs::metadata(&filepath).unwrap().len() > 0);
        Ok(())
    }
}
