use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Fixture {
    dir: std::path::PathBuf,
    db: Database,
    vault: Arc<MemoryVault>,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("credential-fixture-{}", uuid::Uuid::new_v4()));
        let vault = Arc::new(MemoryVault::default());
        let db = Database::open_with_vault(&dir.join("library.db"), vault.clone()).unwrap();
        db.run_migrations().unwrap();
        Self { dir, db, vault }
    }
    fn reopen(&self) -> Database {
        let db =
            Database::open_with_vault(&self.dir.join("library.db"), self.vault.clone()).unwrap();
        db.run_migrations().unwrap();
        db
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}
fn profile(secret: &str) -> String {
    json!([{"id":"one","name":"One","api_key":secret,"endpoint_settings":{"mode":"base_url","base_url":"https://example.test","generation_url":"https://example.test/images/generations","edit_url":"https://example.test/images/edits"}}]).to_string()
}
const PROFILES: &str = "model_provider_profiles::gpt-image-2";
#[test]
fn migrates_all_aliases_and_cleans_database_and_wal_bytes() {
    let f = Fixture::new();
    let marker = "raw-secret-byte-marker-9348";
    for key in [
        "api_key",
        "model_config::gpt-image-2::api_key",
        "model_config::gemini-3.1-flash-image-preview::api_key",
        "model_config::future-model::api_key",
    ] {
        f.db.set_setting(key, marker).unwrap();
    }
    f.db.set_setting(PROFILES, &profile(marker)).unwrap();
    f.db.set_setting(
        "llm_configs",
        &json!([{"id":"llm","api_key":"llm-secret-byte-marker-234"}]).to_string(),
    )
    .unwrap();
    f.db.migrate_credentials().unwrap();
    assert_eq!(
        f.vault.0.lock().unwrap().len(),
        2,
        "identical aliases should share a verified entry"
    );
    assert_eq!(
        f.db.resolve_legacy_credential(&f.db.get_setting("api_key").unwrap().unwrap())
            .unwrap(),
        marker
    );
    for path in [f.dir.join("library.db"), f.dir.join("library.db-wal")] {
        if let Ok(bytes) = std::fs::read(path) {
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains(marker));
            assert!(!text.contains("llm-secret-byte-marker-234"));
        }
    }
    let reopened = f.reopen();
    assert!(reopened
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains(marker));
}
struct FailingVault {
    memory: MemoryVault,
    writes: AtomicUsize,
    fail_at: AtomicUsize,
    bad_readback: bool,
}
impl CredentialVault for FailingVault {
    fn delete(&self, r: &str) -> Result<()> {
        self.memory.delete(r)
    }
    fn get(&self, r: &str) -> Result<Option<String>> {
        let value = self.memory.get(r)?;
        if self.bad_readback && value.is_some() {
            Ok(Some("wrong".into()))
        } else {
            Ok(value)
        }
    }
    fn set(&self, r: &str, s: &str) -> Result<()> {
        let n = self.writes.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_at.load(Ordering::SeqCst) == n {
            Err(unavailable())
        } else {
            self.memory.set(r, s)
        }
    }
}
#[test]
fn failed_migration_retains_original_and_resumes_with_journaled_references() {
    let f = Fixture::new();
    let vault = Arc::new(FailingVault {
        memory: MemoryVault::default(),
        writes: AtomicUsize::new(0),
        fail_at: AtomicUsize::new(2),
        bad_readback: false,
    });
    let db = Database::open_with_vault(&f.dir.join("library.db"), vault.clone()).unwrap();
    db.set_setting("api_key", "original-secret-a").unwrap();
    db.set_setting(
        "llm_configs",
        &json!([{"id":"llm","api_key":"original-secret-b"}]).to_string(),
    )
    .unwrap();
    assert!(db.migrate_credentials().is_err());
    assert_eq!(
        db.get_setting("api_key").unwrap().as_deref(),
        Some("original-secret-a")
    );
    let journal = db.get_setting(PENDING).unwrap().unwrap();
    assert!(!journal.contains("original-secret"));
    let first_refs = vault
        .memory
        .0
        .lock()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    drop(db);
    vault.fail_at.store(0, Ordering::SeqCst);
    let db = Database::open_with_vault(&f.dir.join("library.db"), vault.clone()).unwrap();
    db.migrate_credentials().unwrap();
    assert_eq!(vault.memory.0.lock().unwrap().len(), 2);
    assert!(first_refs
        .iter()
        .all(|r| vault.memory.0.lock().unwrap().contains_key(r)));
    assert!(db.get_setting(PENDING).unwrap().is_none());
    assert!(!db
        .get_setting("api_key")
        .unwrap()
        .unwrap()
        .contains("original-secret"));
}
#[test]
fn bad_readback_never_clears_plaintext() {
    let f = Fixture::new();
    let vault = Arc::new(FailingVault {
        memory: MemoryVault::default(),
        writes: AtomicUsize::new(0),
        fail_at: AtomicUsize::new(0),
        bad_readback: true,
    });
    let db = Database::open_with_vault(&f.dir.join("library.db"), vault).unwrap();
    db.set_setting("api_key", "original-key").unwrap();
    assert!(db.migrate_credentials().is_err());
    assert_eq!(
        db.get_setting("api_key").unwrap().as_deref(),
        Some("original-key")
    );
}
#[test]
fn metadata_keep_replace_and_clear_use_vault_without_raw_storage() {
    let f = Fixture::new();
    f.db.write_credential_json(PROFILES, &profile("first-secret"))
        .unwrap();
    let original = f.db.get_setting(PROFILES).unwrap().unwrap();
    f.db.write_credential_json(PROFILES, &profile("first-secret"))
        .unwrap();
    assert_eq!(original, f.db.get_setting(PROFILES).unwrap().unwrap());
    f.db.write_credential_json(PROFILES, &profile("second-secret"))
        .unwrap();
    let replacement = f.db.get_setting(PROFILES).unwrap().unwrap();
    assert_ne!(original, replacement);
    assert!(!replacement.contains("second-secret"));
    f.db.write_credential_json(PROFILES, &profile("")).unwrap();
    assert!(!f
        .db
        .get_setting(PROFILES)
        .unwrap()
        .unwrap()
        .contains("credential_ref"));
    assert!(!f
        .db
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("second-secret"));
}
#[test]
fn ipc_dtos_never_expose_secrets_or_references_and_write_actions_are_explicit() {
    let profile: crate::models::ModelProviderProfile =
        serde_json::from_str::<Vec<_>>(&profile("secret-read-boundary"))
            .unwrap()
            .remove(0);
    let dto = ProviderProfilesDto::from(crate::models::ModelProviderProfilesState {
        active_provider_id: "one".into(),
        profiles: vec![profile],
    });
    let value = serde_json::to_value(dto).unwrap();
    let wire = value.to_string();
    assert!(!wire.contains("secret-read-boundary"));
    assert!(!wire.contains("credential_ref"));
    assert!(value["profiles"][0].get("api_key").is_none());
    assert_eq!(value["profiles"][0]["api_key_configured"], true);
    assert_eq!(CredentialUpdate::Keep.apply("saved").unwrap(), "saved");
    assert_eq!(CredentialUpdate::Clear.apply("saved").unwrap(), "");
    assert_eq!(
        CredentialUpdate::Replace {
            value: " Bearer next ".into()
        }
        .apply("saved")
        .unwrap(),
        "next"
    );
    assert!(CredentialUpdate::Replace { value: "  ".into() }
        .apply("saved")
        .is_err());
    assert!(serde_json::from_value::<ProviderProfileWrite>(
        json!({"id":"one","name":"One","api_key":"secret"})
    )
    .is_err());
}
#[test]
fn rollback_of_legacy_library_remigrates_before_internal_use() {
    let f = Fixture::new();
    f.db.set_setting(PROFILES, &profile("legacy-rollback-secret"))
        .unwrap();
    f.db.migrate_credentials().unwrap();
    let original = f.db.get_setting(PROFILES).unwrap().unwrap();
    f.db.write_credential_json(PROFILES, &profile("updated-secret"))
        .unwrap();
    // A rollback may reintroduce pre-migration plaintext. The common read gate remigrates it.
    f.db.set_setting(PROFILES, &profile("legacy-rollback-secret"))
        .unwrap();
    let reopened = f.reopen();
    assert!(reopened
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("legacy-rollback-secret"));
    assert!(!reopened
        .get_setting(PROFILES)
        .unwrap()
        .unwrap()
        .contains("legacy-rollback-secret"));
    // Already-migrated rollback references still resolve to their original immutable secrets.
    reopened.set_setting(PROFILES, &original).unwrap();
    assert!(reopened
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("legacy-rollback-secret"));
}

