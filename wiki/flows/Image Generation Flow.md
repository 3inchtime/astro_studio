---
type: concept
title: "Image Generation Flow"
created: 2026-05-09
updated: 2026-05-09
tags: [flows, generation, core]
status: active
---

# Image Generation Flow

## 完整流程

```
用户在 GeneratePage 输入 Prompt
    ↓
前端调用 generateImage(params)
    ↓
invoke("generate_image") → Rust
    ↓
commands::generation::generate_image
    ├─ 创建 Conversation (如果需要)
    ├─ 插入 Generation 记录 (status=processing)
    ├─ 插入 Recovery 记录 (request_state=requested)
    ↓
异步任务启动
    ├─ 获取 API key 和端点配置
    ├─ model_registry::sanitize_request_options_for_model()
    ├─ api_gateway::ImageEngine::generate()
    │   ├─ provider_for_model() 路由
    │   ├─ OpenAI 路径:
    │   │   ├─ openai::build_generation_request_body()
    │   │   ├─ POST /v1/images/generations
    │   │   └─ 解析 b64_json 或 url 响应
    │   └─ Gemini 路径:
    │       ├─ gemini::build_request_body()
    │       ├─ POST /v1beta/models/{model}:generateContent
    │       └─ 解析 Gemini 响应格式
    ├─ 自动重试 (max_retries 次)
    ├─ 批量请求 (当 image_count > API 单次返回数时)
    ↓
图片数据返回
    ↓
FileManager::save_image_at()
    ├─ 按日期创建目录 images/YYYY/MM/DD/
    ├─ 保存原图 (指定输出格式)
    ├─ 生成 256px 缩略图
    ↓
数据库更新
    ├─ 插入 Image 记录
    ├─ 更新 Generation status=completed
    ├─ 删除 Recovery 记录
    ↓
发射 generation:complete 事件
    ↓
前端收到事件
    ├─ React Query 失效相关查询
    ├─ 更新 Conversation 列表
    └─ 显示生成结果
```

## 错误处理

```
API 请求失败
    ├─ 网络错误 → 重试 (最多 max_retries 次)
    ├─ 5xx 错误 → 重试
    ├─ 4xx 错误 → 直接失败
    ↓
最终失败
    ├─ 更新 Generation status=failed, error_message=...
    ├─ 删除 Recovery 记录
    ├─ 发射 generation:failed 事件
    └─ 前端显示错误信息
```

## 批量图片请求

当 `image_count > 1` 时，引擎会循环请求直到满足数量：

```
请求 4 张图片
    ↓
第 1 次请求: image_count=4
    ↓
API 返回 2 张
    ↓
第 2 次请求: image_count=2 (剩余)
    ↓
API 返回 2 张
    ↓
总计 4 张，完成
```

如果 API 持续返回不足数量，会在日志中记录警告。

## 关键代码路径

- 入口：`src-tauri/src/commands/generation.rs` → `generate_image`
- 引擎调用：`src-tauri/src/api_gateway.rs` → `ImageEngine::generate`
- OpenAI 请求：`src-tauri/src/image_engines/openai.rs`
- Gemini 请求：`src-tauri/src/image_engines/gemini.rs`
- 文件保存：`src-tauri/src/file_manager.rs` → `save_image_at`
- 前端 API：`src/lib/api.ts` → `generateImage`
- 前端页面：`src/pages/GeneratePage.tsx`

## 相关页面

- [[Image Edit Flow]] - 图像编辑流程
- [[Generation Recovery Flow]] - 生成恢复流程
- [[Provider Routing Flow]] - 提供商路由
- [[Image Engines]] - 图像引擎详解
