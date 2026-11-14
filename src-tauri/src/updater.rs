use reqwest::{ClientBuilder, StatusCode, Url};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    CloseGuard(String),
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error("updates are only supported on Windows")]
    UnsupportedPlatform,
    #[error("there is no pending update")]
    NoPendingUpdate,
    #[error("{0}")]
    LibraryBusy(String),
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}

type Result<T> = std::result::Result<T, Error>;

const UPDATER_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename_all = "camelCase")]
    Started {
        content_length: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Progress {
        chunk_length: usize,
        total_downloaded: u64,
    },
    Finished,
}

#[derive(Serialize)]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

pub struct PendingUpdate(pub Mutex<Option<Update>>);
struct PreparedUpdate<T> {
    token: String,
    update: T,
    bytes: Vec<u8>,
}
#[derive(Default)]
pub struct DownloadedUpdate(Mutex<Option<PreparedUpdate<Update>>>);

fn install_prepared<T>(
    gate: &crate::storage::MaintenanceGate,
    prepared: &Mutex<Option<PreparedUpdate<T>>>,
    token: &str,
    install: impl FnOnce(&T, &[u8]) -> Result<()>,
) -> Result<()> {
    // Windows installers call process::exit and bypass RunEvent::ExitRequested.
    // Exclude generation/backup/restore writes before entering that irreversible path.
    let _maintenance = gate.maintain().map_err(Error::LibraryBusy)?;
    let mut ready = prepared.lock().map_err(|_| Error::NoPendingUpdate)?;
    let item = ready
        .as_ref()
        .filter(|item| item.token == token)
        .ok_or(Error::NoPendingUpdate)?;
    install(&item.update, &item.bytes)?;
    *ready = None;
    Ok(())
}

fn updater_supported_on_current_platform() -> bool {
    cfg!(target_os = "windows")
}

fn is_missing_update_manifest(status: StatusCode) -> bool {
    status == StatusCode::NOT_FOUND
}

fn update_manifest_endpoints(app: &AppHandle) -> Vec<Url> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("endpoints"))
        .and_then(|endpoints| endpoints.as_array())
        .into_iter()
        .flatten()
        .filter_map(|endpoint| endpoint.as_str())
        .filter_map(|endpoint| endpoint.parse().ok())
        .collect()
}

async fn manifest_exists(endpoint: &Url) -> Result<bool> {
    let response = ClientBuilder::new()
        .user_agent(UPDATER_USER_AGENT)
        .build()?
        .head(endpoint.clone())
        .send()
        .await?;

    Ok(!is_missing_update_manifest(response.status()))
}

#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending_update: State<'_, PendingUpdate>,
) -> Result<Option<UpdateMetadata>> {
    if !updater_supported_on_current_platform() {
        *pending_update.0.lock().unwrap() = None;
        return Ok(None);
    }

    for endpoint in update_manifest_endpoints(&app) {
        if !manifest_exists(&endpoint).await? {
            *pending_update.0.lock().unwrap() = None;
            return Ok(None);
        }
    }

    let update = app.updater()?.check().await?;

    let update_metadata = update.as_ref().map(|update| UpdateMetadata {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        body: update.body.clone(),
        date: update.date.map(|d| d.to_string()),
    });

    *pending_update.0.lock().unwrap() = update;

    Ok(update_metadata)
}

#[tauri::command]
pub fn is_update_supported() -> bool {
    updater_supported_on_current_platform()
}

