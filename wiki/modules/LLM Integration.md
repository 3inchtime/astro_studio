---
type: concept
title: "LLM Integration"
created: 2026-05-09
updated: 2026-05-09
tags: [modules, llm, ai]
status: active
---

# LLM Integration

## 概述

Astro Studio 集成了 LLM（大语言模型）用于提示词优化功能，帮助用户将简单的描述转化为更适合 AI 图像生成的详细提示词。

## LLM 配置

### 数据结构

```rust
pub struct LlmConfig {
    pub id: String,
    pub name: String,
    pub protocol: String,      // "openai" | "anthropic"
    pub model: String,         // 模型名称
    pub api_key: String,
    pub base_url: String,
    pub capability: String,    // "text" | "multimodal"
    pub enabled: bool,
}
```

### 支持的协议

| 协议 | 提供商 | 模型示例 |
|------|--------|----------|
| `openai` | OpenAI / 兼容 API | gpt-4o, gpt-4o-mini |
| `anthropic` | Anthropic Claude | claude-3-opus, claude-3-sonnet |

### 能力类型

| 能力 | 说明 |
|------|------|
| `text` | 纯文本提示词优化 |
| `multimodal` | 支持图片输入的提示词优化 |

## 提示词优化流程

```
用户输入简单描述
    ↓
前端调用 optimizePrompt(prompt, configId, imagePaths?)
    ↓
invoke("optimize_prompt")
    ↓
commands::llm::optimize_prompt
    ├─ 查找 LLM 配置
    ├─ capability == "multimodal" && imagePaths 非空?
    │   ├─ 加载图片 (load_images)
    │   ├─ 创建多模态客户端
    │   └─ chat_with_images()
    └─ 否则
        ├─ 创建文本客户端
        └─ chat()
    ↓
返回优化后的提示词
    ↓
前端展示优化结果，用户可采用或修改
```

## 多模态支持

当 LLM 配置为 `multimodal` 且提供了图片路径时：

1. `load_images()` 读取图片文件
2. `create_multimodal_llm_client()` 创建支持图片的客户端
3. 将图片与文本一起发送给 LLM
4. LLM 基于图片内容优化提示词

### 自动切换

前端会根据是否选择了源图片自动切换文本/多模态 LLM。

## LLM 客户端

### Anthropic (anthropic.rs)

```rust
async fn chat(&self, prompt: &str) -> Result<String, String>
async fn chat_with_images(&self, prompt: &str, images: &[ImageData]) -> Result<String, String>
async fn with_timeout<T>(future: impl Future<Output = T>, secs: u64) -> T
```

### OpenAI (openai.rs)

```rust
async fn chat(&self, prompt: &str) -> Result<String, String>
async fn chat_with_images(&self, prompt: &str, images: &[ImageData]) -> Result<String, String>
async fn with_timeout<T>(future: impl Future<Output = T>, secs: u64) -> T
```

## 配置管理

- 存储在 `settings` 表，key 为 `llm_configs`
- 值为 JSON 数组，包含所有 LLM 配置
- 前端通过 `getLlmConfigs()` / `saveLlmConfigs()` 管理
- 每个配置可独立启用/禁用

## 关键代码路径

- 命令：`src-tauri/src/commands/llm.rs`
- Anthropic 客户端：`src-tauri/src/llm/anthropic.rs`
- OpenAI 客户端：`src-tauri/src/llm/openai.rs`
- 前端 API：`src/lib/api.ts` → `optimizePrompt`
- 前端查询：`src/lib/queries/llm.ts`

## 相关页面

- [[Settings System]] - 设置系统
- [[Image Generation Flow]] - 图像生成流程
