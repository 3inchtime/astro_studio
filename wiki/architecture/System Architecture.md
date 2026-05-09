---
type: concept
title: "System Architecture"
created: 2026-05-09
updated: 2026-05-09
tags: [architecture, system]
status: active
---

# System Architecture

## 整体架构

Astro Studio 采用 Tauri 2.0 的双进程架构：

```
┌─────────────────────────────────────────────┐
│                 Frontend (WebView)            │
│  React + TypeScript + Tailwind CSS           │
│  ┌─────────┐ ┌──────────┐ ┌──────────────┐  │
│  │  Pages   │ │Components│ │  React Query  │  │
│  └────┬─────┘ └────┬─────┘ └──────┬───────┘  │
│       └─────────────┼──────────────┘          │
│                     │ invoke()                │
├─────────────────────┼─────────────────────────┤
│               IPC Bridge (Tauri)              │
├─────────────────────┼─────────────────────────┤
│                 Backend (Rust)                │
│  ┌──────────┐ ┌──────────┐ ┌──────────────┐  │
│  │ Commands │ │  Engine   │ │   Database    │  │
│  └────┬─────┘ └────┬─────┘ └──────┬───────┘  │
│       └─────────────┼──────────────┘          │
│                     │                         │
│  ┌──────────────────┴──────────────────────┐  │
│  │         Tauri Managed State             │  │
│  │  AppConfig | Database | GptImageEngine  │  │
│  └─────────────────────────────────────────┘  │
└─────────────────────────────────────────────┘
```

## 进程模型

### 前端进程 (WebView)
- React SPA 运行在 WebView 中
- 通过 `@tauri-apps/api/core` 的 `invoke()` 调用后端命令
- 通过 `listen()` 订阅后端事件
- 使用 `convertFileSrc()` 将本地文件路径转为可访问的 asset URL

### 后端进程 (Rust)
- Tauri 命令处理器，通过 `#[tauri::command]` 注解暴露给前端
- 管理状态：`AppConfig`、`Database`、`GptImageEngine` 通过 `.manage()` 注入
- 事件发射器：通过 `Emitter` 向前端发送进度/完成/失败事件

## 模块结构

```
src-tauri/src/
├── lib.rs              # 应用入口，Tauri builder 设置
├── main.rs             # 程序入口点
├── config.rs           # TOML 配置管理
├── db.rs               # SQLite 数据库操作
├── error.rs            # 类型化错误处理
├── models.rs           # 共享数据模型与常量
├── api_gateway.rs      # ImageEngine trait 定义与 GptImageEngine 实现
├── model_registry.rs   # 模型归一化与分类
├── file_manager.rs     # 图片文件存储与缩略图生成
├── gallery.rs          # 画廊搜索、回收站、收藏夹
├── runtime_logs.rs     # 实时日志环形缓冲区
├── commands/           # Tauri 命令处理器
│   ├── mod.rs
│   ├── generation.rs   # 图像生成/编辑命令
│   ├── conversations.rs# 对话管理
│   ├── projects.rs     # 项目管理
│   ├── prompts.rs      # 提示词收藏
│   ├── settings.rs     # 设置管理
│   ├── logs.rs         # 日志管理
│   └── llm.rs          # LLM 提示词优化
├── image_engines/      # 多提供商路由
│   ├── mod.rs          # ImageProvider 枚举与路由
│   ├── openai.rs       # OpenAI 请求构建
│   └── gemini.rs       # Gemini 请求构建
└── llm/                # LLM 集成
    ├── mod.rs
    ├── anthropic.rs    # Anthropic Claude
    └── openai.rs       # OpenAI GPT
```

## 关键设计决策

1. **窗口透明** - `transparent: true` 配合平台特定的 vibrancy 代码实现原生毛玻璃效果
2. **单写者数据库** - `Mutex<Connection>` 模式，所有 DB 操作加锁，天然单线程写入
3. **事件驱动生成** - 后端发射 `generation:progress/complete/failed` 事件，前端订阅响应
4. **可扩展引擎** - `ImageEngine` trait 是添加新 AI 引擎的扩展点
5. **多提供商配置** - 每个引擎支持多个命名的提供商配置文件（不同的 API key/endpoint）
6. **生成恢复** - 中断的生成任务在下次启动时通过 `generation_recoveries` 表恢复
7. **版本化迁移** - 数据库 schema 通过 `schema_migrations` 表追踪版本

## 数据流概览

```
用户输入 Prompt
    ↓
前端 invoke("generate_image")
    ↓
后端 commands::generation::generate_image
    ↓
model_registry 路由到对应引擎
    ↓
api_gateway::ImageEngine::generate
    ↓
image_engines/openai 或 gemini 发送 HTTP 请求
    ↓
解析响应，保存图片到文件系统
    ↓
更新数据库 (generations + images 表)
    ↓
发射 generation:complete 事件
    ↓
前端更新 UI
```

## 相关页面

- [[Frontend Architecture]] - 前端架构详解
- [[Backend Architecture]] - 后端架构详解
- [[IPC Communication]] - IPC 通信机制
- [[Database Schema]] - 数据库设计
