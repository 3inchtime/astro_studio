---
type: concept
title: "Provider Routing Flow"
created: 2026-05-09
updated: 2026-05-09
tags: [flows, routing, multi-provider]
status: active
---

# Provider Routing Flow

## 概述

Astro Studio 支持多个 AI 图像生成提供商，通过模型名称自动路由到对应的提供商。

## 路由逻辑

```
用户选择模型 (如 "nano-banana-pro")
    ↓
model_registry::normalize_image_model()
    ├─ 别名解析: "gemini-3-pro-image-preview" → "nano-banana-pro"
    ├─ 标准化: "nano-banana-pro" → "nano-banana-pro"
    ↓
image_engines::provider_for_model()
    ├─ is_gemini_model()? → ImageProvider::Gemini
    └─ 否则 → ImageProvider::OpenAi
    ↓
api_gateway::ImageEngine::generate() / edit()
    ├─ ImageProvider::Gemini → request_gemini_images()
    └─ ImageProvider::OpenAi → OpenAI 标准流程
```

## 模型别名映射

| 标准 ID | 别名 | 提供商 |
|---------|------|--------|
| `gpt-image-2` | - | OpenAI |
| `nano-banana` | `gemini-2.5-flash-image` | Gemini |
| `nano-banana-2` | `gemini-3.1-flash-image-preview` | Gemini |
| `nano-banana-pro` | `gemini-3-pro-image-preview` | Gemini |

## 端点配置

### OpenAI 默认

```
base_url: https://api.openai.com/v1
generation_url: https://api.openai.com/v1/images/generations
edit_url: https://api.openai.com/v1/images/edits
```

### Gemini 默认

```
base_url: https://generativelanguage.googleapis.com/v1beta/models
generation_url: https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent
edit_url: (同 generation_url)
```

### 端点模式

| 模式 | 说明 |
|------|------|
| `base_url` | 自动拼接路径 (默认) |
| `full_url` | 使用完整的 generation_url/edit_url |

## 提供商配置文件

每个模型支持多个提供商配置文件：

```rust
pub struct ModelProviderProfile {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub endpoint_settings: EndpointSettings,
}

pub struct ModelProviderProfilesState {
    pub active_provider_id: String,
    pub profiles: Vec<ModelProviderProfile>,
}
```

- 每个模型有独立的提供商配置
- 默认配置 ID 为 "default"
- 可创建多个配置，切换不同的 API key/endpoint

## Gemini 特殊处理

### 参数清洗

Gemini 不支持的参数会被重置为默认值：

```rust
sanitize_request_options_for_model() →
  quality: "auto"
  background: "auto"
  output_format: "png"
  moderation: "auto"
  input_fidelity: "high"
```

### 端点规范化

Gemini URL 自动补全 `:generateContent` 后缀：

```
输入: https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image
输出: https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent
```

### 请求头

- OpenAI: `Authorization: Bearer {api_key}`
- Gemini: `x-goog-api-key: {api_key}`

## 关键代码路径

- 模型归一化：`src-tauri/src/model_registry.rs` → `normalize_image_model`
- 提供商路由：`src-tauri/src/image_engines/mod.rs` → `provider_for_model`
- 端点构建：`src-tauri/src/model_registry.rs` → `image_endpoint_url_for_model_settings`
- 配置管理：`src-tauri/src/commands/settings.rs`

## 相关页面

- [[Image Engines]] - 图像引擎详解
- [[Image Generation Flow]] - 图像生成流程
- [[Settings System]] - 设置系统
