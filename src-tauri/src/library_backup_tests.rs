use super::*;
use serde_json::json;

struct Fixture {
    db: Database,
    paths: StoragePaths,
    _root: TempDir,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("backup-integration-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("assets/images")).unwrap();
        let paths = StoragePaths::new(
            root.join("bootstrap"),
            root.join("live.db"),
            root.join("assets"),
        )
        .unwrap();
        let db = Database::open(&paths.database).unwrap();
        db.run_migrations().unwrap();
        Self {
            db,
            paths,
            _root: TempDir(root),
        }
    }
    fn image(&self, relative: &str, color: [u8; 4]) -> PathBuf {
        let p = self._root.0.join(relative);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        image::RgbaImage::from_pixel(320, 160, image::Rgba(color))
            .save(&p)
            .unwrap();
        p
    }
    fn operation(&self) -> OperationGuard {
        Operations::default()
            .begin(&uuid::Uuid::new_v4().to_string())
            .unwrap()
    }
    fn archive(&self) -> PathBuf {
        let path = self
            ._root
            .0
            .join(format!("{}.astrobackup", uuid::Uuid::new_v4()));
        create_archive(
            &self.db,
            &self.paths,
            &path,
            &self.operation(),
            &|_, _, _| {},
        )
        .unwrap();
        path
    }
    fn add_generation(&self, id: &str) {
        self.db.conn.lock().unwrap().execute("INSERT INTO generations(id,prompt,engine,status) VALUES(?1,'preserve prompt','gpt-image-2','completed')",[id]).unwrap();
    }
    fn add_image(&self, id: &str, generation: &str, image: &Path, thumb: Option<&Path>) {
        self.db.conn.lock().unwrap().execute("INSERT INTO images(id,generation_id,file_path,thumbnail_path,width,height,file_size) VALUES(?1,?2,?3,?4,320,160,?5)",params![id,generation,image.to_string_lossy(),thumb.map(|p|p.to_string_lossy().into_owned()),fs::metadata(image).unwrap().len()]).unwrap();
    }
}