#[test]
fn interrupted_physical_cleanup_is_reported_and_resumes_after_reader_releases_wal() {
    let f = Fixture::new();
    f.db.set_setting("api_key", "physical-cleanup-secret-marker")
        .unwrap();
    let reader = Connection::open(f.dir.join("library.db")).unwrap();
    reader.execute_batch("BEGIN;").unwrap();
    let _: String = reader
        .query_row("SELECT value FROM settings WHERE key='api_key'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(f.db.migrate_credentials().is_err());
    assert_eq!(f.db.get_setting(CLEANUP).unwrap().as_deref(), Some("true"));
    assert!(f
        .db
        .get_setting("api_key")
        .unwrap()
        .unwrap()
        .starts_with(REF_PREFIX));
    reader.execute_batch("ROLLBACK;").unwrap();
    drop(reader);
    f.db.migrate_credentials().unwrap();
    assert!(f.db.get_setting(CLEANUP).unwrap().is_none());
    for path in [f.dir.join("library.db"), f.dir.join("library.db-wal")] {
        if let Ok(bytes) = std::fs::read(path) {
            assert!(!String::from_utf8_lossy(&bytes).contains("physical-cleanup-secret-marker"));
        }
    }
}

/// Opt-in only. Uses a new random test entry and never reads an existing application entry.
#[cfg(any(target_os = "macos", target_os = "windows"))]
#[test]
#[ignore = "explicit native-vault validation only; ordinary tests use MemoryVault"]
fn native_vault_isolated_roundtrip() {
    let vault = NativeVault;
    let reference = format!("vault-test:{}", uuid::Uuid::new_v4());
    let secret = format!("dummy-test-secret-{}", uuid::Uuid::new_v4());
    struct Cleanup<'a> {
        vault: &'a NativeVault,
        reference: &'a str,
    }
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.vault.delete(self.reference);
        }
    }
    let _cleanup = Cleanup {
        vault: &vault,
        reference: &reference,
    };
    assert!(vault.get(&reference).unwrap().is_none());
    vault.set(&reference, &secret).unwrap();
    assert_eq!(
        vault.get(&reference).unwrap().as_deref(),
        Some(secret.as_str())
    );
    vault.delete(&reference).unwrap();
    assert!(vault.get(&reference).unwrap().is_none());
}

