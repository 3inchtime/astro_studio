---
type: concept
title: "Database Migrations"
created: 2026-05-09
updated: 2026-05-09
tags: [history, database, migrations]
status: active
---

# Database Migrations

## 迁移机制

Astro Studio 使用版本化迁移，通过 `schema_migrations` 表追踪已应用的版本。

```rust
fn apply_migration(conn: &Connection, version: i32, _description: &str, sql: &str) -> Result<(), AppError> {
    if migration_applied(conn, version)? {
        return Ok(());  // 已应用，跳过
    }
    execute_migration_sql(conn, sql)?;  // 执行 SQL
    record_migration(conn, version)     // 记录版本
}
```

## 迁移历史

### v1: 核心表

```sql
-- 生成记录
CREATE TABLE generations (
    id TEXT PRIMARY KEY,
    prompt TEXT NOT NULL,
    engine TEXT NOT NULL DEFAULT 'gpt-image-2',
    size TEXT NOT NULL DEFAULT '1024x1024',
    quality TEXT NOT NULL DEFAULT 'auto',
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 生成图片
CREATE TABLE images (
    id TEXT PRIMARY KEY,
    generation_id TEXT NOT NULL REFERENCES generations(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- 设置
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### v2: 对话系统

```sql
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE generations ADD COLUMN conversation_id TEXT REFERENCES conversations(id);
ALTER TABLE generations ADD COLUMN error_message TEXT;
```

### v3: 软删除

```sql
ALTER TABLE generations ADD COLUMN deleted_at TEXT;
```

### v4: 生成请求参数

```sql
ALTER TABLE generations ADD COLUMN request_kind TEXT NOT NULL DEFAULT 'generate';
ALTER TABLE generations ADD COLUMN background TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE generations ADD COLUMN output_format TEXT NOT NULL DEFAULT 'png';
ALTER TABLE generations ADD COLUMN output_compression INTEGER NOT NULL DEFAULT 100;
ALTER TABLE generations ADD COLUMN moderation TEXT NOT NULL DEFAULT 'auto';
ALTER TABLE generations ADD COLUMN input_fidelity TEXT NOT NULL DEFAULT 'high';
ALTER TABLE generations ADD COLUMN image_count INTEGER NOT NULL DEFAULT 1;
ALTER TABLE generations ADD COLUMN source_image_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE generations ADD COLUMN source_image_paths TEXT NOT NULL DEFAULT '[]';
ALTER TABLE generations ADD COLUMN request_metadata TEXT;
```

### v5: 索引优化

```sql
CREATE INDEX idx_generations_engine ON generations(engine);
CREATE INDEX idx_generations_request_kind ON generations(request_kind);
CREATE INDEX idx_generations_size ON generations(size);
CREATE INDEX idx_generations_quality ON generations(quality);
CREATE INDEX idx_generations_output_format ON generations(output_format);
CREATE INDEX idx_conversations_updated_at ON conversations(updated_at);
```

### v6: 项目系统

```sql
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    archived_at TEXT,
    pinned_at TEXT,
    deleted_at TEXT
);

INSERT INTO projects (id, name) VALUES ('default', 'Default Project');

ALTER TABLE conversations ADD COLUMN project_id TEXT REFERENCES projects(id);
```

### v7: 项目/对话扩展

```sql
ALTER TABLE projects ADD COLUMN deleted_at TEXT;
ALTER TABLE conversations ADD COLUMN archived_at TEXT;
ALTER TABLE conversations ADD COLUMN pinned_at TEXT;
ALTER TABLE conversations ADD COLUMN deleted_at TEXT;

UPDATE conversations SET project_id = 'default' WHERE project_id IS NULL;
```

### v8: 图片文件夹

```sql
CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE folder_images (
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY (folder_id, image_id)
);

INSERT INTO folders (id, name) VALUES ('default', '默认收藏');
```

### v9: 日志系统

```sql
CREATE TABLE logs (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    log_type TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    message TEXT NOT NULL,
    generation_id TEXT,
    metadata TEXT,
    response_file TEXT
);
```

### v10: 生成恢复

```sql
CREATE TABLE generation_recoveries (
    generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
    request_kind TEXT NOT NULL,
    request_state TEXT NOT NULL,
    output_format TEXT NOT NULL,
    response_file TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### v11: 提示词收藏

```sql
CREATE TABLE prompt_favorites (
    id TEXT PRIMARY KEY,
    prompt TEXT NOT NULL COLLATE NOCASE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_prompt_favorites_prompt ON prompt_favorites(prompt);
```

### v12: 提示词文件夹

```sql
CREATE TABLE prompt_folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE prompt_folder_favorites (
    folder_id TEXT NOT NULL REFERENCES prompt_folders(id) ON DELETE CASCADE,
    prompt_favorite_id TEXT NOT NULL REFERENCES prompt_favorites(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL,
    PRIMARY KEY (folder_id, prompt_favorite_id)
);

INSERT INTO prompt_folders (id, name) VALUES ('default', '默认收藏夹');
```

## 迁移特性

- **幂等性** - 使用 `IF NOT EXISTS` 和 `already exists` 错误处理
- **版本追踪** - 通过 `schema_migrations` 表记录
- **增量执行** - 只执行未应用的迁移
- **向后兼容** - 新列使用 `DEFAULT` 值

## 相关页面

- [[Database Schema]] - 当前数据库设计
- [[Version History]] - 版本记录
