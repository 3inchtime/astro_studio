//! Image identity shared by generation, canvas and gallery. Source tables retain
//! their native primary keys, including libraries imported from older versions.
use crate::error::AppError;
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use std::{collections::HashSet, path::Path};

#[derive(Debug, Serialize)]
pub(crate) struct ImageAsset {
    pub id: String,
    pub file_path: String,
}

fn portable_prefix(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_owned()
}

/// Current and previous root spellings for resolving paths in immutable canvas revisions.
pub(crate) struct FormerRoots(Vec<(String, bool)>);

enum PathUse {
    Document,
    RelocatedAsset,
    RelocatedRecovery,
}

impl FormerRoots {
    pub(crate) fn read(conn: &Connection) -> Result<Self, AppError> {
        // A moved library may sit inside a former root. Match the most specific
        // spelling so a current path is never relocated a second time.
        let mut stmt = conn.prepare(
            "SELECT prefix, MAX(active) FROM (
                SELECT prefix, 0 AS active FROM former_asset_roots
                UNION ALL SELECT prefix, 1 FROM library_asset_roots
            ) GROUP BY prefix ORDER BY length(prefix) DESC",
        )?;
        let values = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(Self(values.collect::<rusqlite::Result<_>>()?))
    }

    pub(crate) fn resolve(&self, root: &Path, path: &str) -> Result<String, AppError> {
        self.locate(root, path, PathUse::Document)
    }

    fn relative_path<'a>(&self, normalized: &'a str) -> Option<(&'a str, bool)> {
        for (prefix, active) in &self.0 {
            let Some(start) = normalized.get(..prefix.len()) else {
                continue;
            };
            let windows = prefix.starts_with("//") || prefix.as_bytes().get(1) == Some(&b':');
            if start == prefix || windows && start.eq_ignore_ascii_case(prefix) {
                if let Some(relative) = normalized[prefix.len()..].strip_prefix('/') {
                    return Some((relative, *active));
                }
            }
        }
        None
    }

    fn locate(&self, root: &Path, path: &str, purpose: PathUse) -> Result<String, AppError> {
        let normalized = path.replace('\\', "/");
        let Some((relative, active)) = self.relative_path(&normalized) else {
            return Ok(path.to_owned());
        };
        if active && matches!(purpose, PathUse::Document) {
            return Ok(path.to_owned());
        }
        let location = managed_location(root, relative)?;
        // Source-aware retries bind to canonical absolute names when the file exists.
        let resolved = match location.canonicalize() {
            Ok(real) => {
                if !real.starts_with(root.canonicalize()?) {
                    return Err(AppError::Validation {
                        message: "Relocated asset symlink escapes library".into(),
                    });
                }
                real
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => location,
            Err(error) => return Err(error.into()),
        };
        Ok(match purpose {
            PathUse::RelocatedRecovery => root.join(relative),
            _ => resolved,
        }
        .to_string_lossy()
        .into_owned())
    }

    pub(crate) fn insert_references(&self, paths: &mut AssetPaths, absolute: &str) {
        paths.insert(absolute);
        let normalized = absolute.replace('\\', "/");
        if let Some((relative, true)) = self.relative_path(&normalized) {
            self.extend_references(paths, relative);
        }
    }

    fn extend_references(&self, paths: &mut AssetPaths, relative: &str) {
        for (prefix, _) in &self.0 {
            paths.insert(&format!("{prefix}/{relative}"));
        }
    }
}

fn managed_location(root: &Path, relative: &str) -> Result<std::path::PathBuf, AppError> {
    if relative
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == ".." || p.contains([':', '\\']))
    {
        return Err(AppError::Validation {
            message: "Invalid managed asset location".into(),
        });
    }
    Ok(root.join(relative))
}