#[test]
fn backup_roundtrip_preserves_canvas_external_sources_multi_images_trash_and_projects() {
    let f = Fixture::new();
    let first = f.image("assets/images/a.png", [255, 0, 0, 255]);
    let second = f.image("assets/images/b.png", [0, 0, 255, 255]);
    let external = f.image("external/reference.png", [0, 255, 0, 255]);
    f.add_generation("g");
    f.add_generation("trashed");
    f.add_image("i1", "g", &first, Some(&first));
    f.add_image("i2", "g", &second, Some(&second));
    f.add_image("i3", "trashed", &first, Some(&first));
    let document = f.paths.assets.join("canvas/doc.json");
    fs::create_dir_all(document.parent().unwrap()).unwrap();
    let text = format!(
        "A literal path in text must remain unchanged: {}",
        external.display()
    );
    let canvas = json!({"version":2,"layers":[{"objects":[{"type":"image","image_path":external},{"type":"text","text":text}]}],"generation_requests":[{"source_image_path":external,"prompt":text,"parameters":{"advancedOptions":{"markupPath":external}}}]});
    fs::write(&document, serde_json::to_vec(&canvas).unwrap()).unwrap();
    {
        let conn = f.db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects(id,name) VALUES('p','Portable project')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO conversations(id,title,project_id) VALUES('c','Portable conversation','p')",[]).unwrap();
        conn.execute("UPDATE generations SET conversation_id='c',source_image_paths=?1,source_image_count=1 WHERE id='g'",[json!([external]).to_string()]).unwrap();
        conn.execute(
            "UPDATE generations SET deleted_at='2026-01-01T00:00:00Z' WHERE id='trashed'",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO canvas_documents(id,project_id,name,document_path,width,height) VALUES('canvas','p','Canvas',?1,320,160)",[document.to_string_lossy().as_ref()]).unwrap();
        conn.execute(
            "INSERT INTO folders(id,name) VALUES('folder','Collection')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO folder_images(folder_id,image_id) VALUES('folder','i2')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO prompt_extractions(id,image_path,prompt,llm_config_id) VALUES('extract',?1,'original text','llm')",[external.to_string_lossy().as_ref()]).unwrap();
    }
    let archive = f.archive();
    let destination = restore_archive(&f.paths, &archive, &f.operation(), &|_, _, _| {}).unwrap();
    let restored = Database::open(&destination.join("library.db")).unwrap();
    let conn = restored.conn.lock().unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM images", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        3
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM generations WHERE deleted_at IS NOT NULL",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT project_id FROM conversations WHERE id='c'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "p"
    );
    assert_eq!(
        conn.query_row(
            "SELECT image_id FROM folder_images WHERE folder_id='folder'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "i2"
    );
    for (id, original) in [("i1", &first), ("i2", &second)] {
        let path: String = conn
            .query_row("SELECT file_path FROM images WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(Path::new(&path).starts_with(&destination));
        assert_eq!(fs::read(path).unwrap(), fs::read(original).unwrap());
    }
    let source: String = conn
        .query_row(
            "SELECT source_image_paths FROM generations WHERE id='g'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let source: Vec<String> = serde_json::from_str(&source).unwrap();
    assert!(Path::new(&source[0]).starts_with(&destination));
    assert_eq!(fs::read(&source[0]).unwrap(), fs::read(&external).unwrap());
    let canvas_path: String = conn
        .query_row(
            "SELECT document_path FROM canvas_documents WHERE id='canvas'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(canvas_path).unwrap()).unwrap();
    assert_eq!(saved["layers"][0]["objects"][1]["text"], text);
    assert_eq!(saved["generation_requests"][0]["prompt"], text);
    for p in [
        &saved["layers"][0]["objects"][0]["image_path"],
        &saved["generation_requests"][0]["source_image_path"],
        &saved["generation_requests"][0]["parameters"]["advancedOptions"]["markupPath"],
    ] {
        assert!(Path::new(p.as_str().unwrap()).starts_with(&destination));
        assert!(Path::new(p.as_str().unwrap()).is_file());
    }
    let extraction: String = conn
        .query_row(
            "SELECT image_path FROM prompt_extractions WHERE id='extract'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(Path::new(&extraction).starts_with(&destination));
    assert!(!f.paths.bootstrap.join("active-library.json").exists());
    assert_eq!(
        fs::read(&document).unwrap(),
        serde_json::to_vec(&canvas).unwrap()
    );
}

#[test]
fn null_and_missing_thumbnails_rebuild_without_mutating_live_rows() {
    let f = Fixture::new();
    let original = f.image("assets/images/original.png", [3, 17, 231, 255]);
    f.add_generation("g");
    let missing = f.paths.assets.join("images/missing-thumb.png");
    f.add_image("null", "g", &original, None);
    f.add_image("missing", "g", &original, Some(&missing));
    let archive = f.archive();
    let destination = restore_archive(&f.paths, &archive, &f.operation(), &|_, _, _| {}).unwrap();
    let restored = Database::open(&destination.join("library.db")).unwrap();
    let conn = restored.conn.lock().unwrap();
    for id in ["null", "missing"] {
        let p: String = conn
            .query_row("SELECT thumbnail_path FROM images WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        let thumb = image::open(&p).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 128));
        assert!(Path::new(&p).starts_with(&destination));
    }
    assert!(f
        .db
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT thumbnail_path FROM images WHERE id='null'",
            [],
            |r| r.get::<_, Option<String>>(0)
        )
        .unwrap()
        .is_none());
    assert!(!missing.exists());
}

#[test]
fn backup_missing_original_or_external_reference_is_rejected_without_publication() {
    for source in [false, true] {
        let f = Fixture::new();
        f.add_generation("g");
        let original = f.image("assets/images/original.png", [1, 2, 3, 255]);
        f.add_image("i", "g", &original, Some(&original));
        if source {
            f.db.conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE generations SET source_image_paths=?1",
                    [json!([f._root.0.join("external/missing.png")]).to_string()],
                )
                .unwrap();
        } else {
            fs::remove_file(original).unwrap();
        }
        let target = f._root.0.join("failed.astrobackup");
        assert!(create_archive(&f.db, &f.paths, &target, &f.operation(), &|_, _, _| {}).is_err());
        assert!(!target.exists());
        assert!(!f.paths.bootstrap.join("active-library.json").exists());
    }
}

fn archive_entries(path: &Path) -> Vec<(String, Vec<u8>)> {
    let mut zip = ZipArchive::new(File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut f = zip.by_index(i).unwrap();
            let name = f.name().into();
            let mut bytes = vec![];
            f.read_to_end(&mut bytes).unwrap();
            (name, bytes)
        })
        .collect()
}
fn write_entries(path: &Path, entries: &[(String, Vec<u8>)]) {
    let mut zip = ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn restore_rejects_a_checksummed_database_pointing_to_missing_asset() {
    let f = Fixture::new();
    f.add_generation("g");
    let original = f.image("assets/images/original.png", [1, 2, 3, 255]);
    f.add_image("i", "g", &original, Some(&original));
    let good = f.archive();
    let mut entries = archive_entries(&good);
    let db_file = f._root.0.join("tampered.db");
    fs::write(
        &db_file,
        &entries.iter().find(|(n, _)| n == "library.db").unwrap().1,
    )
    .unwrap();
    {
        let conn = Connection::open(&db_file).unwrap();
        conn.execute(
            "UPDATE images SET file_path='assets/images/not-in-archive.png'",
            [],
        )
        .unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
    }
    let bytes = fs::read(&db_file).unwrap();
    let (size, sha) = hash_file(&db_file).unwrap();
    entries
        .iter_mut()
        .find(|(n, _)| n == "library.db")
        .unwrap()
        .1 = bytes;
    let manifest = &mut entries
        .iter_mut()
        .find(|(n, _)| n == "manifest.json")
        .unwrap()
        .1;
    let mut m: BackupManifest = serde_json::from_slice(manifest).unwrap();
    let db = m.files.iter_mut().find(|f| f.name == "library.db").unwrap();
    db.size = size;
    db.sha256 = sha;
    *manifest = serde_json::to_vec(&m).unwrap();
    let bad = f._root.0.join("missing.astrobackup");
    write_entries(&bad, &entries);
    assert!(inspect_archive(&bad).is_ok());
    assert!(restore_archive(&f.paths, &bad, &f.operation(), &|_, _, _| {}).is_err());
    assert!(!f.paths.bootstrap.join("active-library.json").exists());
}

#[test]
fn archive_inspection_rejects_corruption_and_unsupported_versions() {
    let f = Fixture::new();
    let archive = f.archive();
    let original = archive_entries(&archive);
    for variant in ["checksum", "version", "schema", "traversal"] {
        let mut entries = original.clone();
        if variant == "checksum" {
            entries
                .iter_mut()
                .find(|(n, _)| n == "library.db")
                .unwrap()
                .1[100] ^= 1;
        } else if variant == "traversal" {
            entries.push(("../outside".into(), b"no".to_vec()));
        } else {
            let bytes = &mut entries
                .iter_mut()
                .find(|(n, _)| n == "manifest.json")
                .unwrap()
                .1;
            let mut m: Value = serde_json::from_slice(bytes).unwrap();
            m[if variant == "version" {
                "version"
            } else {
                "schema_version"
            }] = json!(999);
            *bytes = serde_json::to_vec(&m).unwrap();
        }
        let bad = f._root.0.join(format!("{variant}.zip"));
        write_entries(&bad, &entries);
        assert!(inspect_archive(&bad).is_err(), "accepted {variant}");
    }
}

#[test]
fn archive_inspection_rejects_symlink_and_duplicate_names() {
    let f = Fixture::new();
    let symlink = f._root.0.join("symlink.zip");
    {
        let mut zip = ZipWriter::new(File::create(&symlink).unwrap());
        zip.add_symlink("assets/escape", "/etc/passwd", SimpleFileOptions::default())
            .unwrap();
        zip.finish().unwrap();
    }
    assert!(inspect_archive(&symlink).is_err());
    let duplicate = f._root.0.join("duplicate.zip");
    write_entries(
        &duplicate,
        &[
            ("library.db".into(), vec![1]),
            ("duplicx.db".into(), vec![2]),
        ],
    );
    let mut bytes = fs::read(&duplicate).unwrap();
    let needle = b"duplicx.db";
    for i in 0..bytes.len() - needle.len() {
        if &bytes[i..i + needle.len()] == needle {
            bytes[i..i + needle.len()].copy_from_slice(b"library.db");
        }
    }
    fs::write(&duplicate, bytes).unwrap();
    assert!(inspect_archive(&duplicate).is_err());
}

#[test]
fn nested_profile_and_llm_credentials_are_removed_from_portable_database() {
    let f = Fixture::new();
    f.db.set_setting("model_provider_profiles::gpt-image-2",&json!([{"id":"profile","name":"Preserve name","api_key":"UNIQUE_API_SECRET","endpoint_settings":{"base_url":" HTTPS://user:UNIQUE_PASSWORD@example.org/v1?client_secret=UNIQUE_QUERY_SECRET&subscription-key=UNIQUE_SUBSCRIPTION_SECRET&X-Amz-Signature=UNIQUE_SIGNATURE_SECRET&access_key=UNIQUE_ACCESS_SECRET&api-version=2026#UNIQUE_FRAGMENT_SECRET"}}]).to_string()).unwrap();
    f.db.set_setting(
        "llm_configs",
        &json!([{"id":"llm","api_key":"UNIQUE_LLM_SECRET"}]).to_string(),
    )
    .unwrap();
    let archive = f.archive();
    let db = archive_entries(&archive)
        .into_iter()
        .find(|(n, _)| n == "library.db")
        .unwrap()
        .1;
    for secret in [
        "UNIQUE_API_SECRET",
        "UNIQUE_PASSWORD",
        "UNIQUE_QUERY_SECRET",
        "UNIQUE_SUBSCRIPTION_SECRET",
        "UNIQUE_SIGNATURE_SECRET",
        "UNIQUE_ACCESS_SECRET",
        "UNIQUE_FRAGMENT_SECRET",
        "UNIQUE_LLM_SECRET",
    ] {
        assert!(
            !db.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "leaked {secret}"
        );
    }
    assert!(db
        .windows("Preserve name".len())
        .any(|w| w == b"Preserve name"));
}

#[test]
fn restore_approval_is_bound_to_inspected_files_even_if_archive_is_replaced() {
    let f = Fixture::new();
    let archive = f.archive();
    let approved = inspect_archive(&archive).unwrap();
    f.add_generation("unapproved");
    let replacement = f.archive();
    fs::copy(replacement, &archive).unwrap();
    // Simulate replacement after the command's coarse file hash check.
    let outcome =
        restore_approved_archive(&f.paths, &archive, &approved, &f.operation(), &|_, _, _| {});
    assert!(
        outcome.is_err(),
        "a different valid archive must not replace the reviewed library"
    );
    assert!(!f.paths.bootstrap.join("active-library.json").exists());
}
#[test]
fn credential_url_signed_query_material_is_removed() {
    let mut value = json!({"url":"https://example.org/v1?access_key=LEAK_ACCESS&X-Amz-Credential=LEAK_ID&X-Amz-Signature=LEAK_SIGNATURE&api-version=2026"});
    scrub(&mut value);
    assert!(!value.to_string().contains("LEAK_"));
    assert!(value.to_string().contains("api-version=2026"));
}

#[test]
fn extraction_keeps_approved_hashes_after_archive_replacement_and_cleans_staging() {
    let f = Fixture::new();
    let archive = f.archive();
    let approved = inspect_archive(&archive).unwrap();
    f.add_generation("changed-after-final-check");
    let replacement = f.archive();
    fs::copy(replacement, &archive).unwrap();
    // The extraction boundary retains the approved manifest even if replacement
    // occurs after the final inspection but before opening the archive again.
    let result =
        restore_archive_manifest(&f.paths, &archive, &approved, &f.operation(), &|_, _, _| {});
    assert!(result.is_err());
    assert_eq!(
        fs::read_dir(f.paths.bootstrap.join("libraries"))
            .unwrap()
            .count(),
        0
    );
    assert!(!f.paths.bootstrap.join("active-library.json").exists());
}
