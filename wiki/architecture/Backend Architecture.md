---
type: concept
title: "Backend Architecture"
created: 2026-05-09
updated: 2026-05-09
tags: [architecture, backend, rust]
status: active
---

# Backend Architecture

## 技术栈

- **Rust 2021 edition** - 系统语言
- **Tauri 2.0** - 桌面框架
- **rusqlite 0.31** - SQLite 数据库
- **reqwest 0.12** - HTTP 客户端
- **tokio 1.x** - 异步运行时
- **serde / serde_json** - 序列化
- **image 0.25** - 图片处理
- **chrono 0.4** - 时间处理
- **uuid 1.x** - UUID 生成
- **thiserror 2** - 错误处理
- **window-vibrancy 0.5** - 窗口毛玻璃效果

## 模块详解

### lib.rs - 应用入口
- 加载配置 (`AppConfig::load()`)
- 初始化日志 (`config::init_logger()`)
- 打开数据库 (`Database::open()`)
- 运行迁移 (`database.run_migrations()`)
- 创建引擎 (`GptImageEngine::new()`)
- 注册所有 Tauri 命令
- Setup 阶段：修复图片扩展名、恢复中断生成、清理日志、清理回收站、设置窗口 vibrancy

### config.rs - 配置管理
- TOML 格式配置文件 `astro_studio.toml`
- 存储路径：`~/.config/astro-studio/`
- 三个配置段：`log`、`api`、`storage`
- 首次加载自动生成默认配置

### db.rs - 数据库层
- `Database` 结构体封装 `Mutex<Connection>`
- WAL 模式 + 外键约束
- 12 个版本的 schema 迁移
- 基础操作：`get_setting`、`set_setting`、`insert_log`、`search_logs`

### error.rs - 错误处理
- `AppError` 枚举使用 `thiserror` 派生
- 错误类型：`ApiKeyNotSet`、`ProviderProfileNotFound`、`Api`、`Network`、`Database`、`FileSystem`、`Validation`
- 实现了从 `rusqlite::Error`、`std::io::Error`、`serde_json::Error`、`reqwest::Error`、`String` 的自动转换

### models.rs - 数据模型
- 引擎常量：`ENGINE_GPT_IMAGE_2`、`ENGINE_NANO_BANANA` 等
- 设置键常量：`SETTING_IMAGE_MODEL`、`SETTING_API_KEY` 等
- 默认值常量：`DEFAULT_IMAGE_SIZE`、`DEFAULT_IMAGE_QUALITY` 等
- 数据结构：`Generation`、`GeneratedImage`、`Conversation`、`Project`、`Folder`、`LlmConfig` 等

### api_gateway.rs - API 网关
- `ImageEngine` trait：`generate()` 和 `edit()` 方法
- `GptImageEngine` 实现：
  - 双 HTTP 客户端（带超时 / 无超时）
  - 自动重试机制
  - 批量图片请求（当请求多张时循环请求直到满足数量）
  - 响应体日志记录
  - 图片解码（支持 OpenAI b64_json/url 和 Gemini 格式）

### model_registry.rs - 模型注册表
- `normalize_image_model()` - 别名到标准 ID 的映射
- `is_gemini_model()` - 判断是否为 Gemini 模型
- `sanitize_request_options_for_model()` - Gemini 模型不支持的参数重置为默认值
- `default_endpoint_settings_for_model()` - 模型默认端点配置

### file_manager.rs - 文件管理
- 图片按日期组织：`images/YYYY/MM/DD/`
- 256px 缩略图生成
- 输出格式支持：png、jpeg、webp
- 原子写入（先写临时文件再 rename）

### gallery.rs - 画廊系统
- 搜索/分页/过滤
- 软删除（deleted_at 时间戳）
- 回收站清理（按保留天数）
- 文件夹管理（CRUD + 图片关联）
- 收藏夹查询

### runtime_logs.rs - 实时日志
- 内存环形缓冲区
- 自定义 `log::Log` 实现
- 通过 Tauri 事件实时推送到前端

## 命令处理器 (commands/)

### generation.rs
- `generate_image` - 创建生成记录，异步调用引擎，保存图片，发射事件
- `edit_image` - 编辑已有图片
- `copy_image_to_clipboard` - 复制到剪贴板
- `save_image_to_file` - 另存为
- `pick_source_images` - 文件选择器

### conversations.rs
- CRUD 操作：创建、获取、重命名、删除
- 归档/取消归档
- 置顶/取消置顶
- 移动到其他项目

### projects.rs
- CRUD 操作：创建、获取、重命名、删除
- 归档/取消归档
- 置顶/取消置顶

### prompts.rs
- 提示词收藏 CRUD
- 提示词文件夹管理
- 收藏与文件夹关联

### settings.rs
- API key 管理（全局 + 按模型）
- 端点配置（base_url / full_url 模式）
- 提供商配置文件管理
- 字体大小、图片模型选择

### logs.rs
- 日志查询/详情/清理
- 运行时日志获取
- 日志/回收站设置

### llm.rs
- LLM 配置管理
- 提示词优化（支持文本和多模态）

## 相关页面

- [[System Architecture]] - 系统架构总览
- [[Frontend Architecture]] - 前端架构详解
- [[Image Engines]] - 图像引擎详解
- [[Database Schema]] - 数据库设计
