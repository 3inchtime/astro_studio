use super::*;

#[test]
fn canvas_export_registers_its_source_for_image_editing() {
    let dir = std::env::temp_dir().join(format!(
        "astro-canvas-export-registry-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("frame.png");
    let registry = crate::source_images::SelectedImageRegistry::default();
    let bytes = decode_base64_bytes("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==").unwrap();
    let registered = write_registered_canvas_export(&file_path, &bytes, &registry).unwrap();
    assert!(registry.contains_path(Path::new(&registered)).unwrap());
    assert_eq!(fs::read(&registered).unwrap(), bytes);
    assert_eq!(Path::new(&registered), file_path.canonicalize().unwrap());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn save_canvas_export_writes_png_file() {
    let dir = std::env::temp_dir().join(format!(
        "astro-studio-canvas-export-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let registry = SelectedImageRegistry::default();
    let document_id = uuid::Uuid::new_v4().to_string();
    let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==";
    let file_path = save(&dir, &document_id, png_data, &registry).unwrap();
    assert!(Path::new(&file_path).starts_with(
        dir.canonicalize()
            .unwrap()
            .join("canvas/exports")
            .join(&document_id)
    ));
    assert_eq!(
        fs::read(&file_path).unwrap(),
        decode_base64_bytes(png_data).unwrap()
    );
    assert!(registry.contains_path(Path::new(&file_path)).unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn save_canvas_export_rejects_document_id_path_traversal() {
    let result = validate_canvas_document_id_component("../escaped-export");

    assert!(matches!(result, Err(AppError::Validation { .. })));
}
