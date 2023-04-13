use stb_image_rust;
use stb_image_write_rust::ImageWriter::ImageWriter;

use std::fs;
use std::io::Read;

use ddsfile::Caps2;
use ddsfile::D3DFormat;
use ddsfile::DxgiFormat;
use ddsfile::Dds as DDS;

use crate::gfx;
use gfx::{TextureInfo, TextureType, Device};

/// Minimal header to describe image data, with a `TextureInfo` and a `Vec<u8>` of actual image data.
pub struct ImageData {
    /// gfx::TextureInfo describing the texture
    pub info: TextureInfo,
    /// Vector of linear image data tightly packed
    pub data: Vec<u8>,
}

/// Loads an image from file returning information in the ImageData struct
/// supported formats are (png, tga, bmp, jpg, gif, dds)
pub fn load_from_file(filename: &str) -> Result<ImageData, super::Error> {
    // read file
    let path = std::path::Path::new(filename);
    println!("hotline_rs::image:: loading: {}", path.display());
    let mut f = fs::File::open(path).expect("hotline_rs::image:: File not found");
    // dds file
    if filename.ends_with(".dds") {
        let dds = DDS::read(f)?;        
        Ok(ImageData {
            info: TextureInfo {
                tex_type: to_gfx_texture_type(&dds),
                format: to_gfx_format(&dds),
                width: dds.get_width() as u64,
                height: dds.get_height() as u64,
                depth: dds.get_depth(),
                array_layers: dds.get_num_array_layers(),
                mip_levels: dds.get_num_mipmap_levels(),
                samples: 1,
                usage: gfx::TextureUsage::SHADER_RESOURCE,
                initial_state: gfx::ResourceState::ShaderResource
            },
            data: dds.data.to_vec(),
        })
    }
    else {
        // stb image
        let mut contents = vec![];
        f.read_to_end(&mut contents)?;

        let mut x = 0;
        let mut y = 0;
        let mut comp = 0;
        let mut data_out: Vec<u8> = Vec::new();

        unsafe {
            // load image
            let img = stb_image_rust::stbi_load_from_memory(
                contents.as_mut_ptr(),
                contents.len() as i32,
                &mut x,
                &mut y,
                &mut comp,
                stb_image_rust::STBI_rgb_alpha,
            );

            if !img.is_null() {
                // copy data
                let data_size_bytes = x * y * 4;
                data_out.resize(data_size_bytes as usize, 0);
                std::ptr::copy_nonoverlapping(img, data_out.as_mut_ptr(), data_size_bytes as usize);
        
                // cleanup
                stb_image_rust::c_runtime::free(img);

                Ok(ImageData {
                    info: TextureInfo {
                        format: gfx::Format::RGBA8n,
                        width: x as u64,
                        height: y as u64,
                        ..Default::default()
                    },
                    data: data_out,
                })
            }
            else {
                Err(super::Error {
                    msg: format!("hotline_rs::image:: failed to load image via stb_image: {}", filename)
                })
            }
        }
    }
}

/// Loads an image from file and creates a shader resource on the specified heap, or on the device heap if `heap.is_none()`
#[cfg(target_os = "windows")]
pub fn load_texture_from_file(
    device: &mut crate::gfx_platform::Device,
    file: &str,
    heap: Option<&mut crate::gfx_platform::Heap>) -> Result<crate::gfx_platform::Texture, super::Error> {
    let image = load_from_file(file)?;
    device.create_texture_with_heaps(
        &image.info, 
        gfx::TextureHeapInfo {
            shader: heap,
            ..Default::default()
        },
    crate::data![image.data.as_slice()])
}

/// Convert ddsfile to gfx::TextureType
fn to_gfx_texture_type(dds: &DDS) -> TextureType {
    if dds.header.caps.contains(ddsfile::Caps::COMPLEX) {
        let all_faces = Caps2::CUBEMAP_POSITIVEX | Caps2::CUBEMAP_NEGATIVEX | Caps2::CUBEMAP_POSITIVEY |
                        Caps2::CUBEMAP_NEGATIVEY | Caps2::CUBEMAP_POSITIVEZ | Caps2::CUBEMAP_NEGATIVEZ;
        if dds.header.caps2.contains(all_faces) {
            if dds.get_num_array_layers() > 6 {
                TextureType::TextureCubeArray
            }
            else {
                TextureType::TextureCube
            }
        }
        else if dds.get_depth() > 1 {
            TextureType::Texture3D
        }
        else if dds.get_height() == 1 {
            if dds.get_num_array_layers() > 1 {
                TextureType::Texture1DArray
            }
            else {
                TextureType::Texture1D
            }
        }
        else if dds.get_num_array_layers() > 1 {
            TextureType::Texture2DArray
        }
        else {
            TextureType::Texture2D
        }
    }
    else if dds.get_height() == 1 {
        if dds.get_num_array_layers() > 1 {
            TextureType::Texture1DArray
        }
        else {
            TextureType::Texture1D
        }
    }
    else if dds.get_num_array_layers() > 1 {
        TextureType::Texture2DArray
    }
    else {
        TextureType::Texture2D
    }
}

