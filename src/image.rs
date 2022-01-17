use stb_image_rust;
use stb_image_write_rust::ImageWriter::ImageWriter;

use std::fs;
use std::io::Read;

pub struct ImageData {
    /// Horizontal dimension of the image in texels
    pub width: u64,
    /// Vertical dimension of the image in texels
    pub height: u64,
    /// Number of components per-pixel (RGBA = 4)
    pub components: u32,
    /// Vector of linear image data tightly packed
    pub data: Vec<u8>
}

/// Writes a buffer of image data to a file. The type of image format written is determined by filename ext 
/// supported image formats are (png, bmp, tga and jpg).
pub fn write_to_file(filename: String, width: u64, height: u64, components: u32, image_data: &[u8] ) -> Result<(), String> {
    let path = std::path::Path::new(&filename);
    let mut writer = ImageWriter::new(&filename);
    match path.extension() {
        Some(os_str) => {
            match os_str.to_str() {
                Some("png") => {
                    writer.write_png(width as i32, height as i32, components as i32, image_data.as_ptr());
                    Ok(())
                },
                Some("bmp") => {
                    writer.write_bmp(width as i32, height as i32, components as i32, image_data.as_ptr());
                    Ok(())
                },
                Some("tga") => {
                    writer.write_tga(width as i32, height as i32, components as i32, image_data.as_ptr());
                    Ok(())
                },
                Some("jpg") => {
                    writer.write_jpg(width as i32, height as i32, components as i32, image_data.as_ptr(), 90);
                    Ok(())
                },
                _ => {
                    if os_str.to_str().is_some() {
                        return Err(format!("hotline::image: Image format '{}' is not supported", os_str.to_str().unwrap()))
                    }
                    else {
                        Err(format!("hotline::image: Filename '{}' did not specify image format extension!", filename))
                    }
                }
            }
        }
        _ => {
            Err(format!("hotline::image: Filename '{}' has no extension!", filename))
        }
    }
}

/// Loads an image from file returning information in the ImageData struct
/// supported formats are (png, tga, bmp, jpg, gif)
pub fn load_from_file(filename: String) -> ImageData {
    // read file
    let path = std::path::Path::new(&filename);
    let mut f = fs::File::open(path).expect("hotline::image: File not found");
    let mut contents = vec![];
    f.read_to_end(&mut contents).expect("hotline::image: Failed to read file.");

    let mut x = 0;
    let mut y = 0;
    let mut comp = 0;
    let mut data_out : Vec<u8> = Vec::new();

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

        // copy data
        let data_size_bytes = x * y * comp;
        data_out.resize(data_size_bytes as usize, 0);

        std::ptr::copy_nonoverlapping(img, data_out.as_mut_ptr(), data_size_bytes as usize);

        // cleanup
        stb_image_rust::c_runtime::free(img);
    }

    ImageData {
        width: x as u64,
        height: y as u64,
        components: comp as u32,
        data: data_out
    }
}

