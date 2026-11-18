//! File references declared by each library domain, traversed by archive export and restore.
use rusqlite::{params, Connection};
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Copy)]
pub(crate) enum ArtifactKind {
    File,
    Document,
    Expected,
    PathList,
    Structured,
}

pub(crate) struct ArtifactColumn {
    pub(crate) table: &'static str,
    key: &'static str,
    pub(crate) field: &'static str,
    pub(crate) kind: ArtifactKind,
}
impl ArtifactColumn {
    pub(crate) const fn new(
        table: &'static str,
        key: &'static str,
        field: &'static str,
        kind: ArtifactKind,
    ) -> Self {
        Self {
            table,
            key,
            field,
            kind,
        }
    }
}

pub(crate) fn path_key(key: &str) -> bool {
    matches!(
        key,
        "image_path"
            | "imagePath"
            | "sourceImagePath"
            | "sourceImagePaths"
            | "source_image_path"
            | "maskPath"
            | "markupPath"
            | "mask_path"
            | "markup_path"
            | "source_image_paths"
    )
}
pub(crate) fn map_json_paths(
    value: &mut Value,
    f: &mut impl FnMut(&str) -> Result<String>,
) -> Result<()> {
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

pub(crate) fn transform_database_paths(
    conn: &Connection,
    map: &mut impl FnMut(&str, bool, &ArtifactColumn) -> Result<String>,
    document: &mut impl FnMut(&str, &str) -> Result<()>,
) -> Result<()> {
    for col in [
        crate::gallery::GALLERY_ARTIFACTS,
        crate::canvas_assets::CANVAS_ARTIFACTS,
        crate::llm::prompts::EXTRACTIONS_ARTIFACTS,
        crate::generation_recipe::RECIPES_ARTIFACTS,
        crate::generation_recovery::RECOVERIES_ARTIFACTS,
    ]
    .into_iter()
    .flatten()
    {
        let ArtifactColumn {
            table,
            key,
            field,
            kind,
        } = col;
        let mut select = conn.prepare(&format!(
            "SELECT {key},{field} FROM {table} WHERE {field} IS NOT NULL AND {field}<>''"
        ))?;
        let rows = select
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut update = conn.prepare(&format!("UPDATE {table} SET {field}=?1 WHERE {key}=?2"))?;
        for (id, old) in rows {
            let new = match kind {
                ArtifactKind::PathList => {
                    let mut list: Vec<String> = serde_json::from_str(&old)?;
                    for path in &mut list {
                        *path = map(path, false, col)?;
                    }
                    serde_json::to_string(&list)?
                }
                ArtifactKind::Structured => {
                    let mut value = serde_json::from_str(&old)?;
                    map_json_paths(&mut value, &mut |path| map(path, false, col))?;
                    serde_json::to_string(&value)?
                }
                ArtifactKind::Document => {
                    let new = map(&old, false, col)?;
                    document(&old, &new)?;
                    new
                }
                ArtifactKind::Expected => map(&old, true, col)?,
                ArtifactKind::File => map(&old, false, col)?,
            };
            update.execute(params![new, id])?;
        }
    }
    Ok(())
}
