---
type: concept
title: "IPC Communication"
created: 2026-05-09
updated: 2026-05-09
tags: [architecture, ipc, tauri]
status: active
---

# IPC Communication

## 概述

Astro Studio 的前后端通信完全基于 Tauri 的 IPC 机制，不使用 REST API。

## 命令调用 (Frontend → Backend)

### 机制

前端通过 `@tauri-apps/api/core` 的 `invoke()` 函数调用后端命令：

```typescript
// src/lib/api.ts
import { invoke } from "@tauri-apps/api/core";

export async function generateImage(params: GenerationParams): Promise<GenerateResponse> {
  return invoke("generate_image", {
    prompt: params.prompt,
    model: params.model,
    // ...
  });
}
```

后端通过 `#[tauri::command]` 注解暴露命令：

```rust
#[tauri::command]
pub(crate) async fn generate_image(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    engine: tauri::State<'_, GptImageEngine>,
    config: tauri::State<'_, AppConfig>,
    prompt: String,
    model: Option<String>,
    // ...
) -> Result<GenerateResult, String> {
    // ...
}
```

### 参数映射

前端 camelCase 参数自动映射到后端 snake_case：

| 前端 (TypeScript) | 后端 (Rust) |
|-------------------|-------------|
| `outputFormat` | `output_format` |
| `imageCount` | `image_count` |
| `conversationId` | `conversation_id` |

### 命令注册

所有命令在 `lib.rs` 的 `invoke_handler` 中注册：

```rust
.invoke_handler(tauri::generate_handler![
    commands::settings::save_api_key,
    commands::settings::get_api_key,
    commands::generation::generate_image,
    // ... 60+ 命令
])
```

## 事件通信 (Backend → Frontend)

### 机制

后端通过 Tauri 的 `Emitter` 发射事件，前端通过 `listen()` 订阅：

```rust
// 后端发射事件
app.emit("generation:progress", serde_json::json!({
    "generation_id": generation_id,
    "status": "processing"
}))?;
```

```typescript
// 前端订阅事件
import { listen } from "@tauri-apps/api/event";

listen("generation:progress", (e) => {
  handler(e.payload as { generation_id: string; status: string });
});
```

### 事件类型

| 事件名 | 数据 | 说明 |
|--------|------|------|
| `generation:progress` | `{ generation_id, status }` | 生成进度更新 |
| `generation:complete` | `{ generation_id, status }` | 生成完成 |
| `generation:failed` | `{ generation_id, error }` | 生成失败 |
| `runtime-log:new` | `RuntimeLogEntry` | 实时运行日志 |

## 状态管理

Tauri 的 `.manage()` 注入后端状态，命令通过 `tauri::State<T>` 访问：

```rust
// 注册状态
.manage(app_config)
.manage(database)
.manage(engine)

// 命令中访问
async fn generate_image(
    db: tauri::State<'_, Database>,
    engine: tauri::State<'_, GptImageEngine>,
    // ...
) { ... }
```

## 文件访问

本地文件路径通过 `convertFileSrc()` 转换为 WebView 可访问的 URL：

```typescript
import { convertFileSrc } from "@tauri-apps/api/core";

const assetUrl = convertFileSrc(filePath);
// → "asset://localhost/path/to/file"
```

需要在 `tauri.conf.json` 中启用 asset protocol：

```json
{
  "security": {
    "assetProtocol": {
      "enable": true,
      "scope": { "allow": ["**"] }
    }
  }
}
```

## API 层封装

`src/lib/api.ts` 是前端 IPC 的唯一入口，每个函数 1:1 映射到 Rust 命令。React Query hooks 在 `src/lib/queries/` 中添加缓存/失效策略。

```
UI Component
    ↓
React Query Hook (queries/*.ts)
    ↓
API Function (api.ts)
    ↓
invoke() → Rust Command
```

## 相关页面

- [[System Architecture]] - 系统架构总览
- [[Frontend Architecture]] - 前端架构详解
- [[Backend Architecture]] - 后端架构详解
