use std::sync::Mutex;
use serde::Serialize;
use reqwest::{ClientBuilder, StatusCode, Url};
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Updater(#[from] tauri_plugin_updater::Error),
    #[error(transparent)]
    Network(#[from] reqwest::Error),
    #[error("updates are only supported on Windows")]
    UnsupportedPlatform,
    #[error("there is no pending update")]
    NoPendingUpdate,
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
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    pub version: String,
    pub current_version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

pub struct PendingUpdate(pub Mutex<Option<Update>>);

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

    if is_missing_update_manifest(response.status()) {
        return Ok(false);
    }

    Ok(true)
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
pub async fn install_update(
    pending_update: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<()> {
    if !updater_supported_on_current_platform() {
        *pending_update.0.lock().unwrap() = None;
        return Err(Error::UnsupportedPlatform);
    }

    let Some(update) = pending_update.0.lock().unwrap().clone() else {
        return Err(Error::NoPendingUpdate);
    };

    let mut started = false;
    let mut total_downloaded: u64 = 0;

    update
        .download_and_install(
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
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await?;

    *pending_update.0.lock().unwrap() = None;

    Ok(())
}

#[tauri::command]
pub async fn relaunch_app(app: AppHandle) -> Result<()> {
    app.restart();
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;

    #[test]
    fn treats_missing_manifest_as_no_update() {
        assert!(super::is_missing_update_manifest(StatusCode::NOT_FOUND));
        assert!(!super::is_missing_update_manifest(StatusCode::OK));
        assert!(!super::is_missing_update_manifest(StatusCode::INTERNAL_SERVER_ERROR));
    }

    #[test]
    fn only_supports_windows_updates() {
        assert_eq!(
            super::updater_supported_on_current_platform(),
            cfg!(target_os = "windows")
        );
    }
}
