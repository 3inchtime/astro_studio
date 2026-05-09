---
type: overview
title: "Astro Studio"
created: 2026-05-09
updated: 2026-05-09
tags: [overview, project]
status: active
---

# Astro Studio

Astro Studio 是一款跨平台 AI 图像生成桌面客户端，使用 Tauri 2.0 构建，前端为 React，后端为 Rust。聚合多个 AI 图像生成提供商，通过原生桌面界面提供流畅的用户体验。

## 技术栈

| 层级 | 技术 | 版本 |
|------|------|------|
| 桌面框架 | Tauri | 2.0 |
| 前端框架 | React | 19.1 |
| 前端语言 | TypeScript | 5.8 |
| 后端语言 | Rust | 2021 edition |
| 样式 | Tailwind CSS | 4.2 |
| 状态管理 | React Query + Zustand | 5.x |
| 数据库 | SQLite (rusqlite) | 0.31 |
| 路由 | react-router-dom | 7.x |
| 国际化 | i18next | 26.x |
| 动画 | Framer Motion | 12.x |

## 支持平台

- Windows (Mica 窗口效果)
- macOS (HudWindow 毛玻璃效果)

## AI 引擎

| 引擎 | 模型 | 提供商 |
|------|------|--------|
| GPT Image | gpt-image-2 | OpenAI |
| Nano Banana | nano-banana | Google Gemini |
| Nano Banana 2 | nano-banana-2 | Google Gemini |
| Nano Banana Pro | nano-banana-pro | Google Gemini |

## 核心功能

- AI 图像生成与编辑
- 多引擎/多提供商路由
- 项目与对话管理
- 图片画廊与收藏夹
- LLM 提示词优化
- 多语言支持 (8 种语言)
- 生成恢复 (断点续传)
- 软删除与回收站

## 相关页面

- [[System Architecture]] - 系统架构总览
- [[Version History]] - 版本记录
- [[Image Generation Flow]] - 图像生成流程
- [[Database Schema]] - 数据库设计
