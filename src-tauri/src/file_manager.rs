use image::{GenericImageView, ImageFormat};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const THUMBNAIL_SIZE: u32 = 256;

pub struct FileManager {
    base_dir: PathBuf,
}

pub fn extension_for_output_format(output_format: &str) -> &'static str {
    match output_format {
        "jpeg" | "jpg" => "jpeg",
        "webp" => "webp",
        _ => "png",
    }
}

fn extension_for_image_format(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("png"),
        ImageFormat::Jpeg => Some("jpeg"),
        ImageFormat::WebP => Some("webp"),
        ImageFormat::Gif => Some("gif"),
        ImageFormat::Bmp => Some("bmp"),
        ImageFormat::Tiff => Some("tiff"),
        _ => None,
    }
}

pub fn detected_image_extension(data: &[u8]) -> Option<&'static str> {
    image::guess_format(data)
        .ok()
        .and_then(extension_for_image_format)
}

fn temporary_output_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    path.with_file_name(format!(".{}.{}.tmp", file_name, uuid::Uuid::new_v4()))
}

fn write_original_image_bytes(data: &[u8], path: &Path) -> Result<i64, String> {
    let temp_path = temporary_output_path(path);
    let mut file = File::create(&temp_path).map_err(|e| format!("Create image failed: {}", e))?;
    file.write_all(data)
        .map_err(|e| format!("Write image failed: {}", e))?;
    file.sync_all()
        .map_err(|e| format!("Sync image failed: {}", e))?;

    let file_size = data.len() as i64;

    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Replace image failed: {}", e))?;
    }

    std::fs::rename(&temp_path, path).map_err(|e| format!("Write image failed: {}", e))?;
    Ok(file_size)
}

impl FileManager {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn ensure_dirs(&self) -> Result<(), String> {
        let images_dir = self.base_dir.join("images");
        let thumbs_dir = self.base_dir.join("thumbnails");
        std::fs::create_dir_all(&images_dir)
            .map_err(|e| format!("Create images dir failed: {}", e))?;
        std::fs::create_dir_all(&thumbs_dir)
            .map_err(|e| format!("Create thumbnails dir failed: {}", e))?;
        Ok(())
    }

    pub fn save_image_at(
        &self,
        generation_id: &str,
        data: &[u8],
        output_format: &str,
        created_at: Option<&str>,
    ) -> Result<SavedImage, String> {
        let date_path = created_at
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| {
                value
                    .with_timezone(&chrono::Local)
                    .format("%Y/%m/%d")
                    .to_string()
            })
            .unwrap_or_else(|| chrono::Local::now().format("%Y/%m/%d").to_string());
        let img =
            image::load_from_memory(data).map_err(|e| format!("Decode image failed: {}", e))?;
        let (width, height) = img.dimensions();
        let extension =
            detected_image_extension(data).unwrap_or_else(|| extension_for_output_format(output_format));
        let filename = format!("{}.{}", generation_id, extension);

        let image_dir = self.base_dir.join("images").join(&date_path);
        std::fs::create_dir_all(&image_dir)
            .map_err(|e| format!("Create date dir failed: {}", e))?;

        let file_path = image_dir.join(&filename);
        let file_size = write_original_image_bytes(data, &file_path)?;

        let thumb_path = self.generate_thumbnail(&img, &date_path, generation_id)?;

        Ok(SavedImage {
            file_path: file_path.to_string_lossy().to_string(),
            thumbnail_path: thumb_path,
            width: width as i32,
            height: height as i32,
            file_size,
        })
    }

    fn generate_thumbnail(
        &self,
        img: &image::DynamicImage,
        date_path: &str,
        generation_id: &str,
    ) -> Result<String, String> {
        let thumb_dir = self.base_dir.join("thumbnails").join(date_path);
        std::fs::create_dir_all(&thumb_dir)
            .map_err(|e| format!("Create thumbnail dir failed: {}", e))?;

        let thumb = img.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
        let thumb_path = thumb_dir.join(format!("{}_thumb.png", generation_id));
        thumb
            .save(&thumb_path)
            .map_err(|e| format!("Save thumbnail failed: {}", e))?;

        Ok(thumb_path.to_string_lossy().to_string())
    }

    pub fn delete_image(&self, file_path: &str, thumbnail_path: &str) -> Result<(), String> {
        let _ = std::fs::remove_file(file_path);
        let _ = std::fs::remove_file(thumbnail_path);
        Ok(())
    }
}

pub struct SavedImage {
    pub file_path: String,
    pub thumbnail_path: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
    use std::io::Cursor;

    fn test_base_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "astro-studio-file-manager-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn jpeg_bytes() -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(4, 4, Rgb([24, 96, 180])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode jpeg");
        bytes
    }

    #[test]
    fn detects_image_extension_from_response_bytes() {
        assert_eq!(detected_image_extension(&jpeg_bytes()), Some("jpeg"));
    }

    #[test]
    fn save_image_preserves_response_bytes_without_reencoding() {
        let base_dir = test_base_dir();
        let manager = FileManager::new(base_dir.clone());
        let source = jpeg_bytes();
        let saved = manager
            .save_image_at(
                "original-response",
                &source,
                "png",
                Some("2026-04-29T06:18:01Z"),
            )
            .expect("save original response");
        let saved_data = std::fs::read(&saved.file_path).expect("read saved image");

        assert!(
            saved.file_path.ends_with("original-response.jpeg"),
            "expected detected jpeg extension, got {}",
            saved.file_path
        );
        assert_eq!(saved_data, source);
        assert_eq!(saved.file_size, saved_data.len() as i64);

        std::fs::remove_dir_all(base_dir).ok();
    }
}