#[tauri::command]
pub async fn download_update(
    downloaded: State<'_, DownloadedUpdate>,
    pending_update: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<String> {
    if !updater_supported_on_current_platform() {
        *pending_update.0.lock().unwrap() = None;
        return Err(Error::UnsupportedPlatform);
    }

    let Some(update) = pending_update.0.lock().unwrap().clone() else {
        return Err(Error::NoPendingUpdate);
    };

    if let Some(ready) = downloaded
        .0
        .lock()
        .map_err(|_| Error::NoPendingUpdate)?
        .as_ref()
    {
        if ready.update.version == update.version
            && ready.update.download_url == update.download_url
        {
            return Ok(ready.token.clone());
        }
    }
    let mut started = false;
    let mut total_downloaded: u64 = 0;

    let bytes = update
        .download(
            |chunk_length, content_length| {
                if !started {
                    let _ = on_event.send(DownloadEvent::Started { content_length });
                    started = true;
                }

                total_downloaded += chunk_length as u64;
                let _ = on_event.send(DownloadEvent::Progress {
                    chunk_length,
                    total_downloaded,
                });
            },
            || {},
        )
        .await?;

    let token = uuid::Uuid::new_v4().to_string();
    *downloaded.0.lock().map_err(|_| Error::NoPendingUpdate)? = Some(PreparedUpdate {
        token: token.clone(),
        update,
        bytes,
    });
    let _ = on_event.send(DownloadEvent::Finished);
    Ok(token)
}

#[tauri::command]
pub fn install_update(downloaded: State<'_, DownloadedUpdate>, token: String) -> Result<()> {
    if !updater_supported_on_current_platform() {
        return Err(Error::UnsupportedPlatform);
    }
    install_prepared(
        crate::storage::global_gate(),
        &downloaded.0,
        &token,
        |update, bytes| update.install(bytes).map_err(Error::from),
    )
}

#[tauri::command]
pub async fn relaunch_app(app: AppHandle) -> Result<()> {
    if crate::canvas_close::guarded(&app) {
        crate::canvas_close::request(&app, "restart", None).map_err(Error::CloseGuard)?;
        Ok(())
    } else {
        app.restart();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    #[test]
    fn metadata_matches_frontend_current_version_field() {
        let value = serde_json::to_value(UpdateMetadata {
            version: "2.0.0".into(),
            current_version: "1.0.0".into(),
            body: None,
            date: None,
        })
        .unwrap();
        assert_eq!(value["current_version"], "1.0.0");
        assert!(value.get("currentVersion").is_none());
    }

    #[test]
    fn install_handoff_rejects_active_operations_and_retries_verified_bytes() {
        let gate = crate::storage::MaintenanceGate::default();
        let ready = Mutex::new(Some(PreparedUpdate {
            token: "verified".into(),
            update: (),
            bytes: vec![1, 2, 3],
        }));
        let writer = gate.write().unwrap();
        assert!(install_prepared(&gate, &ready, "verified", |_, _| panic!(
            "must not exit while writing"
        ))
        .is_err());
        drop(writer);
        assert!(install_prepared(&gate, &ready, "stale", |_, _| panic!(
            "must not install stale bytes"
        ))
        .is_err());
        assert!(install_prepared(&gate, &ready, "verified", |_, _| Err(
            Error::NoPendingUpdate
        ))
        .is_err());
        assert!(ready.lock().unwrap().is_some());
        install_prepared(&gate, &ready, "verified", |_, bytes| {
            assert_eq!(bytes, [1, 2, 3]);
            assert!(gate.write().is_err());
            assert!(gate.maintain().is_err());
            Ok(())
        })
        .unwrap();
        assert!(ready.lock().unwrap().is_none());
        assert!(gate.write().is_ok());
        gate.require_restart();
        assert!(
            install_prepared(&gate, &ready, "verified", |_, _| panic!("library switched")).is_err()
        );
    }

    #[test]
    fn treats_missing_manifest_as_no_update() {
        assert!(super::is_missing_update_manifest(StatusCode::NOT_FOUND));
        assert!(!super::is_missing_update_manifest(StatusCode::OK));
        assert!(!super::is_missing_update_manifest(
            StatusCode::INTERNAL_SERVER_ERROR
        ));
    }

    #[test]
    fn only_supports_windows_updates() {
        assert_eq!(
            super::updater_supported_on_current_platform(),
            cfg!(target_os = "windows")
        );
    }
}
