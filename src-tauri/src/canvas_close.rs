//! Native close/quit/restart handshake; the renderer flushes dirty drafts first.
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

#[derive(Clone, Serialize)]
pub(crate) struct CloseRequest {
    request_id: String,
    action: String,
    #[serde(skip)]
    window: Option<String>,
}
#[derive(Default)]
pub(crate) struct CloseGuard {
    enabled: bool,
    pending: Option<CloseRequest>,
}
impl CloseGuard {
    fn request(&mut self, action: &str, window: Option<String>) -> Option<CloseRequest> {
        if !self.enabled || self.pending.is_some() {
            return None;
        }
        let request = CloseRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            action: action.into(),
            window,
        };
        self.pending = Some(request.clone());
        Some(request)
    }
    fn finish(&mut self, id: &str) -> Result<CloseRequest, String> {
        if self.pending.as_ref().is_none_or(|r| r.request_id != id) {
            return Err("Close request expired".into());
        }
        self.enabled = false;
        Ok(self.pending.take().unwrap())
    }
    fn cancel(&mut self, id: &str) {
        if self.pending.as_ref().is_some_and(|r| r.request_id == id) {
            self.pending = None;
        }
    }
}
pub(crate) type CanvasCloseState = Mutex<CloseGuard>;

pub(crate) fn guarded(app: &tauri::AppHandle) -> bool {
    app.state::<CanvasCloseState>()
        .lock()
        .map(|s| s.enabled)
        .unwrap_or(true)
}
pub(crate) fn request(
    app: &tauri::AppHandle,
    action: &str,
    window: Option<String>,
) -> Result<(), String> {
    let request = app
        .state::<CanvasCloseState>()
        .lock()
        .map_err(|e| e.to_string())?
        .request(action, window);
    if let Some(request) = request {
        if let Err(error) = app.emit("canvas:close-requested", &request) {
            app.state::<CanvasCloseState>()
                .lock()
                .map_err(|e| e.to_string())?
                .cancel(&request.request_id);
            return Err(error.to_string());
        }
    }
    Ok(())
}
#[tauri::command]
pub(crate) fn enable_canvas_close_guard(state: State<'_, CanvasCloseState>) -> Result<(), String> {
    state.lock().map_err(|e| e.to_string())?.enabled = true;
    Ok(())
}
#[tauri::command]
pub(crate) fn cancel_canvas_close(
    state: State<'_, CanvasCloseState>,
    request_id: String,
) -> Result<(), String> {
    state.lock().map_err(|e| e.to_string())?.cancel(&request_id);
    Ok(())
}
#[tauri::command]
pub(crate) fn finish_canvas_close(
    app: tauri::AppHandle,
    state: State<'_, CanvasCloseState>,
    request_id: String,
) -> Result<(), String> {
    let request = state
        .lock()
        .map_err(|e| e.to_string())?
        .finish(&request_id)?;
    match request.action.as_str() {
        "restart" => app.restart(),
        "close" => {
            if let Some(window) = request
                .window
                .clone()
                .and_then(|name| app.get_webview_window(&name))
            {
                if let Err(error) = window.destroy() {
                    let mut guard = state.lock().map_err(|e| e.to_string())?;
                    guard.enabled = true;
                    guard.pending = Some(request);
                    return Err(error.to_string());
                }
            }
        }
        _ => app.exit(0),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_guard_deduplicates_requests_and_rejects_stale_approval() {
        let mut guard = CloseGuard::default();
        assert!(guard.request("close", Some("main".into())).is_none());
        guard.enabled = true;
        let request = guard.request("close", Some("main".into())).unwrap();
        assert!(guard.request("exit", None).is_none());
        assert!(guard.finish("wrong").is_err());
        assert!(guard.enabled);
        assert_eq!(guard.finish(&request.request_id).unwrap().action, "close");
        assert!(!guard.enabled);
    }
    #[test]
    fn cancelled_close_can_be_retried_without_disabling_the_guard() {
        let mut guard = CloseGuard {
            enabled: true,
            ..Default::default()
        };
        let first = guard.request("restart", None).unwrap();
        guard.cancel(&first.request_id);
        assert!(guard.enabled);
        let next = guard.request("exit", None).unwrap();
        assert_ne!(first.request_id, next.request_id);
    }
}