#[test]
fn missing_vault_entry_is_unconfigured_and_can_be_kept_replaced_or_cleared() {
    let f = Fixture::new();
    f.db.write_credential_json(PROFILES, &profile("original"))
        .unwrap();
    let stored = f.db.get_setting(PROFILES).unwrap().unwrap();
    f.vault.0.lock().unwrap().clear();
    let state = crate::commands::settings::read_model_provider_profiles_state(&f.db, "gpt-image-2")
        .unwrap();
    assert!(state.profiles[0].api_key.is_empty());
    let dto = serde_json::to_value(ProviderProfilesDto::from(state.clone())).unwrap();
    assert_eq!(dto["profiles"][0]["api_key_configured"], false);
    let make = |action: Value| {
        serde_json::from_value::<ProviderProfileWrite>(json!({"id":"one","name":"Updated","endpoint_settings":state.profiles[0].endpoint_settings,"credential_update":action})).unwrap()
    };
    crate::commands::settings::save_provider_profile_updates(
        &f.db,
        "gpt-image-2",
        "one".into(),
        vec![make(json!({"action":"keep"}))],
    )
    .unwrap();
    let kept: Value = serde_json::from_str(&f.db.get_setting(PROFILES).unwrap().unwrap()).unwrap();
    let original: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(kept[0]["credential_ref"], original[0]["credential_ref"]);
    crate::commands::settings::save_provider_profile_updates(
        &f.db,
        "gpt-image-2",
        "one".into(),
        vec![make(json!({"action":"replace","value":"repaired"}))],
    )
    .unwrap();
    assert!(f
        .db
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("repaired"));
    f.vault.0.lock().unwrap().clear();
    crate::commands::settings::save_provider_profile_updates(
        &f.db,
        "gpt-image-2",
        "one".into(),
        vec![make(json!({"action":"clear"}))],
    )
    .unwrap();
    assert!(!f
        .db
        .get_setting(PROFILES)
        .unwrap()
        .unwrap()
        .contains("credential_ref"));
}

