use super::*;
use rusqlite::{OpenFlags, TransactionBehavior};
use std::{fs, path::PathBuf};

struct Migration {
    version: i32,
    sql: &'static str,
}
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("migrations/01-core-tables.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("migrations/02-conversations.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("migrations/03-generation-soft-delete.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("migrations/04-generation-request-params.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("migrations/05-generation-indexes.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("migrations/06-projects.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("migrations/07-project-conversation-extensions.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("migrations/08-image-folders.sql"),
    },
    Migration {
        version: 9,
        sql: include_str!("migrations/09-logs.sql"),
    },
    Migration {
        version: 10,
        sql: include_str!("migrations/10-generation-recoveries.sql"),
    },
    Migration {
        version: 11,
        sql: include_str!("migrations/11-prompt-favorites.sql"),
    },
    Migration {
        version: 12,
        sql: include_str!("migrations/12-prompt-folders.sql"),
    },
    Migration {
        version: 13,
        sql: include_str!("migrations/13-prompt-extractions.sql"),
    },
    Migration {
        version: 14,
        sql: include_str!("migrations/14-canvas-documents.sql"),
    },
    Migration {
        version: 15,
        sql: include_str!("migrations/15-prompt-agent-sessions.sql"),
    },
    Migration {
        version: 16,
        sql: include_str!("migrations/16-generation-jobs.sql"),
    },
    Migration {
        version: 17,
        sql: include_str!("migrations/17-generation-worker-state.sql"),
    },
    Migration {
        version: 18,
        sql: include_str!("migrations/18-seal-lease.sql"),
    },
    Migration {
        version: 19,
        sql: include_str!("migrations/19-canvas-revisions-and-managed-assets.sql"),
    },
    Migration {
        version: 20,
        sql: include_str!("migrations/20-prompt-agent-visual-context.sql"),
    },
    Migration {
        version: 21,
        sql: include_str!("migrations/21-library-assets.sql"),
    },
];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = MIGRATIONS[MIGRATIONS.len() - 1].version as i64;

/// An ordinary SQLite read-only open can create a WAL index beside a crashed
/// library. The startup library lock excludes competing application writers.
pub(super) fn preflight(path: &Path) -> Result<(), AppError> {
    #[cfg(windows)]
    let vfs = "win32-none";
    #[cfg(not(windows))]
    let vfs = "unix-none";
    let reader = Connection::open_with_flags_and_vfs(path, OpenFlags::SQLITE_OPEN_READ_ONLY, vfs)?;
    reader.execute_batch("PRAGMA locking_mode=EXCLUSIVE;")?;
    let probe = reader.query_row("PRAGMA schema_version", [], |row| row.get::<_, i64>(0));
    if let Err(rusqlite::Error::SqliteFailure(code, _)) = &probe {
        if code.extended_code == rusqlite::ffi::SQLITE_READONLY_ROLLBACK {
            // A hot rollback journal may contain the only committed version.
            // Replay a private copy before authorizing recovery of the original.
            drop(reader);
            let temp = crate::storage::temporary(&std::env::temp_dir(), "astro-preflight")?;
            let copy = temp.0.join("library.db");
            let mut journal = path.as_os_str().to_owned();
            journal.push("-journal");
            fs::copy(path, &copy)?;
            fs::copy(journal, temp.0.join("library.db-journal"))?;
            let recovered = Connection::open(&copy)?;
            supported_version(&recovered)?;
            return Ok(());
        }
    }
    probe?;
    supported_version(&reader)?;
    Ok(())
}

pub(super) fn supported_version(conn: &Connection) -> Result<i64, AppError> {
    let table: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    let recorded = if table {
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get::<_, i64>(0),
        )?
    } else {
        0
    };
    let header: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let found = recorded.max(header);
    if found > LATEST_SCHEMA_VERSION {
        return Err(AppError::UnsupportedSchema {
            found,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    Ok(recorded)
}

// Hold the writer reservation while a separate reader snapshots the last committed
// state, including WAL pages. No migration ever modifies an existing asset file.
fn snapshot(database: &Path, version: i64) -> Result<PathBuf, AppError> {
    let parent = database.parent().ok_or_else(|| AppError::FileSystem {
        message: "Missing library directory".into(),
    })?;
    let directory = parent.join("migration-backups");
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(&directory) {
        Ok(()) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::AlreadyExists
                && directory.symlink_metadata()?.is_dir() => {}
        Err(e) => return Err(e.into()),
    }
    let stem = format!("v{version}-{}", uuid::Uuid::new_v4());
    let target = directory.join(format!("{stem}.sqlite3"));
    let temporary = directory.join(format!("{stem}.partial"));
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    drop(options.open(&temporary)?);
    let result = (|| -> Result<(), AppError> {
        let source = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        // Copy only live rows. A byte-for-byte backup also retains deleted data
        // on the database freelist, including secrets from historical clients.
        source.execute(
            "VACUUM INTO ?1",
            [temporary.to_str().ok_or_else(|| AppError::FileSystem {
                message: "Cannot encode snapshot path".into(),
            })?],
        )?;
        let destination =
            Connection::open_with_flags(&temporary, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let check: String = destination.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" || supported_version(&destination)? != version {
            return Err(AppError::Database {
                message: "Pre-upgrade snapshot verification failed".into(),
            });
        }
        drop(destination);
        fs::OpenOptions::new()
            .write(true)
            .open(&temporary)?
            .sync_all()?;
        crate::atomic_files::publish_no_replace(&temporary, &target)?;
        #[cfg(unix)]
        {
            fs::File::open(&directory)?.sync_all()?;
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(target)
}

fn verify_lease(conn: &Connection) -> Result<(), AppError> {
    let valid: bool = conn.query_row(
        "SELECT COUNT(*)=1 AND MIN(id)=1 FROM generation_worker_lease",
        [],
        |r| r.get(0),
    )?;
    if !valid {
        return Err(AppError::Database {
            message: "migration 18 requires exactly one generation worker lease row with id 1"
                .into(),
        });
    }
    Ok(())
}

pub(super) fn upgrade(conn: &mut Connection, root: Option<&Path>) -> Result<(), AppError> {
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    supported_version(conn)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version = supported_version(&tx)?;
    let populated: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%')",
        [], |r| r.get(0),
    )?;
    if populated
        && version < LATEST_SCHEMA_VERSION
        && crate::credentials::allows_private_snapshot(&tx)?
    {
        let path = tx.path().ok_or_else(|| AppError::FileSystem {
            message: "Cannot determine library path before upgrade".into(),
        })?;
        if !path.is_empty() {
            let snapshot = snapshot(Path::new(path), version)?;
            log::info!(
                "Saved pre-upgrade library database at {}",
                snapshot.display()
            );
        }
    }
    ensure_schema_migrations(&tx)?;
    for migration in MIGRATIONS {
        if !migration_applied(&tx, migration.version)? {
            match migration.version {
                18 => {
                    verify_lease(&tx)?;
                    tx.execute_batch(migration.sql)?;
                }
                21.. => tx.execute_batch(migration.sql)?,
                _ => execute_migration_sql(&tx, migration.sql)?,
            }
            record_migration(&tx, migration.version)?;
        }
        if migration.version == 17 {
            tx.execute_batch(include_str!("migrations/lease-fence.sql"))?;
        }
    }
    // Some historic v7 builds recorded the migration before adding every column.
    execute_migration_sql(
        &tx,
        include_str!("migrations/07-project-conversation-extensions.sql"),
    )?;
    if let Some(root) = root {
        crate::storage::assets::bind_root(&tx, root)?;
    }
    tx.commit()?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(())
}

fn ensure_schema_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .map_err(|e| AppError::Database {
        message: format!("Create schema_migrations failed: {}", e),
    })
}

fn migration_applied(conn: &Connection, version: i32) -> Result<bool, AppError> {
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
            params![version],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Database {
            message: format!("Check migration version {version} failed: {e}"),
        })?;
    Ok(count > 0)
}

fn record_migration(conn: &Connection, version: i32) -> Result<(), AppError> {
    let inserted = conn
        .execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)
             ON CONFLICT(version) DO NOTHING",
            params![version, crate::current_timestamp()],
        )
        .map_err(|e| AppError::Database {
            message: format!("record migration {version}: {e}"),
        })?;
    if inserted == 0 && !migration_applied(conn, version)? {
        return Err(AppError::Database {
            message: format!("record migration {version}: version row was not persisted"),
        });
    }
    Ok(())
}

fn execute_migration_sql(conn: &Connection, sql: &str) -> Result<(), AppError> {
    for statement in sql
        .split(';')
        .map(str::trim)
        .filter(|stmt| !stmt.is_empty())
    {
        conn.execute_batch(statement).or_else(|e| {
            let msg = e.to_string();
            if msg.contains("already exists") || msg.contains("duplicate column") {
                Ok(())
            } else {
                Err(AppError::Database {
                    message: format!("Migration SQL failed: {}", msg),
                })
            }
        })?;
    }

    Ok(())
}
