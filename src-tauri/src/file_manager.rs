use crate::error::AppError;
use image::{GenericImageView, ImageFormat};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

const THUMBNAIL_SIZE: u32 = 256;

pub struct FileManager {
    base_dir: PathBuf,
}

struct StagedImageFile {
    image_id: String,
    staged_image_path: PathBuf,
    staged_thumbnail_path: PathBuf,
    final_image_path: PathBuf,
    final_thumbnail_path: PathBuf,
    width: i32,
    height: i32,
    file_size: i64,
}

pub(crate) struct StagedGenerationFiles {
    entries: Vec<StagedImageFile>,
    staging_dir: PathBuf,
}

pub(crate) struct PromotedGenerationFiles {
    entries: Vec<StagedImageFile>,
    cleanup_armed: bool,
}

struct PromotedPathGuard {
    paths: Vec<PathBuf>,
    armed: bool,
}

impl Drop for PromotedPathGuard {
    fn drop(&mut self) {
        if self.armed {
            for path in self.paths.iter().rev() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl StagedGenerationFiles {
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn final_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .flat_map(|entry| {
                [
                    entry.final_image_path.clone(),
                    entry.final_thumbnail_path.clone(),
                ]
            })
            .collect()
    }

    pub(crate) fn promote(mut self) -> Result<PromotedGenerationFiles, String> {
        if self.final_paths().iter().any(|path| path.exists()) {
            return Err("Generation output already exists".to_string());
        }

        let mut promoted = PromotedPathGuard {
            paths: Vec::with_capacity(self.entries.len() * 2),
            armed: true,
        };
        for entry in &self.entries {
            std::fs::rename(&entry.staged_image_path, &entry.final_image_path)
                .map_err(|error| format!("Promote image failed: {error}"))?;
            promoted.paths.push(entry.final_image_path.clone());

            std::fs::rename(&entry.staged_thumbnail_path, &entry.final_thumbnail_path)
                .map_err(|error| format!("Promote thumbnail failed: {error}"))?;
            promoted.paths.push(entry.final_thumbnail_path.clone());
        }

        let mut synced_directories = std::collections::HashSet::new();
        for path in &promoted.paths {
            if let Some(parent) = path.parent() {
                synced_directories.insert(parent.to_path_buf());
            }
        }
        for directory in synced_directories {
            File::open(directory)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("Sync promoted image directory failed: {error}"))?;
        }

        let entries = std::mem::take(&mut self.entries);
        let _ = std::fs::remove_dir_all(&self.staging_dir);
        promoted.armed = false;
        Ok(PromotedGenerationFiles {
            entries,
            cleanup_armed: true,
        })
    }
}

impl Drop for StagedGenerationFiles {
    fn drop(&mut self) {
        for entry in &self.entries {
            let _ = std::fs::remove_file(&entry.staged_image_path);
            let _ = std::fs::remove_file(&entry.staged_thumbnail_path);
        }
        let _ = std::fs::remove_dir_all(&self.staging_dir);
    }
}

impl PromotedGenerationFiles {
    pub(crate) fn final_paths(&self) -> Vec<PathBuf> {
        self.entries
            .iter()
            .flat_map(|entry| {
                [
                    entry.final_image_path.clone(),
                    entry.final_thumbnail_path.clone(),
                ]
            })
            .collect()
    }

    pub(crate) fn saved_images(&self, generation_id: &str) -> Vec<crate::models::GeneratedImage> {
        self.entries
            .iter()
            .map(|entry| crate::models::GeneratedImage {
                id: entry.image_id.clone(),
                generation_id: generation_id.to_string(),
                file_path: entry.final_image_path.to_string_lossy().to_string(),
                thumbnail_path: entry.final_thumbnail_path.to_string_lossy().to_string(),
                width: entry.width,
                height: entry.height,
                file_size: entry.file_size,
            })
            .collect()
    }

