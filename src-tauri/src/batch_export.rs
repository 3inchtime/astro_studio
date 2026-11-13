use crate::{
    db::Database,
    operations::{OperationGuard, Operations},
    storage::StoragePaths,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::State;

type ExportResult<T> = Result<T, String>;
#[derive(Default)]
pub(crate) struct ExportSessions(Mutex<HashMap<String, ExportSession>>);
#[derive(Clone)]
struct ExportItem {
    image_id: String,
    generation_id: String,
    source: PathBuf,
    name: String,
    recipe: Option<serde_json::Value>,
    image_output: Option<PathBuf>,
}
struct ExportSession {
    directory: PathBuf,
    pending: Vec<ExportItem>,
    total: usize,
    completed: usize,
}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ExportFailure {
    image_id: String,
    generation_id: String,
    code: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct BatchExportResult {
    total: usize,
    completed: usize,
    succeeded_image_ids: Vec<String>,
    cancelled_image_ids: Vec<String>,
    failed: Vec<ExportFailure>,
    cancelled: bool,
    retry_token: Option<String>,
}
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn safe_piece(value: &str) -> String {
    let value: String = value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .take(80)
        .collect();
    if value.is_empty() {
        "image".into()
    } else {
        value
    }
}
fn publish(temp: &Path, directory: &Path, name: &str) -> ExportResult<PathBuf> {
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid_name")?;
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    for suffix in 0..100_000 {
        let candidate = directory.join(if suffix == 0 {
            name.into()
        } else {
            format!("{stem}_{suffix}.{extension}")
        });
        match crate::atomic_files::publish_no_replace(temp, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("destination_write_failed".into()),
        }
    }
    Err("destination_name_conflict".into())
}
fn copy_original(
    source: &Path,
    assets: &Path,
    directory: &Path,
    name: &str,
    op: &OperationGuard,
) -> ExportResult<PathBuf> {
    op.checkpoint()?;
    if fs::symlink_metadata(source)
        .map_err(|_| "source_missing")?
        .file_type()
        .is_symlink()
    {
        return Err("unsafe_source".into());
    }
    let source = source.canonicalize().map_err(|_| "source_missing")?;
    let assets = assets.canonicalize().map_err(|_| "source_missing")?;
    if !source.starts_with(&assets) {
        return Err("unsafe_source".into());
    }
    let mut original = File::open(&source).map_err(|_| "source_unreadable")?;
    if !original
        .metadata()
        .map_err(|_| "source_unreadable")?
        .is_file()
    {
        return Err("unsafe_source".into());
    }
    let temp = TempFile(directory.join(format!(".astro-export-{}.tmp", uuid::Uuid::new_v4())));
    let mut copy = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp.0)
        .map_err(|_| "destination_write_failed")?;
    let mut source_hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        op.checkpoint()?;
        let count = original
            .read(&mut buffer)
            .map_err(|_| "source_unreadable")?;
        if count == 0 {
            break;
        }
        source_hash.update(&buffer[..count]);
        copy.write_all(&buffer[..count])
            .map_err(|_| "destination_write_failed")?;
    }
    copy.sync_all().map_err(|_| "destination_write_failed")?;
    drop(copy);
    let mut verify = File::open(&temp.0).map_err(|_| "verification_failed")?;
    let mut destination_hash = Sha256::new();
    loop {
        op.checkpoint()?;
        let count = verify
            .read(&mut buffer)
            .map_err(|_| "verification_failed")?;
        if count == 0 {
            break;
        }
        destination_hash.update(&buffer[..count]);
    }
    if source_hash.finalize() != destination_hash.finalize() {
        return Err("verification_failed".into());
    }
    op.checkpoint()?;
    publish(&temp.0, directory, name)
}
fn scrub_recipe(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|key, _| {
                let k = key.to_ascii_lowercase();
                !k.contains("key")
                    && !k.contains("token")
                    && !k.contains("secret")
                    && !k.contains("password")
                    && !k.contains("authorization")
                    && !k.contains("endpoint")
                    && !k.contains("path")
                    && !k.contains("provider_profile")
                    && !k.contains("url")
            });
            for v in map.values_mut() {
                scrub_recipe(v);
            }
        }
        serde_json::Value::Array(values) => {
            for v in values {
                scrub_recipe(v);
            }
        }
        serde_json::Value::String(text)
            if text.starts_with('/')
                || text.starts_with("\\\\")
                || text.as_bytes().get(1) == Some(&b':') =>
        {
            *value = serde_json::Value::Null
        }
        _ => {}
    }
}
fn export_pending(
    session: &mut ExportSession,
    assets: &Path,
    op: &OperationGuard,
    progress: &dyn Fn(usize, usize),
) -> BatchExportResult {
    let mut succeeded = Vec::new();
    let mut failures = Vec::new();
    let mut cancelled_ids = Vec::new();
    let mut pending = Vec::new();
    for mut item in std::mem::take(&mut session.pending) {
        let outcome = (|| {
            op.checkpoint()?;
            if item.image_output.is_none() {
                item.image_output = Some(copy_original(
                    &item.source,
                    assets,
                    &session.directory,
                    &item.name,
                    op,
                )?);
            }
            if let Some(recipe) = &item.recipe {
                op.checkpoint()?;
                let image_path = item
                    .image_output
                    .as_ref()
                    .ok_or("destination_write_failed")?;
                let name = format!(
                    "{}.json",
                    image_path
                        .file_name()
                        .and_then(|v| v.to_str())
                        .ok_or("invalid_name")?
                );
                let temp = TempFile(
                    session
                        .directory
                        .join(format!(".astro-export-{}.tmp", uuid::Uuid::new_v4())),
                );
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temp.0)
                    .map_err(|_| "sidecar_failed")?;
                file.write_all(&serde_json::to_vec_pretty(recipe).map_err(|_| "sidecar_failed")?)
                    .map_err(|_| "sidecar_failed")?;
                file.sync_all().map_err(|_| "sidecar_failed")?;
                drop(file);
                op.checkpoint()?;
                publish(&temp.0, &session.directory, &name).map_err(|_| "sidecar_failed")?;
            }
            Ok::<(), String>(())
        })();
        match outcome {
            Ok(()) => {
                session.completed += 1;
                succeeded.push(item.image_id.clone());
            }
            Err(code) => {
                if code == "operation_cancelled" {
                    cancelled_ids.push(item.image_id.clone());
                } else {
                    failures.push(ExportFailure {
                        image_id: item.image_id.clone(),
                        generation_id: item.generation_id.clone(),
                        code,
                    });
                }
                pending.push(item);
            }
        }
        progress(session.completed, session.total);
    }
    session.pending = pending;
    BatchExportResult {
        total: session.total,
        completed: session.completed,
        succeeded_image_ids: succeeded,
        cancelled_image_ids: cancelled_ids,
        failed: failures,
        cancelled: op.cancelled(),
        retry_token: None,
    }
}
fn resolve_items(
    db: &Database,
    ids: &[String],
    include_recipe: bool,
) -> ExportResult<Vec<ExportItem>> {
    if ids.is_empty() || ids.len() > 10_000 {
        return Err("Choose between 1 and 10000 generation records.".into());
    }
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for id in ids.iter().filter(|id| seen.insert((*id).clone())) {
        let rows = {
            let conn = db.conn.lock().map_err(|_| "Library unavailable")?;
            let mut statement=conn.prepare("SELECT i.id,i.file_path,g.created_at FROM images i JOIN generations g ON g.id=i.generation_id WHERE g.id=?1 AND g.deleted_at IS NULL ORDER BY i.created_at,i.id").map_err(|_|"Cannot read selected images")?;
            let rows = statement
                .query_map([id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .map_err(|_| "Cannot read selected images")?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "Cannot read selected images")?;
            rows
        };
        if rows.is_empty() {
            return Err(
                "A selected generation no longer has available images. Refresh the selection."
                    .into(),
            );
        }
        let recipe = if include_recipe {
            let mut value = serde_json::to_value(
                crate::generation_recipe::load_recipe(db, id)
                    .map_err(|_| "Cannot read generation recipe")?,
            )
            .map_err(|_| "Cannot serialize recipe")?;
            scrub_recipe(&mut value);
            Some(value)
        } else {
            None
        };
        for (index, (image_id, path, date)) in rows.into_iter().enumerate() {
            let source = PathBuf::from(path);
            let extension = source.extension().and_then(|v| v.to_str()).unwrap_or("bin");
            let name = format!(
                "{}_{}_{}.{}",
                safe_piece(date.get(..10).unwrap_or(&date)),
                safe_piece(id),
                index + 1,
                safe_piece(extension)
            );
            items.push(ExportItem {
                image_id,
                generation_id: id.clone(),
                source,
                name,
                recipe: recipe.clone(),
                image_output: None,
            });
        }
    }
    Ok(items)
}
#[tauri::command]
#[allow(clippy::too_many_arguments)] // Stable IPC command parameters.
pub(crate) async fn export_generations(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    paths: State<'_, StoragePaths>,
    operations: State<'_, Operations>,
    sessions: State<'_, ExportSessions>,
    generation_ids: Vec<String>,
    include_recipe: bool,
    operation_id: String,
    retry_token: Option<String>,
) -> ExportResult<Option<BatchExportResult>> {
    let _permit = crate::storage::global_gate().write()?;
    let operation = operations.begin(&operation_id)?;
    let token = retry_token
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut session = if retry_token.is_some() {
        sessions
            .0
            .lock()
            .map_err(|_| "Export state unavailable")?
            .remove(&token)
            .ok_or("Export retry expired. Start a new export.")?
    } else {
        let items = resolve_items(db.inner(), &generation_ids, include_recipe)?;
        let Some(directory) = rfd::AsyncFileDialog::new()
            .set_title("Export original images")
            .pick_folder()
            .await
        else {
            return Ok(None);
        };
        let directory = directory
            .path()
            .canonicalize()
            .map_err(|_| "Destination folder unavailable")?;
        ExportSession {
            directory,
            pending: items.clone(),
            total: items.len(),
            completed: 0,
        }
    };
    let assets = paths.assets.clone();
    let (mut result, session) = tauri::async_runtime::spawn_blocking(move || {
        let result = export_pending(&mut session, &assets, &operation, &|done, total| {
            operation.progress(&app, "export", done, total)
        });
        (result, session)
    })
    .await
    .map_err(|_| "Export worker interrupted")?;
    if !session.pending.is_empty() {
        result.retry_token = Some(token.clone());
        let mut registry = sessions.0.lock().map_err(|_| "Export state unavailable")?;
        if registry.len() >= 32 {
            if let Some(old) = registry.keys().next().cloned() {
                registry.remove(&old);
            }
        }
        registry.insert(token, session);
    }
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("astro-export-{}", uuid::Uuid::new_v4()));
        let assets = root.join("assets");
        let output = root.join("output");
        fs::create_dir_all(&assets).unwrap();
        fs::create_dir_all(&output).unwrap();
        (root, assets, output)
    }
    #[test]
    fn export_preserves_original_bytes_and_existing_destination() {
        let (root, assets, output) = fixture();
        let source = assets.join("original.svg");
        fs::write(&source, b"<svg>original</svg>").unwrap();
        fs::write(output.join("2026-11-13_gen_1.svg"), b"existing").unwrap();
        let ops = Operations::default();
        let op = ops.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
        let path = copy_original(&source, &assets, &output, "2026-11-13_gen_1.svg", &op).unwrap();
        assert_ne!(path, output.join("2026-11-13_gen_1.svg"));
        assert_eq!(fs::read(path).unwrap(), b"<svg>original</svg>");
        assert_eq!(
            fs::read(output.join("2026-11-13_gen_1.svg")).unwrap(),
            b"existing"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancelled_export_publishes_nothing_and_scrubs_recipe_secrets_and_paths() {
        let (root, assets, output) = fixture();
        let source = assets.join("original.png");
        fs::write(&source, b"image").unwrap();
        let ops = Operations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let op = ops.begin(&id).unwrap();
        ops.cancel(&id).unwrap();
        assert!(copy_original(&source, &assets, &output, "test.png", &op).is_err());
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        let mut recipe = serde_json::json!({"prompt":"a circle", "api_key":"secret", "endpoint_snapshot":"https://secret.example/v1?key=secret", "source_image_paths":["/Users/me/source.png"],"advanced_options":{"maskPath":"C:\\Users\\secret.png"},"provider_profile_id":"credential-profile"});
        scrub_recipe(&mut recipe);
        let text = recipe.to_string();
        assert!(!text.contains("secret"));
        assert!(!text.contains("/Users"));
        assert!(!text.contains("credential-profile"));
        assert!(text.contains("a circle"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_failure_retry_skips_successful_image() {
        let (root, assets, output) = fixture();
        let first = assets.join("first.png");
        fs::write(&first, b"one").unwrap();
        let second = assets.join("second.gif");
        let ops = Operations::default();
        let op = ops.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
        let items = vec![
            ExportItem {
                image_id: "one".into(),
                generation_id: "g".into(),
                source: first,
                name: "g_1.png".into(),
                recipe: None,
                image_output: None,
            },
            ExportItem {
                image_id: "two".into(),
                generation_id: "g".into(),
                source: second.clone(),
                name: "g_2.gif".into(),
                recipe: None,
                image_output: None,
            },
        ];
        let mut session = ExportSession {
            directory: output.clone(),
            pending: items,
            total: 2,
            completed: 0,
        };
        let first = export_pending(&mut session, &assets, &op, &|_, _| {});
        assert_eq!(first.succeeded_image_ids, vec!["one"]);
        assert_eq!(first.failed.len(), 1);
        fs::write(second, b"two").unwrap();
        let retry = export_pending(&mut session, &assets, &op, &|_, _| {});
        assert_eq!(retry.succeeded_image_ids, vec!["two"]);
        assert_eq!(retry.completed, 2);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancellation_reports_untouched_ids_separately_and_keeps_completed_files() {
        let (root, assets, output) = fixture();
        let items = ["one", "two", "three"]
            .into_iter()
            .map(|id| {
                let source = assets.join(format!("{id}.png"));
                fs::write(&source, id).unwrap();
                ExportItem {
                    image_id: id.into(),
                    generation_id: "g".into(),
                    source,
                    name: format!("{id}.png"),
                    recipe: None,
                    image_output: None,
                }
            })
            .collect();
        let ops = Operations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let op = ops.begin(&id).unwrap();
        let mut session = ExportSession {
            directory: output.clone(),
            pending: items,
            total: 3,
            completed: 0,
        };
        let result = export_pending(&mut session, &assets, &op, &|done, _| {
            if done == 1 {
                ops.cancel(&id).unwrap();
            }
        });
        assert_eq!(result.succeeded_image_ids, vec!["one"]);
        assert_eq!(result.cancelled_image_ids, vec!["two", "three"]);
        assert!(result.failed.is_empty());
        assert_eq!(session.pending.len(), 2);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolving_generation_ids_includes_all_images_and_rejects_missing_records() {
        let (root, assets, _) = fixture();
        let db = Database::open(&root.join("library.db")).unwrap();
        db.run_migrations().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute("INSERT INTO generations (id,prompt,engine,status) VALUES ('g','circle','gpt-image-2','completed')", []).unwrap();
            for (id, name) in [("one", "one.png"), ("two", "two.svg")] {
                conn.execute(
                    "INSERT INTO images (id,generation_id,file_path) VALUES (?1,'g',?2)",
                    rusqlite::params![id, assets.join(name).to_string_lossy()],
                )
                .unwrap();
            }
        }
        let items = resolve_items(&db, &["g".into(), "g".into()], false).unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| item.recipe.is_none()));
        assert!(resolve_items(&db, &["missing".into()], false).is_err());
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn export_rejects_symlink_outside_assets() {
        let (root, assets, output) = fixture();
        let external = root.join("secret.png");
        fs::write(&external, b"secret").unwrap();
        std::os::unix::fs::symlink(&external, assets.join("link.png")).unwrap();
        let ops = Operations::default();
        let op = ops.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
        assert!(
            copy_original(&assets.join("link.png"), &assets, &output, "copy.png", &op).is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
