#![cfg(target_os = "macos")]

use crate::gfx;

use gfx::mtl;
use gfx::Device;

use crate::os;
use std::result;

use cocoa::base::{id, nil, YES};
use cocoa::foundation::{NSString, NSUInteger};
use objc::{class, msg_send, sel, sel_impl};
use metal::foreign_types::ForeignTypeRef;

// High level AVPlayer playback, frames are pulled from an AVPlayerItemVideoOutput as BGRA CVPixelBuffers,
// wrapped zero copy as metal textures via a CVMetalTextureCache and then blitted into a hotline texture.

type CFTypeRef = *const std::ffi::c_void;
type CVReturn = i32;
type CVPixelBufferRef = CFTypeRef;
type CVMetalTextureRef = CFTypeRef;
type CVMetalTextureCacheRef = CFTypeRef;

/// kCVPixelFormatType_32BGRA ('BGRA')
const K_CV_PIXEL_FORMAT_TYPE_32_BGRA: u32 = 0x42475241;

/// AVPlayerItemStatus
const AV_PLAYER_ITEM_STATUS_READY_TO_PLAY: isize = 1;
const AV_PLAYER_ITEM_STATUS_FAILED: isize = 2;

/// AVPlayerActionAtItemEnd
const AV_PLAYER_ACTION_AT_ITEM_END_PAUSE: isize = 1;

#[repr(C)]
#[derive(Copy, Clone)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64
}

#[repr(C)]
#[derive(Copy, Clone)]
struct CGSize {
    width: f64,
    height: f64
}

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMTimeGetSeconds(time: CMTime) -> f64;
    static kCMTimeZero: CMTime;
}

#[link(name = "QuartzCore", kind = "framework")]
extern "C" {
    fn CACurrentMediaTime() -> f64;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreVideo", kind = "framework")]
extern "C" {
    static kCVPixelBufferPixelFormatTypeKey: id;
    static kCVPixelBufferMetalCompatibilityKey: id;
    fn CVPixelBufferGetWidth(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetHeight(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVMetalTextureCacheCreate(
        allocator: CFTypeRef,
        cache_attributes: CFTypeRef,
        metal_device: id,
        texture_attributes: CFTypeRef,
        cache_out: *mut CVMetalTextureCacheRef
    ) -> CVReturn;
    fn CVMetalTextureCacheCreateTextureFromImage(
        allocator: CFTypeRef,
        texture_cache: CVMetalTextureCacheRef,
        source_image: CVPixelBufferRef,
        texture_attributes: CFTypeRef,
        pixel_format: NSUInteger,
        width: usize,
        height: usize,
        plane_index: usize,
        texture_out: *mut CVMetalTextureRef
    ) -> CVReturn;
    fn CVMetalTextureCacheFlush(texture_cache: CVMetalTextureCacheRef, options: u64);
    fn CVMetalTextureGetTexture(image: CVMetalTextureRef) -> id;
}

/// A decoded frame being blitted on the gpu, the cv resources are kept alive until the copy has completed
struct PendingFrame {
    cmd: metal::CommandBuffer,
    pixel_buffer: CVPixelBufferRef,
    cv_texture: CVMetalTextureRef
}

impl PendingFrame {
    fn is_complete(&self) -> bool {
        matches!(self.cmd.status(),
            metal::MTLCommandBufferStatus::Completed | metal::MTLCommandBufferStatus::Error)
    }

    fn release(self) {
        unsafe {
            CFRelease(self.cv_texture);
            CFRelease(self.pixel_buffer);
        }
    }
}

pub struct VideoPlayer {
    command_queue: metal::CommandQueue,
    texture_cache: CVMetalTextureCacheRef,
    player: id,
    item: id,
    output: id,
    pending: Vec<PendingFrame>,
    texture: Option<mtl::Texture>,
    width: u32,
    height: u32
}

fn nsstring_to_string(ns: id) -> String {
    unsafe {
        if ns == nil {
            return String::new();
        }
        let utf8: *const std::os::raw::c_char = msg_send![ns, UTF8String];
        std::ffi::CStr::from_ptr(utf8).to_string_lossy().into_owned()
    }
}

impl VideoPlayer {
    fn handle_error(&self) -> result::Result<(), super::Error> {
        unsafe {
            if self.item == nil {
                return Ok(());
            }
            let status: isize = msg_send![self.item, status];
            if status == AV_PLAYER_ITEM_STATUS_FAILED {
                let err: id = msg_send![self.item, error];
                let desc = if err != nil {
                    let desc: id = msg_send![err, localizedDescription];
                    nsstring_to_string(desc)
                }
                else {
                    "unknown error".to_string()
                };
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: {}", desc)
                });
            }
            Ok(())
        }
    }

    /// Releases the current item and video output
    fn release_item(&mut self) {
        unsafe {
            if self.item != nil {
                if self.output != nil {
                    let _: () = msg_send![self.item, removeOutput: self.output];
                    let _: () = msg_send![self.output, release];
                    self.output = nil;
                }
                let _: () = msg_send![self.item, release];
                self.item = nil;
            }
        }
    }

    /// Release cv resources for frames whose blit has finished on the gpu
    fn release_completed_frames(&mut self) {
        let (complete, pending): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|f| f.is_complete());
        self.pending = pending;
        for frame in complete {
            frame.release();
        }
    }

    fn current_seconds(&self) -> Option<(f64, f64)> {
        unsafe {
            if self.item == nil {
                return None;
            }
            let current: CMTime = msg_send![self.item, currentTime];
            let duration: CMTime = msg_send![self.item, duration];
            let current = CMTimeGetSeconds(current);
            let duration = CMTimeGetSeconds(duration);
            if current.is_finite() && duration.is_finite() {
                Some((current, duration))
            }
            else {
                None
            }
        }
    }
}

