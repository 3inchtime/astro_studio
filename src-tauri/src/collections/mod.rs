pub(crate) mod images;
pub(crate) mod prompts;

use crate::{current_timestamp, models::Folder};
use rusqlite::{params, Connection};

/// Folder names and join columns are fixed schema identifiers, never user input.
struct FolderTable {
    folders: &'static str,
    members: &'static str,
    item: &'static str,
}

const IMAGE_FOLDERS: FolderTable = FolderTable {
    folders: "folders",
    members: "folder_images",
    item: "image_id",
};
const PROMPT_FOLDERS: FolderTable = FolderTable {
    folders: "prompt_folders",
    members: "prompt_folder_favorites",
    item: "prompt_favorite_id",
};

impl FolderTable {
    fn create(&self, conn: &Connection, name: String) -> rusqlite::Result<Folder> {
        let folder = Folder {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: current_timestamp(),
        };
        conn.execute(
            &format!(
                "INSERT INTO {} (id, name, created_at) VALUES (?1, ?2, ?3)",
                self.folders
            ),
            params![folder.id, folder.name, folder.created_at],
        )?;
        Ok(folder)
    }

    fn rename(&self, conn: &Connection, id: &str, name: &str) -> rusqlite::Result<()> {
        conn.execute(
            &format!("UPDATE {} SET name = ?1 WHERE id = ?2", self.folders),
            params![name, id],
        )?;
        Ok(())
    }

    fn delete(&self, conn: &Connection, id: &str) -> rusqlite::Result<()> {
        conn.execute(&format!("DELETE FROM {} WHERE id = ?1", self.folders), [id])?;
        Ok(())
    }

    fn list(&self, conn: &Connection) -> rusqlite::Result<Vec<Folder>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT id,name,created_at FROM {} ORDER BY created_at ASC",
            self.folders
        ))?;
        let rows = stmt.query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    fn add(&self, conn: &Connection, id: &str, folders: &[String]) -> rusqlite::Result<()> {
        let tx = conn.unchecked_transaction()?;
        let sql = format!(
            "INSERT OR IGNORE INTO {} (folder_id, {}, added_at) VALUES (?1, ?2, ?3)",
            self.members, self.item
        );
        let timestamp = current_timestamp();
        for folder in folders {
            tx.execute(&sql, params![folder, id, timestamp])?;
        }
        tx.commit()
    }

    fn remove(&self, conn: &Connection, id: &str, folders: &[String]) -> rusqlite::Result<()> {
        let tx = conn.unchecked_transaction()?;
        let sql = format!(
            "DELETE FROM {} WHERE folder_id = ?1 AND {} = ?2",
            self.members, self.item
        );
        for folder in folders {
            tx.execute(&sql, params![folder, id])?;
        }
        tx.commit()
    }

    fn containing(&self, conn: &Connection, id: &str) -> rusqlite::Result<Vec<String>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT folder_id FROM {} WHERE {} = ?1",
            self.members, self.item
        ))?;
        let rows = stmt.query_map([id], |row| row.get(0))?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn folder_batch_failure_is_atomic_and_favorite_search_keeps_folder_scope() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.run_migrations().unwrap();
        let conn = db.conn.lock().unwrap();
        conn.execute_batch("INSERT INTO generations(id,prompt) VALUES ('red','red flower'),('blue','blue flower');
            INSERT INTO images(id,generation_id,file_path) VALUES ('one','red','/red.png'),('two','blue','/blue.png');").unwrap();
        let folder = images::create_folder(&conn, "Test".into()).unwrap();
        assert!(images::add_image_to_folders(
            &conn,
            "one".into(),
            vec![folder.id.clone(), "missing".into()]
        )
        .is_err());
        assert!(images::get_image_folders(&conn, "one".into())
            .unwrap()
            .is_empty());
        images::add_image_to_folders(&conn, "one".into(), vec![folder.id.clone()]).unwrap();
        images::add_image_to_folders(&conn, "two".into(), vec!["default".into()]).unwrap();
        for (scope, query, total) in [
            (None, None, 2),
            (None, Some("blue"), 1),
            (Some(folder.id.as_str()), None, 1),
            (Some(folder.id.as_str()), Some("blue"), 0),
        ] {
            let found = images::get_favorite_images(
                &conn,
                scope.map(str::to_owned),
                query.map(str::to_owned),
                None,
            )
            .unwrap();
            assert_eq!(found.total, total);
            assert_eq!(found.generations.len(), total as usize);
        }
        let prompt = prompts::create_prompt_favorite(&conn, "orchid".into()).unwrap();
        let prompt_folder = prompts::create_prompt_folder(&conn, "Prompts".into()).unwrap();
        prompts::add_prompt_favorite_to_folders(
            &conn,
            prompt.id.clone(),
            vec![prompt_folder.id.clone()],
        )
        .unwrap();
        for (scope, query, total) in [
            (None, None, 1),
            (None, Some("fern"), 0),
            (Some(prompt_folder.id.as_str()), None, 1),
            (Some(prompt_folder.id.as_str()), Some("orch"), 1),
        ] {
            let rows = prompts::get_prompt_favorites(
                &conn,
                query.map(str::to_owned),
                scope.map(str::to_owned),
            )
            .unwrap();
            assert_eq!(rows.len(), total);
        }
    }
}
