---
type: concept
title: "Settings System"
created: 2026-05-09
updated: 2026-05-09
tags: [modules, settings, configuration]
status: active
---

# Settings System

## 概述

Astro Studio 有两层配置系统：
1. **应用配置** (`astro_studio.toml`) - 底层运行时配置
2. **用户设置** (数据库 `settings` 表) - 用户可修改的设置

## 应用配置 (TOML)

文件位置：`~/.config/astro-studio/astro_studio.toml`

```toml
[log]
level = "info"
save_to_file = false
file_path = "astro_studio.log"

[api]
timeout_secs = 0      # 0 = 无限制
max_retries = 0       # 0 = 不重试

[storage]
thumbnail_size = 256
```

## 用户设置 (数据库)

存储在 `settings` 表中，key-value 格式。

### 设置项

| Key | 说明 | 默认值 |
|-----|------|--------|
| `image_model` | 当前选择的图片模型 | `gpt-image-2` |
| `api_key` | 全局 API key | - |
| `base_url` | API 基础 URL | `https://api.openai.com/v1` |
| `endpoint_mode` | 端点模式 | `base_url` |
| `generation_url` | 生成端点完整 URL | - |
| `edit_url` | 编辑端点完整 URL | - |
| `font_size` | 字体大小 | `medium` |
| `log_enabled` | 日志开关 | `true` |
| `log_retention_days` | 日志保留天数 | `7` |
| `trash_retention_days` | 回收站保留天数 | `30` |
| `llm_configs` | LLM 配置 JSON | - |

### 按模型设置

每个模型有独立的设置，使用 `model_config::{model}::{suffix}` 格式的 key：

| Key 模式 | 说明 |
|----------|------|
| `model_config::{model}::api_key` | 模型 API key |
| `model_config::{model}::endpoint_mode` | 端点模式 |
| `model_config::{model}::base_url` | 基础 URL |
| `model_config::{model}::generation_url` | 生成 URL |
| `model_config::{model}::edit_url` | 编辑 URL |

### 提供商配置文件

每个模型支持多个提供商配置文件：

| Key 模式 | 说明 |
|----------|------|
| `model_provider_profiles::{model}` | 提供商配置列表 JSON |
| `model_active_provider::{model}` | 当前活跃提供商 ID |

## 设置命令

```rust
// 全局设置
save_api_key(key) / get_api_key()
save_base_url(url) / get_base_url()
save_font_size(fontSize) / get_font_size()
save_image_model(model) / get_image_model()

// 端点设置
save_endpoint_settings(settings) / get_endpoint_settings()

// 按模型设置
save_model_api_key(model, key) / get_model_api_key(model)
save_model_endpoint_settings(model, settings) / get_model_endpoint_settings(model)

// 提供商配置文件
save_model_provider_profiles(model, state) / get_model_provider_profiles(model)
create_model_provider_profile(model, name)
delete_model_provider_profile(model, providerId)
set_active_model_provider(model, providerId)

// 日志/回收站设置
save_log_settings(enabled, retentionDays) / get_log_settings()
save_trash_settings(retentionDays) / getTrashSettings()

// LLM 设置
save_llm_configs(configs) / get_llm_configs()
```

## 端点模式

### base_url 模式 (默认)

```
base_url: https://api.openai.com/v1
→ 自动生成: https://api.openai.com/v1/images/generations
→ 自动生成: https://api.openai.com/v1/images/edits
```

### full_url 模式

```
generation_url: https://custom-gateway.com/create
edit_url: https://custom-gateway.com/edit
→ 直接使用完整 URL
```

## 关键代码路径

- 后端命令：`src-tauri/src/commands/settings.rs`
- 配置加载：`src-tauri/src/config.rs`
- 模型注册表：`src-tauri/src/model_registry.rs`
- 前端 API：`src/lib/api.ts`
- 前端页面：`src/pages/SettingsPage.tsx`

## 相关页面

- [[Provider Routing Flow]] - 提供商路由
- [[Image Engines]] - 图像引擎详解
- [[LLM Integration]] - LLM 集成