impl super::VideoPlayer<mtl::Device> for VideoPlayer {
    fn create(device: &mtl::Device) -> result::Result<VideoPlayer, super::Error> {
        objc::rc::autoreleasepool(|| unsafe {
            let mut texture_cache: CVMetalTextureCacheRef = std::ptr::null();
            let metal_device = device.get_metal_device().as_ptr() as id;
            let res = CVMetalTextureCacheCreate(
                std::ptr::null(), std::ptr::null(), metal_device, std::ptr::null(), &mut texture_cache);
            if res != 0 || texture_cache.is_null() {
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: failed to create CVMetalTextureCache ({})", res)
                });
            }

            let player: id = msg_send![class!(AVPlayer), alloc];
            let player: id = msg_send![player, init];
            let _: () = msg_send![player, setActionAtItemEnd: AV_PLAYER_ACTION_AT_ITEM_END_PAUSE];

            Ok(VideoPlayer {
                command_queue: device.get_command_queue().clone(),
                texture_cache,
                player,
                item: nil,
                output: nil,
                pending: Vec::new(),
                texture: None,
                width: 0,
                height: 0
            })
        })
    }

    fn set_source(&mut self, filepath: String) -> result::Result<(), super::Error> {
        objc::rc::autoreleasepool(|| unsafe {
            self.release_item();

            let path = NSString::alloc(nil).init_str(&filepath);
            let url: id = msg_send![class!(NSURL), fileURLWithPath: path];
            let _: () = msg_send![path, release];

            // request bgra frames that can be wrapped as metal textures
            let format: id = msg_send![class!(NSNumber), numberWithUnsignedInt: K_CV_PIXEL_FORMAT_TYPE_32_BGRA];
            let metal_compat: id = msg_send![class!(NSNumber), numberWithBool: YES];
            let keys = [kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferMetalCompatibilityKey];
            let values = [format, metal_compat];
            let attributes: id = msg_send![class!(NSDictionary),
                dictionaryWithObjects: values.as_ptr()
                forKeys: keys.as_ptr()
                count: keys.len() as NSUInteger];

            let output: id = msg_send![class!(AVPlayerItemVideoOutput), alloc];
            let output: id = msg_send![output, initWithPixelBufferAttributes: attributes];

            let item: id = msg_send![class!(AVPlayerItem), playerItemWithURL: url];
            let item: id = msg_send![item, retain];
            let _: () = msg_send![item, addOutput: output];
            let _: () = msg_send![self.player, replaceCurrentItemWithPlayerItem: item];

            self.item = item;
            self.output = output;
            Ok(())
        })
    }

    fn play(&self) -> result::Result<(), super::Error> {
        unsafe {
            // restart from the beginning if the video has already ended
            if self.is_ended() {
                let _: () = msg_send![self.player, seekToTime: kCMTimeZero];
            }
            let _: () = msg_send![self.player, play];
        }
        Ok(())
    }

    fn pause(&self) -> result::Result<(), super::Error> {
        unsafe {
            let _: () = msg_send![self.player, pause];
        }
        Ok(())
    }

    fn update(&mut self, device: &mut mtl::Device) -> result::Result<(), super::Error> {
        self.release_completed_frames();

        // handle errors and return early
        self.handle_error()?;

        // return early if not loaded
        if !self.is_loaded() {
            return Ok(());
        }

        objc::rc::autoreleasepool(|| unsafe {
            let item_time: CMTime = msg_send![self.output, itemTimeForHostTime: CACurrentMediaTime()];
            let has_new: bool = msg_send![self.output, hasNewPixelBufferForItemTime: item_time];
            if !has_new {
                return Ok(());
            }

            let display_time: *mut CMTime = std::ptr::null_mut();
            let pixel_buffer: CVPixelBufferRef =
                msg_send![self.output, copyPixelBufferForItemTime: item_time itemTimeForDisplay: display_time];
            if pixel_buffer.is_null() {
                return Ok(());
            }

            let width = CVPixelBufferGetWidth(pixel_buffer);
            let height = CVPixelBufferGetHeight(pixel_buffer);

            // wrap the pixel buffer as a metal texture
            let mut cv_texture: CVMetalTextureRef = std::ptr::null();
            let res = CVMetalTextureCacheCreateTextureFromImage(
                std::ptr::null(),
                self.texture_cache,
                pixel_buffer,
                std::ptr::null(),
                metal::MTLPixelFormat::BGRA8Unorm as NSUInteger,
                width,
                height,
                0,
                &mut cv_texture
            );
            if res != 0 || cv_texture.is_null() {
                CFRelease(pixel_buffer);
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: failed to create metal texture from pixel buffer ({})", res)
                });
            }

            // (re)create the destination texture when the video size changes
            if self.texture.is_none() || self.width != width as u32 || self.height != height as u32 {
                let info = gfx::TextureInfo {
                    tex_type: gfx::TextureType::Texture2D,
                    format: gfx::Format::BGRA8n,
                    width: width as u64,
                    height: height as u64,
                    depth: 1,
                    array_layers: 1,
                    mip_levels: 1,
                    samples: 1,
                    usage: gfx::TextureUsage::VIDEO_DECODE_TARGET | gfx::TextureUsage::SHADER_RESOURCE,
                    initial_state: gfx::ResourceState::ShaderResource
                };
                self.texture = Some(device.create_texture::<u8>(&info, None)?);
                self.width = width as u32;
                self.height = height as u32;
            }

            // copy the frame into the hotline texture
            if let Some(tex) = &self.texture {
                let src = metal::TextureRef::from_ptr(CVMetalTextureGetTexture(cv_texture) as *mut _);
                let cmd = self.command_queue.new_command_buffer().to_owned();
                let blit = cmd.new_blit_command_encoder();
                blit.copy_from_texture(
                    src, 0, 0, metal::MTLOrigin { x: 0, y: 0, z: 0 },
                    metal::MTLSize { width: width as NSUInteger, height: height as NSUInteger, depth: 1 },
                    tex.get_metal_texture(), 0, 0, metal::MTLOrigin { x: 0, y: 0, z: 0 }
                );
                blit.end_encoding();
                cmd.commit();

                self.pending.push(PendingFrame {
                    cmd,
                    pixel_buffer,
                    cv_texture
                });
            }

            CVMetalTextureCacheFlush(self.texture_cache, 0);
            Ok(())
        })
    }

    fn get_size(&self) -> os::Size<u32> {
        if self.texture.is_none() && self.is_loaded() {
            // before the first frame arrives fall back to the item's presentation size
            let size: CGSize = unsafe { msg_send![self.item, presentationSize] };
            return os::Size {
                x: size.width as u32,
                y: size.height as u32
            };
        }
        os::Size {
            x: self.width,
            y: self.height
        }
    }

    fn is_loaded(&self) -> bool {
        if self.item == nil {
            return false;
        }
        let status: isize = unsafe { msg_send![self.item, status] };
        status == AV_PLAYER_ITEM_STATUS_READY_TO_PLAY
    }

    fn is_playing(&self) -> bool {
        if !self.is_loaded() {
            return false;
        }
        let rate: f32 = unsafe { msg_send![self.player, rate] };
        rate != 0.0
    }

    fn is_ended(&self) -> bool {
        if !self.is_loaded() || self.is_playing() {
            return false;
        }
        if let Some((current, duration)) = self.current_seconds() {
            duration > 0.0 && current >= duration
        }
        else {
            false
        }
    }

    fn get_texture(&self) -> &Option<mtl::Texture> {
        &self.texture
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        for frame in self.pending.drain(..) {
            frame.cmd.wait_until_completed();
            frame.release();
        }
        unsafe {
            let _: () = msg_send![self.player, pause];
            let _: () = msg_send![self.player, replaceCurrentItemWithPlayerItem: nil];
            self.release_item();
            let _: () = msg_send![self.player, release];
            CFRelease(self.texture_cache);
        }
    }
}
