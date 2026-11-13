//! Secrets stay in the native vault. SQLite stores opaque references only.
use crate::{db::Database, error::AppError};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

const SERVICE: &str = "com.astro-studio.credentials.v1";
const REF_PREFIX: &str = "vault:v1:";
const PENDING: &str = "credential_migration_pending";
const CLEANUP: &str = "credential_cleanup_pending";
const DELETES: &str = "credential_pending_deletes";

type Result<T> = std::result::Result<T, AppError>;
fn unavailable() -> AppError {
    AppError::CredentialUnavailable
}

pub(crate) trait CredentialVault: Send + Sync {
    fn get(&self, reference: &str) -> Result<Option<String>>;
    fn set(&self, reference: &str, secret: &str) -> Result<()>;
    fn delete(&self, reference: &str) -> Result<()>;
}
struct NativeVault;
impl CredentialVault for NativeVault {
    fn get(&self, reference: &str) -> Result<Option<String>> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            match keyring::Entry::new(SERVICE, reference)
                .map_err(|_| unavailable())?
                .get_password()
            {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(unavailable()),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = reference;
            Err(unavailable())
        }
    }
    fn delete(&self, reference: &str) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            match keyring::Entry::new(SERVICE, reference)
                .map_err(|_| unavailable())?
                .delete_credential()
            {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(unavailable()),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = reference;
            Err(unavailable())
        }
    }
    fn set(&self, reference: &str, secret: &str) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            keyring::Entry::new(SERVICE, reference)
                .map_err(|_| unavailable())?
                .set_password(secret)
                .map_err(|_| unavailable())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = (reference, secret);
            Err(unavailable())
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryVault(pub Mutex<std::collections::HashMap<String, String>>);
#[cfg(test)]
impl CredentialVault for MemoryVault {
    fn delete(&self, reference: &str) -> Result<()> {
        self.0.lock().unwrap().remove(reference);
        Ok(())
    }
    fn get(&self, reference: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(reference).cloned())
    }
    fn set(&self, reference: &str, secret: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .insert(reference.into(), secret.into());
        Ok(())
    }
}