#[test]
fn malformed_collections_preserve_every_legacy_secret_and_return_errors_without_panicking() {
    for malformed in [
        json!([{"api_key":"retained-secret"},null]),
        json!([12]),
        json!([{"api_key":123}]),
        json!([{"credential_ref":false}]),
    ] {
        let f = Fixture::new();
        let stored = malformed.to_string();
        f.db.set_setting(PROFILES, &stored).unwrap();
        f.db.set_setting("api_key", "other-retained-secret")
            .unwrap();
        assert!(f.db.migrate_credentials().is_err());
        assert!(f.db.read_credential_json(PROFILES).is_err());
        assert_eq!(
            f.db.get_setting(PROFILES).unwrap().as_deref(),
            Some(stored.as_str())
        );
        assert_eq!(
            f.db.get_setting("api_key").unwrap().as_deref(),
            Some("other-retained-secret")
        );
        assert!(f.vault.0.lock().unwrap().is_empty());
    }
}

#[test]
fn keep_uses_the_current_reference_even_when_a_form_was_read_before_replacement() {
    let f = Fixture::new();
    f.db.write_credential_json(PROFILES, &profile("before"))
        .unwrap();
    let stale = f.db.read_credential_json(PROFILES).unwrap().unwrap();
    f.db.write_credential_json(PROFILES, &profile("after"))
        .unwrap();
    let current = f.db.get_setting(PROFILES).unwrap().unwrap();
    f.db.write_credential_json_with_kept_ids(
        PROFILES,
        &stale,
        &std::collections::HashSet::from(["one".into()]),
    )
    .unwrap();
    assert_eq!(f.db.get_setting(PROFILES).unwrap().unwrap(), current);
    assert!(f
        .db
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("after"));
}

#[test]
fn first_profile_save_keeps_the_legacy_scalar_credential() {
    for key in ["api_key", "model_config::gpt-image-2::api_key"] {
        let f = Fixture::new();
        f.db.set_setting(key, "legacy-key-to-keep").unwrap();
        let state =
            crate::commands::settings::read_model_provider_profiles_state(&f.db, "gpt-image-2")
                .unwrap();
        let previous = &state.profiles[0];
        let update = serde_json::from_value::<ProviderProfileWrite>(json!({
            "id": previous.id,
            "name": "Renamed legacy provider",
            "endpoint_settings": previous.endpoint_settings,
            "credential_update": {"action": "keep"}
        }))
        .unwrap();
        crate::commands::settings::save_provider_profile_updates(
            &f.db,
            "gpt-image-2",
            state.active_provider_id,
            vec![update],
        )
        .unwrap();
        assert_eq!(
            crate::commands::settings::read_model_api_key(&f.db, "gpt-image-2")
                .unwrap()
                .as_deref(),
            Some("legacy-key-to-keep"),
            "first profile save must preserve the legacy credential from {key}"
        );
        assert!(!f
            .db
            .get_setting(PROFILES)
            .unwrap()
            .unwrap()
            .contains("legacy-key-to-keep"));
    }
}

