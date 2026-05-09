---
type: concept
title: "Image Edit Flow"
created: 2026-05-09
updated: 2026-05-09
tags: [flows, edit, core]
status: active
---

# Image Edit Flow

## 完整流程

```
用户选择源图片 + 输入 Prompt
    ↓
前端调用 editImage(params)
    ↓
invoke("edit_image") → Rust
    ↓
commands::generation::edit_image
    ├─ 创建 Conversation (如果需要)
    ├─ 插入 Generation 记录 (request_kind=edit)
    ├─ 插入 Recovery 记录
    ↓
异步任务启动
    ├─ 获取 API key 和端点配置
    ├─ api_gateway::ImageEngine::edit()
    │   ├─ provider_for_model() 路由
    │   ├─ prepare_edit_images() 读取源图片
    │   ├─ OpenAI 路径:
    │   │   ├─ build_edit_form() 构建 multipart/form-data
    │   │   ├─ POST /v1/images/edits
    │   │   └─ 图片作为 file part 上传
    │   └─ Gemini 路径:
    │       ├─ 将源图片转为 base64 inline
    │       ├─ POST /v1beta/models/{model}:generateContent
    │       └─ 图片作为 inlineData 传递
    ↓
后续流程与 [[Image Generation Flow]] 相同
```

## OpenAI Edit 特殊处理

OpenAI 的 edit 接口使用 `multipart/form-data`：

```
Form:
  - model: "gpt-image-2"
  - prompt: "用户输入的提示词"
  - size: "1024x1024"
  - quality: "auto"
  - image[]: <binary data>  (多个源图片)
```

- `edit_client` 使用无超时的 HTTP 客户端（edit 操作可能耗时较长）
- 源图片通过 `Part::bytes()` 添加到 form

## Gemini Edit 特殊处理

Gemini 没有独立的 edit 接口，使用同一个 `generateContent` 端点：

```json
{
  "contents": [{
    "parts": [
      {"text": "prompt"},
      {"inlineData": {"mimeType": "image/png", "data": "base64..."}},
      {"inlineData": {"mimeType": "image/jpeg", "data": "base64..."}}
    ]
  }],
  "generationConfig": { ... }
}
```

## 关键代码路径

- 入口：`src-tauri/src/commands/generation.rs` → `edit_image`
- 引擎调用：`src-tauri/src/api_gateway.rs` → `ImageEngine::edit`
- OpenAI form 构建：`src-tauri/src/api_gateway.rs` → `build_edit_form`
- Gemini 请求：`src-tauri/src/image_engines/gemini.rs`
- 前端 API：`src/lib/api.ts` → `editImage`

## 相关页面

- [[Image Generation Flow]] - 图像生成流程
- [[Provider Routing Flow]] - 提供商路由
- [[Image Engines]] - 图像引擎详解