/// Writes a buffer of image data to a file. The type of image format written is determined by filename ext
/// supported image formats are (png, bmp, tga and jpg).
pub fn write_to_file(filename: &str, width: u64, height: u64, components: u32, image_data: &[u8]) -> Result<(), super::Error> {
    let path = std::path::Path::new(&filename);
    let mut writer = ImageWriter::new(filename);
    match path.extension() {
        Some(os_str) => match os_str.to_str() {
            Some("png") => {
                writer.write_png(
                    width as i32,
                    height as i32,
                    components as i32,
                    image_data.as_ptr(),
                );
                Ok(())
            }
            Some("bmp") => {
                writer.write_bmp(
                    width as i32,
                    height as i32,
                    components as i32,
                    image_data.as_ptr(),
                );
                Ok(())
            }
            Some("tga") => {
                writer.write_tga(
                    width as i32,
                    height as i32,
                    components as i32,
                    image_data.as_ptr(),
                );
                Ok(())
            }
            Some("jpg") => {
                writer.write_jpg(
                    width as i32,
                    height as i32,
                    components as i32,
                    image_data.as_ptr(),
                    90,
                );
                Ok(())
            }
            _ => {
                if os_str.to_str().is_some() {
                    Err(super::Error {
                            msg: format!("hotline_rs::image: Image format '{}' is not supported", os_str.to_str().unwrap())
                    })
                } else {
                    Err(super::Error {
                        msg: format!("hotline_rs::image: Filename '{}' did not specify image format extension!",filename)
                    })
                }
            }
        },
        _ => Err(super::Error {
                msg: format!("hotline_rs::image: Filename '{}' has no extension!", filename)
             }),
    }
}

/// Writes an image from file which is formed of data read back from the GPU. This will account for alignment and padding
pub fn write_to_file_from_gpu(filename: &str, data: &gfx::ReadBackData) -> Result<(), super::Error> {
    let fmt = if data.format == gfx::Format::Unknown {
        gfx::Format::RGBA8n
    }
    else {
        data.format
    };
    let w = data.row_pitch / gfx::block_size_for_format(fmt) as usize;
    let h = data.slice_pitch / data.row_pitch;
    let c = gfx::components_for_format(fmt);
    write_to_file(filename, w as u64, h as u64, c, data.data)
}

