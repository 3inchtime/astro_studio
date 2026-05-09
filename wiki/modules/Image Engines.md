---
type: concept
title: "Image Engines"
created: 2026-05-09
updated: 2026-05-09
tags: [modules, engines, providers]
status: active
---

# Image Engines

## 架构

```
ImageEngine (trait)
    ├── generate()
    └── edit()
         │
         ▼
GptImageEngine (实现)
    ├── provider_for_model() 路由
    ├── OpenAI 路径
    │   ├── build_generation_request_body()
    │   └── build_edit_form()
    └── Gemini 路径
        ├── build_request_body()
        └── request_gemini_images()
```

## ImageEngine Trait

```rust
#[async_trait]
pub trait ImageEngine: Send + Sync {
    async fn generate(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
        db: Option<&Database>,
        log_dir: Option<&Path>,
    ) -> Result<EngineImagesResult, String>;

    async fn edit(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_image_paths: &[String],
        options: &GptImageRequestOptions,
        db: Option<&Database>,
        log_dir: Option<&Path>,
    ) -> Result<EngineImagesResult, String>;
}
```

## GptImageEngine

### 初始化

```rust
pub fn new(config: &ApiConfig) -> Self {
    let client = Self::build_client(timeout_secs);      // 带超时
    let edit_client = Self::build_client(None);          // 无超时 (edit 可能耗时长)
    Self { client, edit_client, max_retries, timeout_secs }
}
```

### HTTP 客户端

- `client` - 用于 generate 请求，可配置超时
- `edit_client` - 用于 edit 请求，无超时限制
- 超时值 `0` 或 `120` 都被视为无限制（兼容旧版本）

### 重试机制

```
for attempt in 0..=max_retries {
    发送请求
    ├─ 成功 → break
    ├─ 网络错误 → 重试
    ├─ 5xx 错误 → 重试
    ├─ 4xx 错误 → 直接失败
    └─ 超时 → 重试
}
```

### 响应解码

`decode_images_from_response()` 支持两种格式：

1. **OpenAI 格式** - `{"data": [{"b64_json": "...", "url": "..."}]}`
2. **Gemini 格式** - `{"candidates": [{"content": {"parts": [{"inlineData": {...}}]}}]}`

先尝试 OpenAI 格式，失败后尝试 Gemini 格式。

## OpenAI 引擎 (openai.rs)

### 请求构建

```json
{
  "model": "gpt-image-2",
  "prompt": "a cat",
  "size": "1024x1024",
  "quality": "auto",
  "background": "auto",
  "output_format": "png",
  "output_compression": 100,
  "moderation": "auto",
  "n": 1
}
```

### Edit 请求

使用 `multipart/form-data`：
- 文本字段：model, prompt, size, quality, background 等
- 文件字段：`image[]` (多个源图片)

## Gemini 引擎 (gemini.rs)

### 请求构建

```json
{
  "contents": [{
    "parts": [
      {"text": "generate a cat"},
      {"inlineData": {"mimeType": "image/png", "data": "base64..."}}
    ]
  }],
  "generationConfig": {
    "responseModalities": ["TEXT", "IMAGE"]
  }
}
```

### 错误增强

`augment_transport_error()` 为 Gemini 模型的错误添加上下文提示。

## 关键代码路径

- Trait 定义：`src-tauri/src/api_gateway.rs`
- OpenAI 实现：`src-tauri/src/image_engines/openai.rs`
- Gemini 实现：`src-tauri/src/image_engines/gemini.rs`
- 提供商路由：`src-tauri/src/image_engines/mod.rs`
- 模型注册表：`src-tauri/src/model_registry.rs`

## 相关页面

- [[Provider Routing Flow]] - 提供商路由流程
- [[Image Generation Flow]] - 图像生成流程
- [[Image Edit Flow]] - 图像编辑流程
