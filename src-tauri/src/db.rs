use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

fn migrate_step(conn: &Connection, sql: &str) -> Result<(), String> {
    conn.execute_batch(sql).or_else(|e| {
        let msg = e.to_string();
        if msg.contains("already exists") || msg.contains("duplicate column") {
            Ok(())
        } else {
            Err(format!("Migration failed: {}", msg))
        }
    })
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Create db dir failed: {}", e))?;
        }
        let conn =
            Connection::open(path).map_err(|e| format!("Open db failed: {}", e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("PRAGMA failed: {}", e))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS generations (
                id TEXT PRIMARY KEY,
                prompt TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT 'gpt-image-2',
                size TEXT NOT NULL DEFAULT '1024x1024',
                quality TEXT NOT NULL DEFAULT 'auto',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS images (
                id TEXT PRIMARY KEY,
                generation_id TEXT NOT NULL REFERENCES generations(id) ON DELETE CASCADE,
                file_path TEXT NOT NULL,
                thumbnail_path TEXT,
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                file_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_images_generation_id ON images(generation_id);
            CREATE INDEX IF NOT EXISTS idx_generations_created_at ON generations(created_at);
            "#,
        )
        .map_err(|e| format!("Migration failed: {}", e))?;

        migrate_step(&conn,
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        )?;

        migrate_step(&conn,
            "ALTER TABLE generations ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;"
        )?;

        migrate_step(&conn,
            "CREATE INDEX IF NOT EXISTS idx_generations_conversation_id ON generations(conversation_id);"
        )?;

        migrate_step(&conn,
            "CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);"
        )?;

        migrate_step(&conn,
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );"
        )?;

        migrate_step(&conn,
            "CREATE TABLE IF NOT EXISTS folder_images (
                folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
                image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (folder_id, image_id)
            );"
        )?;

        migrate_step(&conn,
            "CREATE INDEX IF NOT EXISTS idx_folder_images_image_id ON folder_images(image_id);"
        )?;

        migrate_step(&conn,
            "INSERT OR IGNORE INTO folders (id, name, created_at) VALUES ('default', '默认收藏', datetime('now'));"
        )?;

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(&format!("SELECT value FROM settings WHERE key = '{}'", key))
            .map_err(|e| e.to_string())?;
        Ok(stmt
            .query_row([], |row| row.get::<_, String>(0))
            .ok())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