    pub(crate) fn disarm_cleanup(&mut self) {
        self.cleanup_armed = false;
    }
}

impl Drop for PromotedGenerationFiles {
    fn drop(&mut self) {
        if self.cleanup_armed {
            for path in self.final_paths() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
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

pub(crate) fn canonicalize_existing_managed_path(
    path: &Path,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, AppError> {
    let canonical_path = path.canonicalize().map_err(|e| AppError::FileSystem {
        message: format!("Resolve path failed: {}", e),
    })?;

    for root in allowed_roots {
        if let Ok(canonical_root) = root.canonicalize() {
            if canonical_path.starts_with(&canonical_root) {
                return Ok(canonical_path);
            }
        }
    }

    Err(AppError::Validation {
        message: "File path is outside managed storage.".to_string(),
    })
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

    pub(crate) fn stage_generation_images(
        &self,
        generation_id: &str,
        images_data: &[Vec<u8>],
        output_format: &str,
        created_at: &str,
    ) -> Result<StagedGenerationFiles, String> {
        if generation_id.is_empty()
            || generation_id.len() > 128
            || !generation_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err("Generation output identity is invalid".to_string());
        }
        if images_data.is_empty() || images_data.len() > 4 {
            return Err("Generation response must contain between one and four images".to_string());
        }

        let date_path = chrono::DateTime::parse_from_rfc3339(created_at)
            .map_err(|_| "Generation creation timestamp is invalid".to_string())?
            .with_timezone(&chrono::Local)
            .format("%Y/%m/%d")
            .to_string();
        let image_dir = self.base_dir.join("images").join(&date_path);
        let thumbnail_dir = self.base_dir.join("thumbnails").join(&date_path);
        std::fs::create_dir_all(&image_dir)
            .map_err(|error| format!("Create image output directory failed: {error}"))?;
        std::fs::create_dir_all(&thumbnail_dir)
            .map_err(|error| format!("Create thumbnail output directory failed: {error}"))?;

        let staging_dir = self
            .base_dir
            .join(".generation-staging")
            .join(generation_id)
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&staging_dir)
            .map_err(|error| format!("Create generation staging directory failed: {error}"))?;
        let mut staged = StagedGenerationFiles {
            entries: Vec::with_capacity(images_data.len()),
            staging_dir,
        };

        for (index, data) in images_data.iter().enumerate() {
            let image = image::load_from_memory(data)
                .map_err(|error| format!("Decode staged image failed: {error}"))?;
            let (width, height) = image.dimensions();
            let extension = detected_image_extension(data)
                .unwrap_or_else(|| extension_for_output_format(output_format));
            let image_id = format!("{generation_id}_{index}");
            let staged_image_path = staged.staging_dir.join(format!("{image_id}.{extension}"));
            let staged_thumbnail_path = staged.staging_dir.join(format!("{image_id}_thumb.png"));
            let final_image_path = image_dir.join(format!("{image_id}.{extension}"));
            let final_thumbnail_path = thumbnail_dir.join(format!("{image_id}_thumb.png"));

            staged.entries.push(StagedImageFile {
                image_id,
                staged_image_path: staged_image_path.clone(),
                staged_thumbnail_path: staged_thumbnail_path.clone(),
                final_image_path,
                final_thumbnail_path,
                width: width as i32,
                height: height as i32,
                file_size: data.len() as i64,
            });

            let mut image_file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged_image_path)
                .map_err(|error| format!("Create staged image failed: {error}"))?;
            image_file
                .write_all(data)
                .map_err(|error| format!("Write staged image failed: {error}"))?;
            image_file
                .sync_all()
                .map_err(|error| format!("Sync staged image failed: {error}"))?;
            drop(image_file);

            image
                .thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
                .save_with_format(&staged_thumbnail_path, ImageFormat::Png)
                .map_err(|error| format!("Write staged thumbnail failed: {error}"))?;
            File::open(&staged_thumbnail_path)
                .and_then(|file| file.sync_all())
                .map_err(|error| format!("Sync staged thumbnail failed: {error}"))?;
        }

        File::open(&staged.staging_dir)
            .and_then(|file| file.sync_all())
            .map_err(|error| format!("Sync generation staging directory failed: {error}"))?;
        Ok(staged)
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
        let extension = detected_image_extension(data)
            .unwrap_or_else(|| extension_for_output_format(output_format));
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
        let roots = [
            self.base_dir.join("images"),
            self.base_dir.join("thumbnails"),
        ];
        if let Ok(path) = canonicalize_existing_managed_path(Path::new(file_path), &roots) {
            let _ = std::fs::remove_file(path);
        }
        if let Ok(path) = canonicalize_existing_managed_path(Path::new(thumbnail_path), &roots) {
            let _ = std::fs::remove_file(path);
        }
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

    #[test]
    fn staged_generation_files_promote_without_overwrite_and_cleanup_until_disarmed() {
        let base_dir = test_base_dir();
        let manager = FileManager::new(base_dir.clone());
        let source = jpeg_bytes();
        let staged = manager
            .stage_generation_images(
                "generation-1",
                &[source.clone()],
                "png",
                "2026-04-29T06:18:01Z",
            )
            .expect("stage generation image");
        assert_eq!(staged.len(), 1);
        assert!(staged.final_paths().iter().all(|path| !path.exists()));

        let promoted = staged.promote().expect("promote generation image");
        let saved = promoted.saved_images("generation-1");
        assert_eq!(saved.len(), 1);
        assert_eq!(std::fs::read(&saved[0].file_path).unwrap(), source);
        let promoted_paths = promoted.final_paths();
        drop(promoted);
        assert!(promoted_paths.iter().all(|path| !path.exists()));

        let staged = manager
            .stage_generation_images(
                "generation-1",
                &[jpeg_bytes()],
                "png",
                "2026-04-29T06:18:01Z",
            )
            .expect("restage generation image");
        let mut promoted = staged.promote().expect("promote generation image");
        let committed_paths = promoted.final_paths();
        promoted.disarm_cleanup();
        drop(promoted);
        assert!(committed_paths.iter().all(|path| path.exists()));

        std::fs::remove_dir_all(base_dir).ok();
    }
}
