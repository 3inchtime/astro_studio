use crate::{
    db::Database,
    operations::{OperationGuard, Operations},
    storage::{MaintenanceGate, StoragePaths},
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::State;
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
const MAX_TOTAL: u64 = 128 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 200_000;
const MANIFEST_LIMIT: u64 = 64 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupFile {
    name: String,
    size: u64,
    sha256: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackupManifest {
    pub version: u32,
    created_at: String,
    app_version: String,
    schema_version: i64,
    pub generations: i64,
    pub images: i64,
    pub projects: i64,
    pub canvases: i64,
    files: Vec<BackupFile>,
}
#[derive(Serialize)]
pub(crate) struct BackupInspection {
    token: String,
    manifest: BackupManifest,
}
#[derive(Default)]
pub(crate) struct BackupInspections(Mutex<HashMap<String, (PathBuf, String, BackupManifest)>>);
#[derive(Serialize)]
pub(crate) struct LibraryOperationResult {
    status: &'static str,
    path: Option<String>,
}
struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn temporary(parent: &Path) -> Result<TempDir> {
    fs::create_dir_all(parent)?;
    let p = parent.join(format!(".backup-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&p)?;
    Ok(TempDir(p))
}
#[cfg(test)]
fn hash_file(path: &Path) -> Result<(u64, String)> {
    hash_file_checked(path, None)
}
fn hash_file_checked(path: &Path, op: Option<&OperationGuard>) -> Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut total = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        if let Some(op) = op {
            op.checkpoint()?;
        }
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        total += n as u64;
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
fn safe_entry(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0'])
        && !name.starts_with('/')
        && name
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}
fn path_key(key: &str) -> bool {
    matches!(
        key,
        "image_path"
            | "source_image_path"
            | "maskPath"
            | "markupPath"
            | "mask_path"
            | "markup_path"
            | "source_image_paths"
    )
}
fn map_json_paths(value: &mut Value, f: &mut impl FnMut(&str) -> Result<String>) -> Result<()> {
    match value {
        Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if path_key(key) {
                    if let Some(path) = child.as_str() {
                        if !path.is_empty() {
                            *child = Value::String(f(path)?);
                        }
                    } else if let Value::Array(items) = child {
                        for item in items {
                            if let Some(path) = item.as_str() {
                                *item = Value::String(f(path)?);
                            }
                        }
                    }
                } else {
                    map_json_paths(child, f)?;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                map_json_paths(item, f)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn credential_key(key: &str) -> bool {
    let key = key.to_lowercase().replace(['-', '_'], "");
    key.contains("password")
        || key.contains("secret")
        || key.contains("token")
        || key.contains("apikey")
        || key.contains("credential")
        || key.ends_with("signature")
        || matches!(
            key.as_str(),
            "key"
                | "accesskey"
                | "accesskeyid"
                | "appkey"
                | "subscriptionkey"
                | "ocpapimsubscriptionkey"
                | "authorization"
                | "auth"
                | "signature"
                | "sig"
        )
}
fn scrub(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "credential_ref"
                        | "credential_update"
                        | "api_key_configured"
                        | "api_key_masked"
                )
            });
            for (key, value) in map {
                let key = key.to_lowercase();
                if credential_key(&key) {
                    *value = Value::String(String::new());
                } else {
                    scrub(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                scrub(item);
            }
        }
        Value::String(s) => {
            if let Ok(mut url) = reqwest::Url::parse(s.trim()) {
                if !matches!(url.scheme(), "http" | "https") {
                    return;
                }
                let _ = url.set_username("");
                let _ = url.set_password(None);
                let pairs: Vec<(String, String)> = url
                    .query_pairs()
                    .filter(|(k, _)| !credential_key(k))
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
                url.set_query(None);
                url.set_fragment(None);
                if !pairs.is_empty() {
                    url.query_pairs_mut().extend_pairs(pairs);
                }
                *s = url.to_string();
            }
        }
        _ => {}
    }
}
fn json_column(
    conn: &Connection,
    table: &str,
    column: &str,
    f: &mut impl FnMut(&str) -> Result<String>,
) -> Result<()> {
    let rows = conn
        .prepare(&format!(
            "SELECT id,{column} FROM {table} WHERE {column} IS NOT NULL"
        ))?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, text) in rows {
        let mut value: Value = serde_json::from_str(&text)?;
        if column == "source_image_paths" {
            if let Value::Array(items) = &mut value {
                for item in items {
                    if let Some(p) = item.as_str() {
                        *item = Value::String(f(p)?);
                    }
                }
            }
        } else {
            map_json_paths(&mut value, f)?;
        }
        conn.execute(
            &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
            params![serde_json::to_string(&value)?, id],
        )?;
    }
    Ok(())
}
fn transform_database_paths(
    conn: &Connection,
    f: &mut impl FnMut(&str, bool) -> Result<String>,
    document: &mut impl FnMut(&str, &str) -> Result<()>,
) -> Result<()> {
    for (table, column) in [
        ("images", "file_path"),
        ("canvas_assets", "file_path"),
        ("images", "thumbnail_path"),
        ("canvas_documents", "document_path"),
        ("canvas_documents", "preview_path"),
        ("prompt_extractions", "image_path"),
    ] {
        let rows = conn
            .prepare(&format!(
                "SELECT id,{column} FROM {table} WHERE {column} IS NOT NULL AND {column}<>''"
            ))?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, old) in rows {
            let new = f(&old, false)?;
            if column == "document_path" {
                document(&old, &new)?;
            }
            conn.execute(
                &format!("UPDATE {table} SET {column}=?1 WHERE id=?2"),
                params![new, id],
            )?;
        }
    }
    for (table, column) in [
        ("generations", "source_image_paths"),
        ("generations", "request_metadata"),
        ("generation_jobs", "request_json"),
        ("generation_jobs", "source_ref_json"),
    ] {
        json_column(conn, table, column, &mut |p| f(p, false))?;
    }
    let rows = conn
        .prepare(
            "SELECT generation_id,response_file,expected_response_file FROM generation_recoveries",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, response, expected) in rows {
        let response = response
            .filter(|p| !p.is_empty())
            .map(|p| f(&p, false))
            .transpose()?;
        // Expected paths may not have a file (the request never returned).
        let expected = expected
            .filter(|p| !p.is_empty())
            .map(|p| f(&p, true))
            .transpose()?;
        conn.execute("UPDATE generation_recoveries SET response_file=?1,expected_response_file=?2 WHERE generation_id=?3",params![response,expected,id])?;
    }
    Ok(())
}
fn scrub_snapshot(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA secure_delete=ON;")?;
    conn.execute("DELETE FROM settings WHERE key IN ('credential_migration_pending','credential_cleanup_pending','credential_pending_deletes')", [])?;
    conn.execute("DELETE FROM logs", [])?;
    let rows = conn
        .prepare("SELECT key,value FROM settings")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (key, text) in rows {
        let mut v = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        if credential_key(&key) {
            v = Value::String(String::new());
        } else {
            scrub(&mut v);
        }
        let text = match v {
            Value::String(s) => s,
            other => serde_json::to_string(&other)?,
        };
        conn.execute(
            "UPDATE settings SET value=?1 WHERE key=?2",
            params![text, key],
        )?;
    }
    conn.execute("UPDATE generation_worker_lease SET owner_id=NULL,acquired_at=NULL,heartbeat_at=NULL,expires_at=NULL WHERE owner_id IS NOT NULL",[])?;
    // Private URL material must not leak through historical endpoints either.
    let rows = conn
        .prepare("SELECT id,endpoint_snapshot FROM generation_jobs")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, url) in rows {
        let mut v = Value::String(url);
        scrub(&mut v);
        conn.execute(
            "UPDATE generation_jobs SET endpoint_snapshot=?1 WHERE id=?2",
            params![v.as_str(), id],
        )?;
    }
    Ok(())
}
fn prepare_snapshot_files(
    conn: &Connection,
    paths: &StoragePaths,
    temp: &Path,
    op: &OperationGuard,
) -> Result<BTreeMap<String, PathBuf>> {
    use base64::Engine;
    let mut overrides = BTreeMap::new();
    let images = conn
        .prepare("SELECT id,file_path,thumbnail_path FROM images")?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, source, thumb) in images {
        op.checkpoint()?;
        if !Path::new(&source).is_file() {
            return Err(format!("Original image is missing: {source}").into());
        }
        if !thumb.as_deref().is_some_and(|p| Path::new(p).is_file()) {
            let regenerated = temp.join(format!("{}.png", uuid::Uuid::new_v4()));
            crate::file_manager::decode_image_for_preview(&fs::read(&source)?)?
                .thumbnail(256, 256)
                .save(&regenerated)?;
            conn.execute(
                "UPDATE images SET thumbnail_path=?1 WHERE id=?2",
                params![regenerated.to_string_lossy(), id],
            )?;
        }
    }
    let previews = conn
        .prepare("SELECT id,preview_path FROM canvas_documents WHERE preview_path IS NOT NULL")?
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, path) in previews {
        if !Path::new(&path).is_file() {
            conn.execute(
                "UPDATE canvas_documents SET preview_path=NULL WHERE id=?1",
                [id],
            )?;
        }
    }
    let responses = conn.prepare(
        "SELECT r.generation_id,r.response_file,r.expected_response_file,j.id,
                r.request_state,r.response_size,r.response_sha256
         FROM generation_recoveries r LEFT JOIN generation_jobs j ON j.generation_id=r.generation_id"
    )?.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,Option<String>>(1)?,
        r.get::<_,Option<String>>(2)?, r.get::<_,Option<String>>(3)?, r.get::<_,String>(4)?,
        r.get::<_,Option<i64>>(5)?, r.get::<_,Option<String>>(6)?)))?
        .collect::<std::result::Result<Vec<_>,_>>()?;
    for (generation_id, response, expected, job_id, state, size, sha256) in responses {
        op.checkpoint()?;
        let Some(path) = response
            .clone()
            .or_else(|| expected.clone().filter(|p| Path::new(p).is_file()))
        else {
            continue;
        };
        let (body, requested_count) = if let Some(job_id) = job_id {
            use crate::generation_lifecycle::{
                FileResponseArtifactStore, GenerationExecutionContext,
            };
            use crate::models::{GenerationJobStage, GenerationJobStatus};
            let job = crate::generation_jobs::get_job(conn, &job_id)?;
            let request: crate::generation_jobs::GenerationJobRequest =
                serde_json::from_value(job.request)?;
            let (conversation_id, count): (String, u8) = conn.query_row(
                "SELECT conversation_id,image_count FROM generations WHERE id=?1",
                [&generation_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let context = GenerationExecutionContext {
                generation_id: generation_id.clone(),
                job_id,
                conversation_id,
                provider_kind: job.provider_kind,
                model: request.model,
                endpoint_url: job.endpoint_snapshot,
                provider_profile_id: job.provider_profile_id,
            };
            let store = FileResponseArtifactStore::new(paths.assets.clone());
            let expected_path = store
                .response_path(&context)
                .map_err(|_| "Invalid saved response identity")?;
            if expected.as_deref().map(Path::new) != Some(expected_path.as_path())
                || Path::new(&path) != expected_path
            {
                return Err("Saved response does not belong to its generation".into());
            }
            let verified = tauri::async_runtime::block_on(
                store.load_verified_response(&context, Path::new(&path)),
            )
            .map_err(|_| "Saved response failed artifact verification")?;
            if verified.requested_image_count != count {
                return Err("Saved response image count disagrees with its generation".into());
            }
            match state.as_str() {
                "response_ready"
                    if response.as_deref() == Some(path.as_str())
                        && size == Some(verified.response_size as i64)
                        && sha256.as_deref() == Some(verified.response_sha256.as_str())
                        && ((job.status == GenerationJobStatus::Running
                            && matches!(
                                job.stage,
                                GenerationJobStage::ResponseReady
                                    | GenerationJobStage::LocalProcessing
                            ))
                            || (matches!(
                                job.status,
                                GenerationJobStatus::Failed | GenerationJobStatus::Interrupted
                            ) && job.stage == GenerationJobStage::Terminal)) => {}
                "requesting"
                    if response.is_none()
                        && size.is_none()
                        && sha256.is_none()
                        && ((job.status == GenerationJobStatus::Running
                            && matches!(
                                job.stage,
                                GenerationJobStage::ProviderRequest
                                    | GenerationJobStage::RetryBackoff
                            ))
                            || (matches!(
                                job.status,
                                GenerationJobStatus::Failed | GenerationJobStatus::Interrupted
                            ) && job.stage == GenerationJobStage::Terminal
                                && job.cancel_requested_at.is_none())) => {}
                _ => {
                    return Err(
                        "Saved response metadata disagrees with the generation stage".into(),
                    )
                }
            }
            (verified.body_text, Some(verified.requested_image_count))
        } else {
            let canonical = Path::new(&path).canonicalize()?;
            if !canonical.starts_with(paths.assets.canonicalize()?) {
                return Err("Recovery response is outside the library".into());
            }
            let maximum = crate::api_gateway::MAX_PROVIDER_RESPONSE_BODY_BYTES;
            let mut bytes = Vec::new();
            File::open(canonical)?
                .take(maximum as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > maximum {
                return Err("Saved response exceeds the supported size".into());
            }
            (String::from_utf8(bytes)?, None)
        };
        // Normalize only verified data into offline image bytes. Requesting jobs
        // keep their pre-promotion state so startup can perform the fenced commit.
        let engine = crate::api_gateway::GptImageEngine::new(&crate::config::ApiConfig {
            timeout_secs: 30,
            max_retries: 0,
        })?;
        let mut images = tauri::async_runtime::block_on(
            engine.decode_images_from_response(&body, &|| op.cancelled()),
        )?;
        if let Some(count) = requested_count {
            images.truncate(usize::from(count));
        }
        if images.is_empty() {
            return Err("Saved response contains no recoverable image".into());
        }
        let normalized = serde_json::to_string(
            &serde_json::json!({"data":images.into_iter().map(|bytes|
            serde_json::json!({"b64_json":base64::engine::general_purpose::STANDARD.encode(bytes)})).collect::<Vec<_>>()}),
        )?;
        if normalized.len() > crate::api_gateway::MAX_PROVIDER_RESPONSE_BODY_BYTES {
            return Err("Normalized saved response exceeds the supported size".into());
        }
        let hash = format!("{:x}", Sha256::digest(normalized.as_bytes()));
        let output = if let Some(count) = requested_count {
            serde_json::to_vec(
                &serde_json::json!({"version":1,"body_text":normalized,"body_size":normalized.len(),
                "body_sha256":hash,"requested_image_count":count}),
            )?
        } else {
            normalized.as_bytes().to_vec()
        };
        if output.len() > crate::api_gateway::MAX_PROVIDER_RESPONSE_BODY_BYTES.saturating_mul(2) {
            return Err("Normalized response envelope exceeds the supported size".into());
        }
        let rewritten = temp.join(format!("{}.response.json", uuid::Uuid::new_v4()));
        fs::write(&rewritten, output)?;
        let relative = Path::new(&path)
            .strip_prefix(&paths.assets)?
            .to_string_lossy()
            .replace('\\', "/");
        overrides.insert(format!("assets/{relative}"), rewritten);
        if requested_count.is_some() && state == "response_ready" {
            conn.execute("UPDATE generation_recoveries SET response_size=?1,response_sha256=?2 WHERE generation_id=?3",
                params![normalized.len() as i64, hash, generation_id])?;
        }
    }
    Ok(overrides)
}

fn create_archive(
    db: &Database,
    paths: &StoragePaths,
    target: &Path,
    op: &OperationGuard,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<BackupManifest> {
    let temp = temporary(&paths.bootstrap)?;
    let snapshot = temp.0.join("library.db");
    {
        let conn = db.conn.lock().map_err(|_| "Database unavailable")?;
        let mut copy = Connection::open(&snapshot)?;
        let backup = rusqlite::backup::Backup::new(&conn, &mut copy)?;
        loop {
            op.checkpoint()?;
            match backup.step(128)? {
                rusqlite::backup::StepResult::Done => break,
                rusqlite::backup::StepResult::More => {}
                _ => {
                    return Err(
                        "Database snapshot is busy; try again after active writes finish".into(),
                    )
                }
            }
        }
    }
    let conn = Connection::open(&snapshot)?;
    let overrides = prepare_snapshot_files(&conn, paths, &temp.0, op)?;
    scrub_snapshot(&conn)?;
    let mut files: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut mapping: HashMap<String, String> = HashMap::new();
    let mut documents = Vec::new();
    let root = paths.assets.canonicalize()?;
    {
        let mut map = |old: &str, optional: bool| -> Result<String> {
            op.checkpoint()?;
            if let Some(mapped) = mapping.get(old) {
                return Ok(mapped.clone());
            }
            let p = Path::new(old);
            let relative = if p.starts_with(&paths.assets) {
                p.strip_prefix(&paths.assets)?
                    .to_string_lossy()
                    .replace('\\', "/")
            } else {
                let (_, sha) = hash_file_checked(p, Some(op))?;
                format!(
                    "imports/{sha}.{}",
                    p.extension().and_then(|s| s.to_str()).unwrap_or("bin")
                )
            };
            let name = format!("assets/{relative}");
            if !safe_entry(&name) {
                return Err("Invalid referenced asset path".into());
            }
            if p.is_file() {
                let canonical = p.canonicalize()?;
                if p.starts_with(&paths.assets) && !canonical.starts_with(&root) {
                    return Err("Asset symlink escapes library".into());
                }
                files.insert(name.clone(), canonical);
            } else if !optional {
                return Err(format!("Referenced file is missing: {}", p.display()).into());
            }
            mapping.insert(old.into(), name.clone());
            Ok(name)
        };
        transform_database_paths(&conn, &mut map, &mut |old, new| {
            documents.push((old.to_owned(), new.to_owned()));
            Ok(())
        })?;
    }
    // Canvas paths are structured fields, never replacements in prompts or text objects.
    for (old, new) in documents {
        let mut content: Value = serde_json::from_slice(&fs::read(&old)?)?;
        map_json_paths(&mut content, &mut |p| {
            if !Path::new(p).is_file() {
                return Err(format!("Referenced canvas asset is missing: {p}").into());
            }
            if let Some(mapped) = mapping.get(p) {
                return Ok(mapped.clone());
            }
            let source = Path::new(p);
            let (_, sha) = hash_file_checked(source, Some(op))?;
            let name = format!(
                "assets/imports/{sha}.{}",
                source.extension().and_then(|e| e.to_str()).unwrap_or("bin")
            );
            files.insert(name.clone(), source.to_path_buf());
            mapping.insert(p.into(), name.clone());
            Ok(name)
        })?;
        let rewritten = temp.0.join(format!("{}.json", uuid::Uuid::new_v4()));
        fs::write(&rewritten, serde_json::to_vec(&content)?)?;
        files.insert(new, rewritten);
    }
    conn.execute_batch("VACUUM;")?;
    let count = |table: &str| -> Result<i64> {
        Ok(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?)
    };
    let mut manifest = BackupManifest {
        version: 1,
        created_at: crate::current_timestamp(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
            r.get(0)
        })?,
        generations: count("generations")?,
        images: count("images")?,
        projects: count("projects")?,
        canvases: count("canvas_documents")?,
        files: vec![],
    };
    drop(conn);
    files.extend(overrides);
    files.insert("library.db".into(), snapshot);
    if files.len() > MAX_FILES {
        return Err("Too many files for one backup".into());
    }
    let parent = target.parent().ok_or("Invalid backup destination")?;
    fs::create_dir_all(parent)?;
    let required = files
        .values()
        .try_fold(0u64, |sum, p| -> std::io::Result<u64> {
            Ok(sum.saturating_add(fs::metadata(p)?.len()))
        })?;
    if fs2::available_space(parent)? < required.saturating_add(16 * 1024 * 1024) {
        return Err("Not enough disk space for a complete backup".into());
    }
    let staging = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        let mut zip = ZipWriter::new(output);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .large_file(true);
        let mut total = 0;
        for (name, path) in &files {
            op.checkpoint()?;
            let (size, sha256) = hash_file_checked(path, Some(op))?;
            total += size;
            if total > MAX_TOTAL {
                return Err("Backup exceeds supported size".into());
            }
            zip.start_file(name, options)?;
            let mut input = File::open(path)?;
            let mut hash = Sha256::new();
            let mut buffer = [0; 65536];
            let mut copied = 0;
            loop {
                op.checkpoint()?;
                let n = input.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buffer[..n])?;
                hash.update(&buffer[..n]);
                copied += n as u64;
            }
            if copied != size || format!("{:x}", hash.finalize()) != sha256 {
                return Err("An asset changed during backup. Retry after saving it.".into());
            }
            manifest.files.push(BackupFile {
                name: name.clone(),
                size,
                sha256,
            });
            progress("backup", manifest.files.len(), files.len());
        }
        zip.start_file("manifest.json", options)?;
        zip.write_all(&serde_json::to_vec(&manifest)?)?;
        zip.finish()?.sync_all()?;
        inspect_archive_checked(&staging, Some(op))?;
        op.checkpoint()?;
        // Atomic publication without overwriting an existing backup.
        crate::atomic_files::publish_no_replace(&staging, target)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(staging);
    }
    result?;
    Ok(manifest)
}
#[cfg(test)]
fn inspect_archive(path: &Path) -> Result<BackupManifest> {
    inspect_archive_checked(path, None)
}
fn inspect_archive_checked(path: &Path, op: Option<&OperationGuard>) -> Result<BackupManifest> {
    let mut zip = ZipArchive::new(File::open(path)?)?;
    if zip.len() > MAX_FILES + 1 {
        return Err("Archive has too many files".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        if let Some(op) = op {
            op.checkpoint()?;
        }
        let entry = zip.by_index(i)?;
        if !safe_entry(entry.name())
            || !(entry.name() == "library.db"
                || entry.name() == "manifest.json"
                || entry.name().starts_with("assets/"))
            || entry.is_dir()
            || entry.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || !names.insert(entry.name().to_owned())
        {
            return Err("Unsafe or duplicate archive entry".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Archive size overflow")?;
        if total > MAX_TOTAL {
            return Err("Archive exceeds size limit".into());
        }
    }
    let mut bytes = Vec::new();
    zip.by_name("manifest.json")?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err("Manifest too large".into());
    }
    let manifest: BackupManifest = serde_json::from_slice(&bytes)?;
    if manifest.version != 1 {
        return Err("Unsupported backup format version".into());
    }
    if manifest.schema_version > 20 {
        return Err("Backup was created by a newer application".into());
    }
    if manifest.files.len() + 1 != names.len() || !names.contains("library.db") {
        return Err("Incomplete backup manifest".into());
    }
    let mut described = HashSet::new();
    for expected in &manifest.files {
        if !described.insert(&expected.name)
            || !names.contains(&expected.name)
            || expected.name == "manifest.json"
        {
            return Err("Invalid backup file list".into());
        }
        let mut entry = zip.by_name(&expected.name)?;
        if entry.size() != expected.size {
            return Err("Backup file size mismatch".into());
        }
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            if let Some(op) = op {
                op.checkpoint()?;
            }
            let n = entry.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        if format!("{:x}", hash.finalize()) != expected.sha256 {
            return Err("Backup checksum mismatch".into());
        }
    }
    Ok(manifest)
}
fn restore_approved_archive(
    paths: &StoragePaths,
    archive: &Path,
    approved: &BackupManifest,
    op: &OperationGuard,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<PathBuf> {
    let current = inspect_archive_checked(archive, Some(op))?;
    if &current != approved {
        return Err("Backup changed after inspection; inspect it again".into());
    }
    // Retain the reviewed file list and hashes through extraction. Reopening an
    // externally replaceable archive must never select a new manifest.
    restore_archive_manifest(paths, archive, approved, op, progress)
}
#[cfg(test)]
fn restore_archive(
    paths: &StoragePaths,
    archive: &Path,
    op: &OperationGuard,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<PathBuf> {
    let manifest = inspect_archive_checked(archive, Some(op))?;
    restore_archive_manifest(paths, archive, &manifest, op, progress)
}
fn restore_archive_manifest(
    paths: &StoragePaths,
    archive: &Path,
    manifest: &BackupManifest,
    op: &OperationGuard,
    progress: &dyn Fn(&str, usize, usize),
) -> Result<PathBuf> {
    op.checkpoint()?;
    let parent = paths.bootstrap.join("libraries");
    fs::create_dir_all(&parent)?;
    let needed: u64 = manifest.files.iter().map(|f| f.size).sum();
    if fs2::available_space(&parent)? < needed.saturating_add(64 * 1024 * 1024) {
        return Err("Not enough disk space to restore this library".into());
    }
    let destination = parent.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&destination)?;
    let result = (|| -> Result<()> {
        let mut zip = ZipArchive::new(File::open(archive)?)?;
        for (i, file) in manifest.files.iter().enumerate() {
            op.checkpoint()?;
            let path = destination.join(&file.name);
            fs::create_dir_all(path.parent().ok_or("Invalid file path")?)?;
            let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
            let mut input = zip.by_name(&file.name)?;
            let mut buf = [0; 65536];
            let mut hash = Sha256::new();
            let mut copied = 0u64;
            loop {
                op.checkpoint()?;
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                copied += n as u64;
                if copied > file.size {
                    return Err("Extracted file exceeds manifest size".into());
                }
                hash.update(&buf[..n]);
                output.write_all(&buf[..n])?;
            }
            if copied != file.size || format!("{:x}", hash.finalize()) != file.sha256 {
                return Err("Extracted file checksum mismatch".into());
            }
            output.sync_all()?;
            progress("restore", i + 1, manifest.files.len());
        }
        fs::create_dir_all(destination.join("assets"))?;
        let database = Database::open(&destination.join("library.db"))?;
        database.run_migrations()?;
        let conn = database.conn.lock().map_err(|_| "Database unavailable")?;
        let mut docs = Vec::new();

        let mut map = |p: &str, optional: bool| -> Result<String> {
            if !safe_entry(p) || !p.starts_with("assets/") {
                return Err("Backup contains an unmanaged file reference".into());
            }
            if !destination.join(p).is_file() && !optional {
                return Err(format!("Required backup asset is missing: {p}").into());
            }
            Ok(destination.join(p).to_string_lossy().into_owned())
        };
        transform_database_paths(&conn, &mut map, &mut |_, new| {
            docs.push(new.to_owned());
            Ok(())
        })?;
        for doc in docs {
            let mut value: Value = serde_json::from_slice(&fs::read(&doc)?)?;
            map_json_paths(&mut value, &mut |p| map(p, false))?;
            crate::storage::atomic_write(Path::new(&doc), &serde_json::to_vec(&value)?)?;
        }
        scrub_snapshot(&conn)?;
        let check: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if check != "ok" {
            return Err("Restored database failed integrity check".into());
        }
        let foreign_errors: i64 =
            conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })?;
        if foreign_errors != 0 {
            return Err("Restored database has broken references".into());
        }
        conn.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")?;
        drop(conn);
        drop(database);
        sync_restored_tree(&destination, op)?;
        op.checkpoint()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    Ok(destination)
}

fn sync_restored_tree(root: &Path, op: &OperationGuard) -> Result<()> {
    for entry in fs::read_dir(root)? {
        op.checkpoint()?;
        let path = entry?.path();
        if path.is_dir() {
            sync_restored_tree(&path, op)?;
        } else {
            // Windows FlushFileBuffers requires a writable file handle.
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)?
                .sync_all()?;
        }
    }
    crate::file_manager::sync_directory(root)?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn create_library_backup(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    paths: State<'_, StoragePaths>,
    gate: State<'_, MaintenanceGate>,
    operations: State<'_, Operations>,
    operation_id: String,
) -> std::result::Result<LibraryOperationResult, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name(format!(
            "astro-{}.astrobackup",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ))
        .add_filter("Astro Studio backup", &["astrobackup"])
        .save_file()
        .await
    else {
        return Ok(LibraryOperationResult {
            status: "cancelled",
            path: None,
        });
    };
    let permit = gate.maintain()?;
    let op = operations.begin(&operation_id)?;
    let database = db.inner().clone();
    let paths = paths.inner().clone();
    let target = file.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        match create_archive(&database, &paths, &target, &op, &|phase, n, total| {
            op.progress(&app, phase, n, total)
        }) {
            Ok(_) => Ok(LibraryOperationResult {
                status: "completed",
                path: Some(target.to_string_lossy().into_owned()),
            }),
            Err(e) if e.to_string() == "operation_cancelled" => Ok(LibraryOperationResult {
                status: "cancelled",
                path: None,
            }),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) async fn inspect_library_backup(
    inspections: State<'_, BackupInspections>,
    operations: State<'_, Operations>,
    operation_id: Option<String>,
) -> std::result::Result<Option<BackupInspection>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("Astro Studio backup", &["astrobackup"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let op = operations.begin(&operation_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))?;
    let path = file.path().to_path_buf();
    let (manifest, hash) = tokio::task::spawn_blocking({
        let path = path.clone();
        move || -> Result<_> {
            Ok((
                inspect_archive_checked(&path, Some(&op))?,
                hash_file_checked(&path, Some(&op))?.1,
            ))
        }
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let token = uuid::Uuid::new_v4().to_string();
    let mut entries = inspections
        .0
        .lock()
        .map_err(|_| "Backup inspection unavailable")?;
    entries.clear();
    entries.insert(token.clone(), (path, hash, manifest.clone()));
    Ok(Some(BackupInspection { token, manifest }))
}
#[tauri::command]
pub(crate) async fn restore_library_backup(
    app: tauri::AppHandle,
    paths: State<'_, StoragePaths>,
    gate: State<'_, MaintenanceGate>,
    operations: State<'_, Operations>,
    inspections: State<'_, BackupInspections>,
    token: String,
    operation_id: String,
) -> std::result::Result<LibraryOperationResult, String> {
    let (archive, expected_hash, approved) = inspections
        .0
        .lock()
        .map_err(|_| "Backup inspection unavailable")?
        .get(&token)
        .cloned()
        .ok_or("Inspect the backup before restoring")?;
    let permit = gate.maintain()?;
    let op = operations.begin(&operation_id)?;
    let paths = paths.inner().clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let result = (|| -> Result<_> {
            if hash_file_checked(&archive, Some(&op))?.1 != expected_hash {
                return Err("Backup changed after inspection".into());
            }
            let root =
                restore_approved_archive(&paths, &archive, &approved, &op, &|phase, n, total| {
                    op.progress(&app, phase, n, total)
                })?;
            op.checkpoint()?;
            paths.activate(&root)?;
            crate::storage::global_gate().require_restart();
            Ok(LibraryOperationResult {
                status: "restart_required",
                path: None,
            })
        })();
        result.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub(crate) fn rollback_library_restore(
    paths: State<'_, StoragePaths>,
    gate: State<'_, MaintenanceGate>,
) -> std::result::Result<LibraryOperationResult, String> {
    let _permit = gate.maintain()?;
    paths.rollback()?;
    crate::storage::global_gate().require_restart();
    Ok(LibraryOperationResult {
        status: "restart_required",
        path: None,
    })
}
#[tauri::command]
pub(crate) fn can_rollback_library_restore(paths: State<'_, StoragePaths>) -> bool {
    paths.previous.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_restores_actual_assets_and_scrubs_secrets_without_touching_original() {
        let root = std::env::temp_dir().join(format!("backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("assets/images")).unwrap();
        let paths = StoragePaths::new(
            root.join("bootstrap"),
            root.join("original.db"),
            root.join("assets"),
        )
        .unwrap();
        let db = Database::open(&paths.database).unwrap();
        db.run_migrations().unwrap();
        let image = paths.assets.join("images/test.png");
        fs::write(&image, b"original-image-bytes").unwrap();
        db.set_setting("api_key", "sk-backup-secret-marker")
            .unwrap();
        db.set_setting("model_provider_profiles::gpt-image-2", r#"[{"id":"test","credential_ref":"vault:v1:backup-private-reference","api_key_configured":true,"api_key_masked":"masked"}]"#).unwrap();
        db.set_setting(
            "credential_migration_pending",
            r#"{"test":"vault:v1:pending-private-reference"}"#,
        )
        .unwrap();
        db.conn.lock().unwrap().execute("INSERT INTO generations(id,prompt,engine,size,quality,status) VALUES('g','keep this prompt','gpt-image-2','1024x1024','high','completed')", []).unwrap();
        db.conn.lock().unwrap().execute("INSERT INTO images(id,generation_id,file_path,thumbnail_path,width,height,file_size) VALUES('i','g',?1,?1,1,1,20)", [image.to_string_lossy().as_ref()]).unwrap();
        let ops = Operations::default();
        let operation = ops.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
        let archive = root.join("test.astrobackup");
        create_archive(&db, &paths, &archive, &operation, &|_, _, _| {}).unwrap();
        let manifest = inspect_archive(&archive).unwrap();
        assert_eq!(manifest.version, 1);
        let restored = restore_archive(&paths, &archive, &operation, &|_, _, _| {}).unwrap();
        let copy = Database::open(&restored.join("library.db")).unwrap();
        assert_eq!(
            db.get_setting("api_key").unwrap().as_deref(),
            Some("sk-backup-secret-marker")
        );
        assert_eq!(copy.get_setting("api_key").unwrap().as_deref(), Some(""));
        assert!(copy
            .get_setting("credential_migration_pending")
            .unwrap()
            .is_none());
        let metadata = copy
            .get_setting("model_provider_profiles::gpt-image-2")
            .unwrap()
            .unwrap();
        assert!(!metadata.contains("credential_ref"));
        assert!(!metadata.contains("api_key_configured"));
        let bytes = fs::read(restored.join("library.db")).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("backup-private-reference"));
        assert!(!String::from_utf8_lossy(&bytes).contains("pending-private-reference"));
        let restored_path: String = copy
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT file_path FROM images WHERE id='i'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(restored_path.starts_with(restored.to_string_lossy().as_ref()));
        assert_eq!(fs::read(restored_path).unwrap(), b"original-image-bytes");
        assert!(!fs::read(restored.join("library.db"))
            .unwrap()
            .windows(b"sk-backup-secret-marker".len())
            .any(|w| w == b"sk-backup-secret-marker"));
        drop(copy);
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn credential_urls_and_nested_profiles_are_scrubbed() {
        let mut value = serde_json::json!({"api_key":"sensitive-key", "endpoint_settings":{"base_url":"https://user:pass@example.com/v1?client_secret=sensitive-query#token=fragment"},"nested":{"x-api-key":"sensitive-header"}});
        scrub(&mut value);
        let json = value.to_string();
        assert!(!json.contains("sensitive"));
        assert!(!json.contains("fragment"));
        assert!(!json.contains("user:pass"));
    }
    #[test]
    fn restart_required_prevents_further_writes() {
        let gate = MaintenanceGate::default();
        let permit = gate.maintain().unwrap();
        gate.require_restart();
        drop(permit);
        assert!(gate.write().is_err());
        assert!(gate.maintain().is_err());
    }
    #[test]
    fn traversal_archive_entries_are_rejected() {
        assert!(!safe_entry("../outside"));
        assert!(!safe_entry("/etc/passwd"));
        assert!(!safe_entry("C:\\outside"));
        assert!(!safe_entry("assets/../../outside"));
        assert!(safe_entry("assets/images/example.png"));
    }
}

#[cfg(test)]
#[path = "library_backup_tests.rs"]
mod integration_tests;

#[cfg(test)]
#[path = "library_backup_recovery_tests.rs"]
mod recovery_tests;
