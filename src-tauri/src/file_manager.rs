use std::path::PathBuf;
use image::GenericImageView;

const THUMBNAIL_SIZE: u32 = 256;

pub struct FileManager {
    base_dir: PathBuf,
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

    pub fn save_image(&self, generation_id: &str, data: &[u8]) -> Result<SavedImage, String> {
        let now = chrono::Local::now();
        let date_path = now.format("%Y/%m/%d").to_string();
        let filename = format!("{}.png", generation_id);

        let image_dir = self.base_dir.join("images").join(&date_path);
        std::fs::create_dir_all(&image_dir)
            .map_err(|e| format!("Create date dir failed: {}", e))?;

        let file_path = image_dir.join(&filename);
        std::fs::write(&file_path, data)
            .map_err(|e| format!("Write image failed: {}", e))?;

        let img = image::load_from_memory(data)
            .map_err(|e| format!("Decode image failed: {}", e))?;
        let (width, height) = img.dimensions();

        let thumb_path = self.generate_thumbnail(&img, &date_path, generation_id)?;

        Ok(SavedImage {
            file_path: file_path.to_string_lossy().to_string(),
            thumbnail_path: thumb_path,
            width: width as i32,
            height: height as i32,
            file_size: data.len() as i64,
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
        thumb.save(&thumb_path)
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