#[test]
fn interrupted_profile_write_removes_provisional_vault_entries_on_reopen() {
    let f = Fixture::new();
    let vault = Arc::new(FailingVault {
        memory: MemoryVault::default(),
        writes: AtomicUsize::new(0),
        fail_at: AtomicUsize::new(2),
        bad_readback: false,
    });
    let db = Database::open_with_vault(&f.dir.join("library.db"), vault.clone()).unwrap();
    let mut profiles: Value = serde_json::from_str(&profile("provisional-secret-one")).unwrap();
    let mut second = profiles[0].clone();
    second["id"] = json!("two");
    second["api_key"] = json!("provisional-secret-two");
    profiles.as_array_mut().unwrap().push(second);
    assert!(db
        .write_credential_json(PROFILES, &profiles.to_string())
        .is_err());
    assert!(db.get_setting(PROFILES).unwrap().is_none());
    assert_eq!(vault.memory.0.lock().unwrap().len(), 1);
    drop(db);
    let reopened = Database::open_with_vault(&f.dir.join("library.db"), vault.clone()).unwrap();
    reopened.migrate_credentials().unwrap();
    assert!(
        vault.memory.0.lock().unwrap().is_empty(),
        "uncommitted secrets must be cleaned on recovery"
    );
    assert!(reopened.get_setting(DELETES).unwrap().is_none());
}

#[test]
fn failed_profile_metadata_commit_cleans_only_provisional_credentials() {
    let f = Fixture::new();
    f.db.write_credential_json(PROFILES, &profile("original-immutable-secret"))
        .unwrap();
    let original = f.db.get_setting(PROFILES).unwrap().unwrap();
    f.db.conn.lock().unwrap().execute_batch(
        "CREATE TRIGGER reject_profile_update BEFORE UPDATE ON settings WHEN NEW.key = 'model_provider_profiles::gpt-image-2' BEGIN SELECT RAISE(FAIL, 'injected metadata failure'); END;"
    ).unwrap();
    assert!(f
        .db
        .write_credential_json(PROFILES, &profile("uncommitted-replacement"))
        .is_err());
    assert_eq!(f.vault.0.lock().unwrap().len(), 2);
    let reopened = f.reopen();
    reopened.migrate_credentials().unwrap();
    assert_eq!(f.vault.0.lock().unwrap().len(), 1);
    assert_eq!(reopened.get_setting(PROFILES).unwrap().unwrap(), original);
    assert!(reopened
        .read_credential_json(PROFILES)
        .unwrap()
        .unwrap()
        .contains("original-immutable-secret"));
}

struct DeleteFailingVault {
    memory: MemoryVault,
    fail_delete: std::sync::atomic::AtomicBool,
}
impl CredentialVault for DeleteFailingVault {
    fn get(&self, reference: &str) -> Result<Option<String>> {
        self.memory.get(reference)
    }
    fn set(&self, reference: &str, secret: &str) -> Result<()> {
        self.memory.set(reference, secret)
    }
    fn delete(&self, reference: &str) -> Result<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            Err(unavailable())
        } else {
            self.memory.delete(reference)
        }
    }
}

#[test]
fn failed_vault_cleanup_keeps_the_journal_until_recovery_succeeds() {
    let f = Fixture::new();
    let vault = Arc::new(DeleteFailingVault {
        memory: MemoryVault::default(),
        fail_delete: std::sync::atomic::AtomicBool::new(true),
    });
    let db = Database::open_with_vault(&f.dir.join("library.db"), vault.clone()).unwrap();
    db.write_credential_json(PROFILES, &profile("key-to-clear"))
        .unwrap();
    assert!(
        db.get_setting(DELETES).unwrap().is_none(),
        "committed entries must be disarmed"
    );
    assert!(db.write_credential_json(PROFILES, &profile("")).is_err());
    let journal = db.get_setting(DELETES).unwrap().unwrap();
    assert!(!journal.contains("key-to-clear"));
    assert!(db.migrate_credentials().is_err());
    assert_eq!(
        db.get_setting(DELETES).unwrap().as_deref(),
        Some(journal.as_str())
    );
    vault.fail_delete.store(false, Ordering::SeqCst);
    db.migrate_credentials().unwrap();
    assert!(db.get_setting(DELETES).unwrap().is_none());
    assert!(vault.memory.0.lock().unwrap().is_empty());
}