pub(crate) struct Credentials {
    vault: Arc<dyn CredentialVault>,
    // Serializes migration, readback, and updates across all clones of a database.
    gate: Mutex<()>,
}
impl Default for Credentials {
    fn default() -> Self {
        #[cfg(test)]
        let vault = Arc::new(MemoryVault::default());
        #[cfg(not(test))]
        let vault = Arc::new(NativeVault);
        Self::new(vault)
    }
}
impl Credentials {
    pub(crate) fn new(vault: Arc<dyn CredentialVault>) -> Self {
        Self {
            vault,
            gate: Mutex::new(()),
        }
    }
    fn resolve(&self, reference: &str) -> Result<String> {
        if !reference.starts_with(REF_PREFIX) {
            return Err(unavailable());
        }
        Ok(self.vault.get(reference)?.unwrap_or_default())
    }
    fn verified_write(&self, reference: &str, secret: &str) -> Result<()> {
        if self.vault.get(reference)?.as_deref() != Some(secret) {
            self.vault.set(reference, secret)?;
        }
        if self.vault.get(reference)?.as_deref() != Some(secret) {
            return Err(unavailable());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct CredentialStatus {
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
}
impl CredentialStatus {
    pub(crate) fn from_secret(secret: &str) -> Self {
        // Never reveal any substring of a short credential.
        Self {
            api_key_configured: !secret.trim().is_empty(),
            api_key_masked: (!secret.trim().is_empty()).then(|| "••••••••".into()),
        }
    }
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CredentialUpdate {
    #[default]
    Keep,
    Replace {
        value: String,
    },
    Clear,
}
impl CredentialUpdate {
    pub(crate) fn apply(self, previous: &str) -> Result<String> {
        match self {
            Self::Keep => Ok(previous.into()),
            Self::Clear => Ok(String::new()),
            Self::Replace { value } => {
                let value = normalize(&value);
                if value.is_empty() {
                    return Err(AppError::Validation {
                        message: "Replacement credential cannot be empty. Use clear to remove it."
                            .into(),
                    });
                }
                Ok(value)
            }
        }
    }
}
fn normalize(value: &str) -> String {
    let trimmed = value.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    if parts
        .next()
        .unwrap_or_default()
        .eq_ignore_ascii_case("bearer")
    {
        parts.next().unwrap_or_default().trim().into()
    } else {
        trimmed.into()
    }
}
fn scalar_key(key: &str) -> bool {
    key == "api_key" || (key.starts_with("model_config::") && key.ends_with("::api_key"))
}
fn collection_key(key: &str) -> bool {
    key == "llm_configs" || key.starts_with("model_provider_profiles::")
}
fn rows(conn: &Connection) -> Result<Vec<(String, String)>> {
    Ok(conn
        .prepare("SELECT key,value FROM settings ORDER BY key")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?)
}
fn put(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key,value])?;
    Ok(())
}
fn cleanup(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA secure_delete=ON; VACUUM;")?;
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
    if busy != 0 {
        return Err(unavailable());
    }
    conn.execute("DELETE FROM settings WHERE key=?1", [CLEANUP])?;
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
    if busy != 0 {
        return Err(unavailable());
    }
    Ok(())
}

impl Database {
    /// Idempotent and resumable: no plaintext is removed before every vault readback succeeds.
    pub(crate) fn migrate_credentials(&self) -> Result<()> {
        let _guard = self.credentials.gate.lock().map_err(|_| unavailable())?;
        self.migrate_credentials_locked()
    }
    fn migrate_credentials_locked(&self) -> Result<()> {
        let mut conn = self.conn.lock().map_err(|_| unavailable())?;
        let all = rows(&conn)?;
        let mut pending: std::collections::BTreeMap<String, String> = all
            .iter()
            .find(|(k, _)| k == PENDING)
            .map(|(_, v)| serde_json::from_str(v))
            .transpose()?
            .unwrap_or_default();
        let mut plans = Vec::<(String, String, String)>::new();
        let mut changed = Vec::new();
        for (key, text) in &all {
            if scalar_key(key) && !text.trim().is_empty() && !text.starts_with(REF_PREFIX) {
                let slot = key.clone();
                let reference = pending
                    .entry(slot.clone())
                    .or_insert_with(|| format!("{REF_PREFIX}{}", uuid::Uuid::new_v4()))
                    .clone();
                plans.push((slot, reference.clone(), normalize(text)));
                changed.push((key.clone(), reference));
            } else if collection_key(key) {
                let mut value: Value =
                    serde_json::from_str(text).map_err(|_| AppError::Validation {
                        message: "Credential settings are malformed; original data was retained."
                            .into(),
                    })?;
                let items = value.as_array_mut().ok_or_else(unavailable)?;
                let mut touched = false;
                for (index, item) in items.iter_mut().enumerate() {
                    if !item.is_object()
                        || item.get("api_key").is_some_and(|v| !v.is_string())
                        || item.get("credential_ref").is_some_and(|v| !v.is_string())
                    {
                        return Err(AppError::Validation {
                            message:
                                "Credential settings are malformed; original data was retained."
                                    .into(),
                        });
                    }
                    let secret = item
                        .get("api_key")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if secret.trim().is_empty() {
                        continue;
                    }
                    let slot = format!("{key}/{index}");
                    let reference = pending
                        .entry(slot.clone())
                        .or_insert_with(|| format!("{REF_PREFIX}{}", uuid::Uuid::new_v4()))
                        .clone();
                    plans.push((slot, reference.clone(), normalize(secret)));
                    item.as_object_mut()
                        .ok_or_else(unavailable)?
                        .remove("api_key");
                    item["credential_ref"] = Value::String(reference);
                    touched = true;
                }
                if touched {
                    changed.push((key.clone(), serde_json::to_string(&value)?));
                }
            }
        }
        let mut by_secret = std::collections::HashMap::<String, String>::new();
        for (slot, reference, secret) in &mut plans {
            if let Some(existing) = by_secret.get(secret) {
                let old = reference.clone();
                *reference = existing.clone();
                pending.insert(slot.clone(), existing.clone());
                for (_, text) in &mut changed {
                    *text = text.replace(&old, existing);
                }
            } else {
                by_secret.insert(secret.clone(), reference.clone());
            }
        }
        if !plans.is_empty() {
            // Journal contains only random references, never the source secrets.
            put(&conn, PENDING, &serde_json::to_string(&pending)?)?;
            for (_, reference, secret) in &plans {
                self.credentials.verified_write(reference, secret)?;
            }
            conn.execute_batch("PRAGMA secure_delete=ON;")?;
            let tx = conn.transaction()?;
            for (key, value) in changed {
                put(&tx, &key, &value)?;
            }
            tx.execute("DELETE FROM settings WHERE key=?1", [PENDING])?;
            put(&tx, CLEANUP, "true")?;
            tx.commit()?;
            cleanup(&conn)?;
        } else if all.iter().any(|(k, _)| k == CLEANUP) {
            cleanup(&conn)?;
        }
        self.finish_deletes(&conn)?;
        Ok(())
    }
    fn finish_deletes(&self, conn: &Connection) -> Result<()> {
        use rusqlite::OptionalExtension;
        let pending: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key=?1", [DELETES], |r| {
                r.get(0)
            })
            .optional()?;
        if let Some(pending) = pending {
            for reference in serde_json::from_str::<Vec<String>>(&pending)? {
                self.credentials.vault.delete(&reference)?;
                if self.credentials.vault.get(&reference)?.is_some() {
                    return Err(unavailable());
                }
            }
            conn.execute("DELETE FROM settings WHERE key=?1", [DELETES])?;
        }
        Ok(())
    }
    pub(crate) fn check_credentials(&self) -> Result<()> {
        self.migrate_credentials()?;
        let _guard = self.credentials.gate.lock().map_err(|_| unavailable())?;
        let conn = self.conn.lock().map_err(|_| unavailable())?;
        for (key, text) in rows(&conn)? {
            if scalar_key(&key) && !text.is_empty() {
                self.credentials.resolve(&text)?;
            }
            if collection_key(&key) {
                let items: Value = serde_json::from_str(&text)?;
                for item in items.as_array().ok_or_else(unavailable)? {
                    if let Some(reference) = item
                        .get("credential_ref")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        self.credentials.resolve(reference)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn resolve_legacy_credential(&self, value: &str) -> Result<String> {
        if value.is_empty() {
            return Ok(String::new());
        }
        let _guard = self.credentials.gate.lock().map_err(|_| unavailable())?;
        self.credentials.resolve(value)
    }
    pub(crate) fn read_credential_json(&self, key: &str) -> Result<Option<String>> {
        let _guard = self.credentials.gate.lock().map_err(|_| unavailable())?;
        self.migrate_credentials_locked()?;
        let Some(text) = self.get_setting(key)? else {
            return Ok(None);
        };
        let mut value: Value = serde_json::from_str(&text)?;
        for item in value.as_array_mut().ok_or_else(unavailable)? {
            let secret = match item
                .get("credential_ref")
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
            {
                Some(reference) => self.credentials.resolve(reference)?,
                None => String::new(),
            };
            item["api_key"] = Value::String(secret);
            item.as_object_mut()
                .ok_or_else(unavailable)?
                .remove("credential_ref");
        }
        Ok(Some(serde_json::to_string(&value)?))
    }
    #[cfg(test)]
    pub(crate) fn write_credential_json(&self, key: &str, text: &str) -> Result<()> {
        self.write_credential_json_with_kept_ids(key, text, &std::collections::HashSet::new())
    }
    pub(crate) fn write_credential_json_with_kept_ids(
        &self,
        key: &str,
        text: &str,
        kept_ids: &std::collections::HashSet<String>,
    ) -> Result<()> {
        let _guard = self.credentials.gate.lock().map_err(|_| unavailable())?;
        self.migrate_credentials_locked()?;
        let old: Value = self
            .get_setting(key)?
            .map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or(Value::Array(vec![]));
        let mut value: Value = serde_json::from_str(text)?;
        for item in value.as_array_mut().ok_or_else(unavailable)? {
            let secret = normalize(
                item.get("api_key")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            let old_ref = old
                .as_array()
                .and_then(|items| items.iter().find(|old| old.get("id") == item.get("id")))
                .and_then(|old| old.get("credential_ref"))
                .and_then(Value::as_str)
                .filter(|r| !r.is_empty());
            item.as_object_mut()
                .ok_or_else(unavailable)?
                .remove("api_key");
            if item
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| kept_ids.contains(id))
            {
                if let Some(reference) = old_ref {
                    item["credential_ref"] = Value::String(reference.to_owned());
                } else {
                    item.as_object_mut().unwrap().remove("credential_ref");
                }
                continue;
            }
            if secret.is_empty() {
                item.as_object_mut().unwrap().remove("credential_ref");
            } else {
                let reference = match old_ref {
                    Some(r) if self.credentials.resolve(r)? == secret => r.to_owned(),
                    _ => format!("{REF_PREFIX}{}", uuid::Uuid::new_v4()),
                };
                self.credentials.verified_write(&reference, &secret)?;
                item["credential_ref"] = Value::String(reference);
            }
        }
        // Clear/delete removes an unreferenced vault entry after the metadata commit.
        // Replacements retain the old immutable entry so a library rollback still works.
        let mut deletions = Vec::new();
        for previous in old.as_array().ok_or_else(unavailable)? {
            let Some(reference) = previous
                .get("credential_ref")
                .and_then(Value::as_str)
                .filter(|r| !r.is_empty())
            else {
                continue;
            };
            let current = value
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item.get("id") == previous.get("id"));
            if current.is_none_or(|item| item.get("credential_ref").is_none()) {
                deletions.push(reference.to_owned());
            }
        }
        let mut conn = self.conn.lock().map_err(|_| unavailable())?;
        let tx = conn.transaction()?;
        put(&tx, key, &serde_json::to_string(&value)?)?;
        let existing = rows(&tx)?;
        deletions.retain(|reference| {
            !existing
                .iter()
                .any(|(key, text)| key != DELETES && text.contains(reference))
        });
        if !deletions.is_empty() {
            put(&tx, DELETES, &serde_json::to_string(&deletions)?)?;
        }
        tx.commit()?;
        self.finish_deletes(&conn)
    }
}

#[derive(Serialize)]
pub(crate) struct VaultStatus {
    pub ready: bool,
    pub message: Option<&'static str>,
}
#[tauri::command]
pub(crate) fn get_credential_status(db: tauri::State<'_, Database>) -> VaultStatus {
    let result =
        crate::storage::write_operation::<AppError>().and_then(|_permit| db.check_credentials());
    VaultStatus {
        ready: result.is_ok(),
        message: result.err().map(|_| "credential_migration_failed"),
    }
}

#[derive(Serialize)]
pub(crate) struct ProviderProfileDto {
    pub id: String,
    pub name: String,
    pub endpoint_settings: crate::models::EndpointSettings,
    #[serde(flatten)]
    pub credential: CredentialStatus,
}
#[derive(Serialize)]
pub(crate) struct ProviderProfilesDto {
    pub active_provider_id: String,
    pub profiles: Vec<ProviderProfileDto>,
}
impl From<crate::models::ModelProviderProfilesState> for ProviderProfilesDto {
    fn from(state: crate::models::ModelProviderProfilesState) -> Self {
        Self {
            active_provider_id: state.active_provider_id,
            profiles: state
                .profiles
                .into_iter()
                .map(|p| ProviderProfileDto {
                    credential: CredentialStatus::from_secret(&p.api_key),
                    id: p.id,
                    name: p.name,
                    endpoint_settings: p.endpoint_settings,
                })
                .collect(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderProfileWrite {
    pub id: String,
    pub name: String,
    pub endpoint_settings: crate::models::EndpointSettings,
    #[serde(default)]
    pub credential_update: CredentialUpdate,
}
impl ProviderProfileWrite {
    pub(crate) fn apply(
        self,
        previous: &[crate::models::ModelProviderProfile],
    ) -> Result<crate::models::ModelProviderProfile> {
        let key = previous
            .iter()
            .find(|p| p.id == self.id)
            .map(|p| p.api_key.as_str())
            .unwrap_or_default();
        Ok(crate::models::ModelProviderProfile {
            api_key: self.credential_update.apply(key)?,
            id: self.id,
            name: self.name,
            endpoint_settings: self.endpoint_settings,
        })
    }
}
#[derive(Serialize)]
pub(crate) struct LlmConfigDto {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub model: String,
    pub base_url: String,
    pub capability: String,
    pub enabled: bool,
    #[serde(flatten)]
    pub credential: CredentialStatus,
}
impl From<crate::models::LlmConfig> for LlmConfigDto {
    fn from(c: crate::models::LlmConfig) -> Self {
        Self {
            credential: CredentialStatus::from_secret(&c.api_key),
            id: c.id,
            name: c.name,
            protocol: c.protocol,
            model: c.model,
            base_url: c.base_url,
            capability: c.capability,
            enabled: c.enabled,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LlmConfigWrite {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub model: String,
    pub base_url: String,
    pub capability: String,
    pub enabled: bool,
    #[serde(default)]
    pub credential_update: CredentialUpdate,
}
impl LlmConfigWrite {
    pub(crate) fn apply(
        self,
        previous: &[crate::models::LlmConfig],
    ) -> Result<crate::models::LlmConfig> {
        let key = previous
            .iter()
            .find(|c| c.id == self.id)
            .map(|c| c.api_key.as_str())
            .unwrap_or_default();
        Ok(crate::models::LlmConfig {
            api_key: self.credential_update.apply(key)?,
            id: self.id,
            name: self.name,
            protocol: self.protocol,
            model: self.model,
            base_url: self.base_url,
            capability: self.capability,
            enabled: self.enabled,
        })
    }
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
