use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, State};

#[derive(Clone, Default)]
pub(crate) struct Operations(Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>);
pub(crate) struct OperationGuard {
    id: String,
    flag: Arc<AtomicBool>,
    registry: Operations,
}
impl Operations {
    pub(crate) fn begin(&self, id: &str) -> Result<OperationGuard, String> {
        uuid::Uuid::parse_str(id).map_err(|_| "Invalid operation ID")?;
        let mut entries = self
            .0
            .lock()
            .map_err(|_| "Operation registry unavailable")?;
        if entries.contains_key(id) {
            return Err("Operation is already running".into());
        }
        let flag = Arc::new(AtomicBool::new(false));
        entries.insert(id.to_owned(), flag.clone());
        Ok(OperationGuard {
            id: id.into(),
            flag,
            registry: self.clone(),
        })
    }
    pub(crate) fn cancel(&self, id: &str) -> Result<(), String> {
        if let Some(flag) = self
            .0
            .lock()
            .map_err(|_| "Operation registry unavailable")?
            .get(id)
        {
            flag.store(true, Ordering::Release);
        }
        Ok(())
    }
}
impl OperationGuard {
    pub(crate) fn cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }
    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if self.cancelled() {
            Err("operation_cancelled".into())
        } else {
            Ok(())
        }
    }
    pub(crate) fn progress(
        &self,
        app: &tauri::AppHandle,
        phase: &str,
        completed: usize,
        total: usize,
    ) {
        #[derive(Clone, Serialize)]
        struct Progress<'a> {
            operation_id: &'a str,
            phase: &'a str,
            completed: usize,
            total: usize,
        }
        let _ = app.emit(
            "library:operation-progress",
            Progress {
                operation_id: &self.id,
                phase,
                completed,
                total,
            },
        );
    }
}
impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut entries) = self.registry.0.lock() {
            entries.remove(&self.id);
        }
    }
}
#[tauri::command]
pub(crate) fn cancel_operation(
    operations: State<'_, Operations>,
    operation_id: String,
) -> Result<(), String> {
    operations.cancel(&operation_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_scoped_and_cleanup_allows_retry() {
        let ops = Operations::default();
        let id = uuid::Uuid::new_v4().to_string();
        let guard = ops.begin(&id).unwrap();
        assert!(ops.begin(&id).is_err());
        assert!(!guard.cancelled());
        ops.cancel(&id).unwrap();
        assert!(guard.checkpoint().is_err());
        drop(guard);
        assert!(ops.begin(&id).is_ok());
    }
}