/// Convert ddsfile format D3D or DXGI to gfx::Format.. gfx does not expose all formats. this may grow over time.
fn to_gfx_format(dds: &DDS) -> gfx::Format {
    if let Some(fmt) = dds.get_d3d_format() {
        match fmt {
            D3DFormat::A8B8G8R8 => gfx::Format::RGBA8n,
            D3DFormat::G16R16 => panic!(),
            D3DFormat::A2B10G10R10 => panic!(),
            D3DFormat::A1R5G5B5 => panic!(),
            D3DFormat::R5G6B5 => panic!(),
            D3DFormat::A8 => panic!(),
            D3DFormat::A8R8G8B8 => panic!(),
            D3DFormat::X8R8G8B8 => panic!(),
            D3DFormat::X8B8G8R8 => panic!(),
            D3DFormat::A2R10G10B10 => panic!(),
            D3DFormat::R8G8B8 => panic!(),
            D3DFormat::X1R5G5B5 => panic!(),
            D3DFormat::A4R4G4B4 => panic!(),
            D3DFormat::X4R4G4B4 => panic!(),
            D3DFormat::A8R3G3B2 => panic!(),
            D3DFormat::A8L8 => panic!(),
            D3DFormat::L16 => panic!(),
            D3DFormat::L8 => panic!(),
            D3DFormat::A4L4 => panic!(),
            D3DFormat::DXT1 => panic!(),
            D3DFormat::DXT3 => panic!(),
            D3DFormat::DXT5 => panic!(),
            D3DFormat::R8G8_B8G8 => panic!(),
            D3DFormat::G8R8_G8B8 => panic!(),
            D3DFormat::A16B16G16R16 => panic!(),
            D3DFormat::Q16W16V16U16 => panic!(),
            D3DFormat::R16F => gfx::Format::R16f,
            D3DFormat::G16R16F => gfx::Format::RG16f,
            D3DFormat::A16B16G16R16F => gfx::Format::RGBA16f,
            D3DFormat::R32F => gfx::Format::R32f,
            D3DFormat::G32R32F => gfx::Format::RG32f,
            D3DFormat::A32B32G32R32F => gfx::Format::RGBA32f,
            D3DFormat::DXT2 => panic!(),
            D3DFormat::DXT4 => panic!(),
            D3DFormat::UYVY => panic!(),
            D3DFormat::YUY2 => panic!(),
            D3DFormat::CXV8U8 => panic!(),
        }
    }
    else if let Some(fmt) = dds.get_dxgi_format() {
        match fmt {
            DxgiFormat::Unknown => gfx::Format::Unknown,
            DxgiFormat::R32G32B32A32_Typeless => panic!(),
            DxgiFormat::R32G32B32A32_Float => gfx::Format::RGBA32f,
            DxgiFormat::R32G32B32A32_UInt => gfx::Format::RGBA32u,
            DxgiFormat::R32G32B32A32_SInt => gfx::Format::RGBA32i,
            DxgiFormat::R32G32B32_Typeless => panic!(),
            DxgiFormat::R32G32B32_Float => gfx::Format::RGB32f,
            DxgiFormat::R32G32B32_UInt => gfx::Format::RGB32u,
            DxgiFormat::R32G32B32_SInt => gfx::Format::RGB32i,
            DxgiFormat::R16G16B16A16_Typeless => panic!(),
            DxgiFormat::R16G16B16A16_Float => gfx::Format::RGBA16f,
            DxgiFormat::R16G16B16A16_UNorm => panic!(),
            DxgiFormat::R16G16B16A16_UInt => gfx::Format::RGBA16u,
            DxgiFormat::R16G16B16A16_SNorm => panic!(),
            DxgiFormat::R16G16B16A16_SInt => gfx::Format::RGBA16i,
            DxgiFormat::R32G32_Typeless => panic!(),
            DxgiFormat::R32G32_Float => gfx::Format::RG32f,
            DxgiFormat::R32G32_UInt => gfx::Format::RG32u,
            DxgiFormat::R32G32_SInt => gfx::Format::RG32i,
            DxgiFormat::R32G8X24_Typeless => panic!(),
            DxgiFormat::D32_Float_S8X24_UInt => panic!(),
            DxgiFormat::R32_Float_X8X24_Typeless => panic!(),
            DxgiFormat::X32_Typeless_G8X24_UInt => panic!(),
            DxgiFormat::R10G10B10A2_Typeless => panic!(),
            DxgiFormat::R10G10B10A2_UNorm => panic!(),
            DxgiFormat::R10G10B10A2_UInt => panic!(),
            DxgiFormat::R11G11B10_Float => panic!(),
            DxgiFormat::R8G8B8A8_Typeless => panic!(),
            DxgiFormat::R8G8B8A8_UNorm => gfx::Format::RGBA8n,
            DxgiFormat::R8G8B8A8_UNorm_sRGB => gfx::Format::RGBA8nSRGB,
            DxgiFormat::R8G8B8A8_UInt => gfx::Format::RGBA8u,
            DxgiFormat::R8G8B8A8_SNorm => panic!(),
            DxgiFormat::R8G8B8A8_SInt => gfx::Format::RGBA8i,
            DxgiFormat::R16G16_Typeless => panic!(),
            DxgiFormat::R16G16_Float => gfx::Format::RG16f,
            DxgiFormat::R16G16_UNorm => panic!(),
            DxgiFormat::R16G16_UInt => gfx::Format::RG16u,
            DxgiFormat::R16G16_SNorm => panic!(),
            DxgiFormat::R16G16_SInt => gfx::Format::RG16i,
            DxgiFormat::R32_Typeless => panic!(),
            DxgiFormat::D32_Float => gfx::Format::D32f,
            DxgiFormat::R32_Float => gfx::Format::R32f,
            DxgiFormat::R32_UInt => gfx::Format::R32u,
            DxgiFormat::R32_SInt => gfx::Format::R32i,
            DxgiFormat::R24G8_Typeless => panic!(),
            DxgiFormat::D24_UNorm_S8_UInt => gfx::Format::D24nS8u,
            DxgiFormat::R24_UNorm_X8_Typeless => panic!(),
            DxgiFormat::X24_Typeless_G8_UInt => panic!(),
            DxgiFormat::R8G8_Typeless => panic!(),
            DxgiFormat::R8G8_UNorm => panic!(),
            DxgiFormat::R8G8_UInt => panic!(),
            DxgiFormat::R8G8_SNorm => panic!(),
            DxgiFormat::R8G8_SInt => panic!(),
            DxgiFormat::R16_Typeless => panic!(),
            DxgiFormat::R16_Float => gfx::Format::R16f,
            DxgiFormat::D16_UNorm => gfx::Format::D16n,
            DxgiFormat::R16_UNorm => gfx::Format::R16n,
            DxgiFormat::R16_UInt => gfx::Format::R16u,
            DxgiFormat::R16_SNorm => panic!(),
            DxgiFormat::R16_SInt => gfx::Format::R16i,
            DxgiFormat::R8_Typeless => panic!(),
            DxgiFormat::R8_UNorm => panic!(),
            DxgiFormat::R8_UInt => panic!(),
            DxgiFormat::R8_SNorm => panic!(),
            DxgiFormat::R8_SInt => panic!(),
            DxgiFormat::A8_UNorm => panic!(),
            DxgiFormat::R1_UNorm => panic!(),
            DxgiFormat::R9G9B9E5_SharedExp => panic!(),
            DxgiFormat::R8G8_B8G8_UNorm => panic!(),
            DxgiFormat::G8R8_G8B8_UNorm => panic!(),
            DxgiFormat::BC1_Typeless => panic!(),
            DxgiFormat::BC1_UNorm => panic!(),
            DxgiFormat::BC1_UNorm_sRGB => panic!(),
            DxgiFormat::BC2_Typeless => panic!(),
            DxgiFormat::BC2_UNorm => panic!(),
            DxgiFormat::BC2_UNorm_sRGB => panic!(),
            DxgiFormat::BC3_Typeless => panic!(),
            DxgiFormat::BC3_UNorm => panic!(),
            DxgiFormat::BC3_UNorm_sRGB => panic!(),
            DxgiFormat::BC4_Typeless => panic!(),
            DxgiFormat::BC4_UNorm => panic!(),
            DxgiFormat::BC4_SNorm => panic!(),
            DxgiFormat::BC5_Typeless => panic!(),
            DxgiFormat::BC5_UNorm => panic!(),
            DxgiFormat::BC5_SNorm => panic!(),
            DxgiFormat::B5G6R5_UNorm => panic!(),
            DxgiFormat::B5G5R5A1_UNorm => panic!(),
            DxgiFormat::B8G8R8A8_UNorm => gfx::Format::BGRA8n,
            DxgiFormat::B8G8R8X8_UNorm => gfx::Format::BGRX8n,
            DxgiFormat::R10G10B10_XR_Bias_A2_UNorm => panic!(),
            DxgiFormat::B8G8R8A8_Typeless => panic!(),
            DxgiFormat::B8G8R8A8_UNorm_sRGB => gfx::Format::BGRA8nSRGB,
            DxgiFormat::B8G8R8X8_Typeless => panic!(),
            DxgiFormat::B8G8R8X8_UNorm_sRGB => gfx::Format::BGRX8nSRGB,
            DxgiFormat::BC6H_Typeless => panic!(),
            DxgiFormat::BC6H_UF16 => panic!(),
            DxgiFormat::BC6H_SF16 => panic!(),
            DxgiFormat::BC7_Typeless => panic!(),
            DxgiFormat::BC7_UNorm => panic!(),
            DxgiFormat::BC7_UNorm_sRGB => panic!(),
            DxgiFormat::AYUV => panic!(),
            DxgiFormat::Y410 => panic!(),
            DxgiFormat::Y416 => panic!(),
            DxgiFormat::NV12 => panic!(),
            DxgiFormat::P010 => panic!(),
            DxgiFormat::P016 => panic!(),
            DxgiFormat::Format_420_Opaque => panic!(),
            DxgiFormat::YUY2 => panic!(),
            DxgiFormat::Y210 => panic!(),
            DxgiFormat::Y216 => panic!(),
            DxgiFormat::NV11 => panic!(),
            DxgiFormat::AI44 => panic!(),
            DxgiFormat::IA44 => panic!(), 
            DxgiFormat::P8 => panic!(),
            DxgiFormat::A8P8 => panic!(),
            DxgiFormat::B4G4R4A4_UNorm => panic!(),
            DxgiFormat::P208 => panic!(),
            DxgiFormat::V208 => panic!(),
            DxgiFormat::V408 => panic!(),
            DxgiFormat::Force_UInt => panic!(),
        }
    }
    else {
        panic!("hotline_rs::image:: unsupported dds format is neither d3d or dxgi!");
    }
}