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
//
// Recording writes frames read back to the cpu into BGRA CVPixelBuffers from an AVAssetWriterInputPixelBufferAdaptor
// pool, which an AVAssetWriter encodes to h264.

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

/// AVAssetWriterStatus
const AV_ASSET_WRITER_STATUS_WRITING: isize = 1;
const AV_ASSET_WRITER_STATUS_FAILED: isize = 3;

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {
    static AVMediaTypeVideo: id;
    static AVFileTypeMPEG4: id;
    static AVFileTypeQuickTimeMovie: id;
    static AVVideoCodecKey: id;
    static AVVideoCodecTypeH264: id;
    static AVVideoWidthKey: id;
    static AVVideoHeightKey: id;
    static AVVideoCompressionPropertiesKey: id;
    static AVVideoAverageBitRateKey: id;
    static AVVideoExpectedSourceFrameRateKey: id;
}

#[link(name = "CoreMedia", kind = "framework")]
extern "C" {
    fn CMTimeGetSeconds(time: CMTime) -> f64;
    fn CMTimeMakeWithSeconds(seconds: f64, preferred_timescale: i32) -> CMTime;
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
    static kCVPixelBufferWidthKey: id;
    static kCVPixelBufferHeightKey: id;
    fn CVPixelBufferGetWidth(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetBytesPerRow(pixel_buffer: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetBaseAddress(pixel_buffer: CVPixelBufferRef) -> *mut std::ffi::c_void;
    fn CVPixelBufferLockBaseAddress(pixel_buffer: CVPixelBufferRef, flags: u64) -> CVReturn;
    fn CVPixelBufferUnlockBaseAddress(pixel_buffer: CVPixelBufferRef, flags: u64) -> CVReturn;
    fn CVPixelBufferPoolCreatePixelBuffer(
        allocator: CFTypeRef,
        pixel_buffer_pool: CFTypeRef,
        pixel_buffer_out: *mut CVPixelBufferRef
    ) -> CVReturn;
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

/// Creates an autoreleased NSDictionary from parallel key / value arrays
unsafe fn ns_dictionary(keys: &[id], values: &[id]) -> id {
    msg_send![class!(NSDictionary),
        dictionaryWithObjects: values.as_ptr()
        forKeys: keys.as_ptr()
        count: keys.len() as NSUInteger]
}

/// Creates an autoreleased NSNumber
unsafe fn ns_number(value: f64) -> id {
    msg_send![class!(NSNumber), numberWithDouble: value]
}

/// Returns the localized description of an NSError or "unknown error" if `err` is nil
fn error_description(err: id) -> String {
    if err == nil {
        return "unknown error".to_string();
    }
    unsafe {
        let desc: id = msg_send![err, localizedDescription];
        nsstring_to_string(desc)
    }
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
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: {}", error_description(err))
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
            let attributes = ns_dictionary(
                &[kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferMetalCompatibilityKey],
                &[format, metal_compat]
            );

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

pub struct VideoRecorder {
    writer: id,
    input: id,
    adaptor: id,
    width: u32,
    height: u32
}

impl VideoRecorder {
    fn writer_error(&self) -> super::Error {
        let err: id = unsafe { msg_send![self.writer, error] };
        super::Error {
            msg: format!("hotline_rs::av::avf: video recorder: {}", error_description(err))
        }
    }

    fn writer_status(&self) -> isize {
        unsafe { msg_send![self.writer, status] }
    }

    /// Flushes the encoder and closes the file, called once from finish or drop
    fn finalise(&mut self) -> result::Result<(), super::Error> {
        if self.writer == nil {
            return Ok(());
        }

        let result = if self.writer_status() == AV_ASSET_WRITER_STATUS_WRITING {
            unsafe {
                let _: () = msg_send![self.input, markAsFinished];
                let handler = block::ConcreteBlock::new(|| {}).copy();
                let _: () = msg_send![self.writer, finishWritingWithCompletionHandler: &*handler];
            }

            // completion is signalled on a background queue, wait for the status to change
            let timeout = std::time::Instant::now();
            while self.writer_status() == AV_ASSET_WRITER_STATUS_WRITING && timeout.elapsed().as_secs() < 10 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            match self.writer_status() {
                AV_ASSET_WRITER_STATUS_FAILED => Err(self.writer_error()),
                AV_ASSET_WRITER_STATUS_WRITING => Err(super::Error {
                    msg: "hotline_rs::av::avf: timed out finishing video".to_string()
                }),
                _ => Ok(())
            }
        }
        else if self.writer_status() == AV_ASSET_WRITER_STATUS_FAILED {
            Err(self.writer_error())
        }
        else {
            Ok(())
        };

        unsafe {
            for obj in [&mut self.adaptor, &mut self.input, &mut self.writer] {
                if *obj != nil {
                    let _: () = msg_send![*obj, release];
                    *obj = nil;
                }
            }
        }
        result
    }
}

impl super::VideoRecorder for VideoRecorder {
    fn create(filepath: &str, width: u32, height: u32, fps: u32) -> result::Result<VideoRecorder, super::Error> {
        let (width, height) = (width & !1, height & !1);
        objc::rc::autoreleasepool(|| unsafe {
            // the asset writer fails if the file already exists
            let _ = std::fs::remove_file(filepath);

            let path = NSString::alloc(nil).init_str(filepath);
            let url: id = msg_send![class!(NSURL), fileURLWithPath: path];
            let _: () = msg_send![path, release];

            let file_type = if filepath.to_lowercase().ends_with(".mov") {
                AVFileTypeQuickTimeMovie
            }
            else {
                AVFileTypeMPEG4
            };

            let mut err: id = nil;
            let writer: id = msg_send![class!(AVAssetWriter), alloc];
            let writer: id = msg_send![writer, initWithURL: url fileType: file_type error: &mut err];
            if writer == nil {
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: failed to create video {}: {}", filepath, error_description(err))
                });
            }

            // the recorder owns the writer from here so it gets released on any early return
            let mut recorder = VideoRecorder {
                writer,
                input: nil,
                adaptor: nil,
                width,
                height
            };

            // encoded h264 output, roughly 0.1 bits per pixel per frame
            let compression = ns_dictionary(
                &[AVVideoAverageBitRateKey, AVVideoExpectedSourceFrameRateKey],
                &[ns_number(width as f64 * height as f64 * fps as f64 * 0.1), ns_number(fps as f64)]
            );
            let output_settings = ns_dictionary(
                &[AVVideoCodecKey, AVVideoWidthKey, AVVideoHeightKey, AVVideoCompressionPropertiesKey],
                &[AVVideoCodecTypeH264, ns_number(width as f64), ns_number(height as f64), compression]
            );
            let input: id = msg_send![class!(AVAssetWriterInput),
                assetWriterInputWithMediaType: AVMediaTypeVideo outputSettings: output_settings];
            recorder.input = msg_send![input, retain];

            // offline so frames are never dropped, write_frame waits for the encoder instead
            let _: () = msg_send![input, setExpectsMediaDataInRealTime: cocoa::base::NO];

            // raw bgra input
            let format: id = msg_send![class!(NSNumber), numberWithUnsignedInt: K_CV_PIXEL_FORMAT_TYPE_32_BGRA];
            let source_attributes = ns_dictionary(
                &[kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey, kCVPixelBufferHeightKey],
                &[format, ns_number(width as f64), ns_number(height as f64)]
            );
            let adaptor: id = msg_send![class!(AVAssetWriterInputPixelBufferAdaptor),
                assetWriterInputPixelBufferAdaptorWithAssetWriterInput: input
                sourcePixelBufferAttributes: source_attributes];
            recorder.adaptor = msg_send![adaptor, retain];

            let can_add: bool = msg_send![writer, canAddInput: input];
            if !can_add {
                return Err(recorder.writer_error());
            }
            let _: () = msg_send![writer, addInput: input];

            let started: bool = msg_send![writer, startWriting];
            if !started {
                return Err(recorder.writer_error());
            }
            let _: () = msg_send![writer, startSessionAtSourceTime: kCMTimeZero];

            Ok(recorder)
        })
    }

    fn write_frame(&mut self, frame: &gfx::ReadBackData, time: f64, _duration: f64) -> result::Result<(), super::Error> {
        let row_bytes = self.width as usize * 4;
        if frame.row_pitch < row_bytes || frame.data.len() < frame.row_pitch * self.height as usize {
            return Err(super::Error {
                msg: "hotline_rs::av::avf: frame is smaller than the recorder".to_string()
            });
        }

        if self.writer_status() != AV_ASSET_WRITER_STATUS_WRITING {
            return Err(self.writer_error());
        }

        objc::rc::autoreleasepool(|| unsafe {
            let pool: CFTypeRef = msg_send![self.adaptor, pixelBufferPool];
            if pool.is_null() {
                return Err(self.writer_error());
            }

            let mut pixel_buffer: CVPixelBufferRef = std::ptr::null();
            let res = CVPixelBufferPoolCreatePixelBuffer(std::ptr::null(), pool, &mut pixel_buffer);
            if res != 0 || pixel_buffer.is_null() {
                return Err(super::Error {
                    msg: format!("hotline_rs::av::avf: failed to create pixel buffer ({})", res)
                });
            }

            // crops the gpu row padding and any odd column / row
            CVPixelBufferLockBaseAddress(pixel_buffer, 0);
            let dst = CVPixelBufferGetBaseAddress(pixel_buffer) as *mut u8;
            let dst_pitch = CVPixelBufferGetBytesPerRow(pixel_buffer);
            for y in 0..self.height as usize {
                std::ptr::copy_nonoverlapping(
                    frame.data.as_ptr().add(y * frame.row_pitch),
                    dst.add(y * dst_pitch),
                    row_bytes
                );
            }
            CVPixelBufferUnlockBaseAddress(pixel_buffer, 0);

            // wait for the encoder to accept more frames
            let timeout = std::time::Instant::now();
            loop {
                let ready: bool = msg_send![self.input, isReadyForMoreMediaData];
                if ready || self.writer_status() != AV_ASSET_WRITER_STATUS_WRITING || timeout.elapsed().as_secs() >= 2 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_micros(500));
            }

            let appended: bool = msg_send![self.adaptor,
                appendPixelBuffer: pixel_buffer withPresentationTime: CMTimeMakeWithSeconds(time, 1_000_000)];
            CFRelease(pixel_buffer);
            if !appended {
                return Err(self.writer_error());
            }
            Ok(())
        })
    }

    fn get_size(&self) -> os::Size<u32> {
        os::Size {
            x: self.width,
            y: self.height
        }
    }

    fn finish(mut self) -> result::Result<(), super::Error> {
        self.finalise()
    }
}

impl Drop for VideoRecorder {
    fn drop(&mut self) {
        let _ = self.finalise();
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