/// Called under the migration or restore transaction, before serving the library.
pub(crate) fn bind_root(conn: &Connection, root: &Path) -> Result<(), AppError> {
    let mut prefixes = vec![root.to_path_buf()];
    if root.is_absolute() {
        if let Ok(real) = root.canonicalize() {
            if real != root {
                prefixes.push(real);
            }
        }
    }
    let prefixes = prefixes
        .into_iter()
        .map(|p| portable_prefix(&p.to_string_lossy()))
        .collect::<std::collections::BTreeSet<_>>();
    let existing = conn
        .prepare("SELECT prefix FROM library_asset_roots")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
    if existing == prefixes {
        return Ok(());
    }
    let relocated = root.is_absolute() && !existing.is_empty() && !existing.contains("assets");
    let source_roots = relocated.then(|| FormerRoots::read(conn)).transpose()?;
    if relocated {
        for prefix in existing.difference(&prefixes) {
            conn.execute(
                "INSERT OR IGNORE INTO former_asset_roots(prefix) VALUES(?1)",
                [prefix],
            )?;
        }
    }
    conn.execute("DELETE FROM library_asset_roots", [])?;
    for prefix in prefixes {
        conn.execute(
            "INSERT INTO library_asset_roots(prefix) VALUES(?1)",
            [prefix],
        )?;
    }
    if let Some(aliases) = source_roots {
        crate::storage::artifacts::transform_database_paths(
            conn,
            &mut |old, _, column| {
                let purpose = if column.table == "generation_recoveries" {
                    PathUse::RelocatedRecovery
                } else {
                    PathUse::RelocatedAsset
                };
                Ok(aliases.locate(root, old, purpose)?)
            },
            &mut |_, _| Ok(()),
        )
        .map_err(|e| AppError::Database {
            message: format!("Relocate library assets: {e}"),
        })?;
    }
    for (table, kind) in [("images", "generated"), ("canvas_assets", "canvas")] {
        conn.execute(&format!("UPDATE {table} SET managed_path=(SELECT relative_path FROM library_relative_paths p WHERE p.kind=?1 AND p.id={table}.id LIMIT 1) WHERE managed_path IS NULL"), [kind])?;
    }
    Ok(())
}

pub(crate) fn resolve_document_asset(
    conn: &Connection,
    id: &str,
    root: &Path,
    old: &str,
) -> Result<Option<String>, AppError> {
    let mut stmt = conn
        .prepare("SELECT file_path,managed_path FROM library_assets WHERE id=?1 ORDER BY kind")?;
    let rows = stmt
        .query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let row = if rows.len() <= 1 || old.is_empty() {
        rows.into_iter().next()
    } else {
        // A v19 import and a generated image could share the same historical ID.
        // Preserve the document's original target when the ID alone is ambiguous.
        let mut aliases = AssetPaths::default();
        aliases.insert(old);
        rows.into_iter()
            .find(|(native, _)| aliases.contains(native))
    };
    row.map(|(native, relative)| {
        if let Some(relative) = relative {
            return Ok(managed_location(root, &relative)?
                .to_string_lossy()
                .into_owned());
        }
        Ok(native)
    })
    .transpose()
}

#[cfg(test)]
pub(crate) fn resolve_asset(conn: &Connection, id: &str) -> Result<Option<String>, AppError> {
    Ok(conn
        .query_row(
            "SELECT file_path FROM library_assets WHERE id=?1 ORDER BY kind LIMIT 1",
            [id],
            |row| row.get(0),
        )
        .optional()?)
}

fn row_asset(row: &rusqlite::Row) -> rusqlite::Result<ImageAsset> {
    Ok(ImageAsset {
        id: row.get(0)?,
        file_path: row.get(1)?,
    })
}

pub(crate) fn asset_for_path(
    db: &crate::db::Database,
    path: &str,
) -> Result<Option<ImageAsset>, AppError> {
    let candidates = {
        let conn = db.connection()?;
        if let Some(exact)= conn.query_row("SELECT id,file_path FROM library_assets WHERE file_path=?1 ORDER BY kind,id LIMIT 1",[path],row_asset).optional()? {
            return Ok(Some(exact));
        }
        // Restrict filesystem inspection to names that could denote this legacy file.
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let mut stmt=conn.prepare("SELECT id,file_path FROM library_assets WHERE lower(substr(file_path,-length(?1)))=lower(?1) ORDER BY kind,id")?;
        let rows = stmt.query_map([name], row_asset)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let mut paths = AssetPaths::default();
    paths.insert(path);
    Ok(candidates
        .into_iter()
        .find(|a| paths.contains(&a.file_path)))
}

fn path_key(path: &str) -> String {
    // Only apply Windows rules to unambiguously Windows file names.
    let bytes = path.as_bytes();
    if path.starts_with(r"\\")
        || path.starts_with("//")
        || (bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
    {
        let normalized = path.replace('/', r"\");
        let normalized = normalized.strip_prefix(r"\\?\").unwrap_or(&normalized);
        if let Some(unc) = normalized.strip_prefix(r"UNC\") {
            return format!(r"\\{unc}").to_lowercase();
        }
        normalized.to_lowercase()
    } else {
        path.to_owned()
    }
}

#[derive(Default)]
pub(crate) struct AssetPaths(HashSet<String>);
impl AssetPaths {
    pub(crate) fn insert(&mut self, path: &str) {
        if !path.is_empty() {
            self.0.insert(path_key(path));
            if let Ok(real) = Path::new(path).canonicalize() {
                self.0.insert(path_key(&real.to_string_lossy()));
            }
        }
    }

    pub(crate) fn contains(&self, path: &str) -> bool {
        !path.is_empty()
            && (self.0.contains(&path_key(path))
                || Path::new(path)
                    .canonicalize()
                    .is_ok_and(|real| self.0.contains(&path_key(&real.to_string_lossy()))))
    }
}
