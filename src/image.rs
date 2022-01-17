//use stb_image_rust;
use stb_image_write_rust::ImageWriter::ImageWriter;

/// Writes a buffer of image data to file, supported image formats are (png, bmp, tga and jpg).
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