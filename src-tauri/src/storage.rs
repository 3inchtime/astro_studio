use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct GateState {
    writers: usize,
    maintenance: bool,
    restart_required: bool,
}
#[derive(Clone, Default)]
pub(crate) struct MaintenanceGate(Arc<Mutex<GateState>>);
pub(crate) struct WritePermit(Arc<Mutex<GateState>>);
pub(crate) struct MaintenancePermit(Arc<Mutex<GateState>>);
impl MaintenanceGate {
    pub(crate) fn write(&self) -> Result<WritePermit, String> {
        let mut state = self.0.lock().map_err(|_| "Library lock unavailable")?;
        if state.maintenance || state.restart_required {
            return Err("Library maintenance is in progress. Please try again afterwards.".into());
        }
        state.writers += 1;
        Ok(WritePermit(self.0.clone()))
    }
    pub(crate) fn require_restart(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.restart_required = true;
        }
    }
    pub(crate) fn maintain(&self) -> Result<MaintenancePermit, String> {
        let mut state = self.0.lock().map_err(|_| "Library lock unavailable")?;
        if state.maintenance || state.restart_required || state.writers > 0 {
            return Err(
                "The library is busy. Finish active operations before backing up or restoring."
                    .into(),
            );
        }
        state.maintenance = true;
        Ok(MaintenancePermit(self.0.clone()))
    }
}
impl Drop for WritePermit {
    fn drop(&mut self) {
        if let Ok(mut s) = self.0.lock() {
            s.writers = s.writers.saturating_sub(1);
        }
    }
}
impl Drop for MaintenancePermit {
    fn drop(&mut self) {
        if let Ok(mut s) = self.0.lock() {
            s.maintenance = false;
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct LibraryLocation {
    pub database: PathBuf,
    pub assets: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
struct LibraryPointer {
    version: u32,
    current: LibraryLocation,
    previous: Option<LibraryLocation>,
}
#[derive(Clone, Debug)]
pub(crate) struct StoragePaths {
    pub bootstrap: PathBuf,
    pub database: PathBuf,
    pub assets: PathBuf,
    pub previous: Option<LibraryLocation>,
}
impl StoragePaths {
    pub(crate) fn new(
        bootstrap: PathBuf,
        legacy_db: PathBuf,
        legacy_assets: PathBuf,
    ) -> Result<Self, String> {
        fs::create_dir_all(&bootstrap).map_err(|e| e.to_string())?;
        let pointer = bootstrap.join("active-library.json");
        if pointer.exists() {
            let value: LibraryPointer =
                serde_json::from_slice(&fs::read(pointer).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            if value.version != 1 {
                return Err("Unsupported library pointer version".into());
            }
            Ok(Self {
                bootstrap,
                database: value.current.database,
                assets: value.current.assets,
                previous: value.previous,
            })
        } else {
            Ok(Self {
                bootstrap,
                database: legacy_db,
                assets: legacy_assets,
                previous: None,
            })
        }
    }
    pub(crate) fn activate(&self, root: &Path) -> Result<(), String> {
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let libraries = self
            .bootstrap
            .join("libraries")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !root.starts_with(libraries) {
            return Err("Invalid restored library location".into());
        }
        self.write_pointer(LibraryPointer {
            version: 1,
            current: LibraryLocation {
                database: root.join("library.db"),
                assets: root.join("assets"),
            },
            previous: Some(LibraryLocation {
                database: self.database.clone(),
                assets: self.assets.clone(),
            }),
        })
    }
    pub(crate) fn rollback(&self) -> Result<(), String> {
        let previous = self
            .previous
            .clone()
            .ok_or("There is no previous library to restore")?;
        self.write_pointer(LibraryPointer {
            version: 1,
            current: previous,
            previous: None,
        })
    }
    fn write_pointer(&self, pointer: LibraryPointer) -> Result<(), String> {
        atomic_write(
            &self.bootstrap.join("active-library.json"),
            &serde_json::to_vec_pretty(&pointer).map_err(|e| e.to_string())?,
        )
    }
}

/// Prevent simultaneous processes from opening the same application library set.
pub(crate) struct LibraryLock(File);
impl LibraryLock {
    pub(crate) fn acquire(bootstrap: &Path) -> Result<Self, String> {
        fs::create_dir_all(bootstrap).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(bootstrap.join("library.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock_exclusive().map_err(|_| {
            "Astro Studio is already using this library. Close the other instance first."
                .to_string()
        })?;
        Ok(Self(file))
    }
}
impl Drop for LibraryLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
            };
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        #[cfg(not(windows))]
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            // Rename is the publication point. Never report an uncommitted operation
            // after the visible pointer has already changed.
            if let Err(error) = File::open(parent).and_then(|f| f.sync_all()) {
                log::warn!("Published file; directory sync unavailable: {error}");
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

pub(crate) fn global_gate() -> &'static MaintenanceGate {
    static GATE: std::sync::OnceLock<MaintenanceGate> = std::sync::OnceLock::new();
    GATE.get_or_init(MaintenanceGate::default)
}
pub(crate) fn write_operation<E: From<String>>() -> Result<WritePermit, E> {
    global_gate().write().map_err(E::from)
}
pub(crate) fn asset_root(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    use tauri::Manager;
    if let Some(paths) = app.try_state::<StoragePaths>() {
        Ok(paths.assets.clone())
    } else {
        app.path().app_data_dir()
    }
}

/// Debug builds can run native acceptance checks without opening the user's library.
/// Release builds deliberately ignore this test-only override.
pub(crate) fn development_library_root() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        std::env::var_os("ASTRO_STUDIO_TEST_LIBRARY")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_library_lock_is_released_when_process_handle_closes() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let lock = LibraryLock::acquire(&root).unwrap();
        assert!(LibraryLock::acquire(&root).is_err());
        drop(lock);
        let next = LibraryLock::acquire(&root).unwrap();
        drop(next);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn maintenance_excludes_writers_and_releases_on_drop() {
        let gate = MaintenanceGate::default();
        let writer = gate.write().unwrap();
        assert!(gate.maintain().is_err());
        drop(writer);
        let maintenance = gate.maintain().unwrap();
        assert!(gate.write().is_err());
        drop(maintenance);
        assert!(gate.write().is_ok());
    }
    #[test]
    fn library_pointer_keeps_legacy_paths_and_supports_rollback() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let paths = StoragePaths::new(
            root.join("bootstrap"),
            root.join("old.db"),
            root.join("assets"),
        )
        .unwrap();
        assert_eq!(paths.database, root.join("old.db"));
        let next = paths
            .bootstrap
            .join("libraries")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&next).unwrap();
        paths.activate(&next).unwrap();
        let restored = StoragePaths::new(
            paths.bootstrap.clone(),
            root.join("old.db"),
            root.join("assets"),
        )
        .unwrap();
        assert_eq!(
            restored.database,
            next.canonicalize().unwrap().join("library.db")
        );
        assert!(restored.previous.is_some());
        restored.rollback().unwrap();
        let rolled = StoragePaths::new(
            paths.bootstrap.clone(),
            root.join("old.db"),
            root.join("assets"),
        )
        .unwrap();
        assert_eq!(rolled.database, root.join("old.db"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
