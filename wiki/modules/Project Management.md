---
type: concept
title: "Project Management"
created: 2026-05-09
updated: 2026-05-09
tags: [modules, projects, conversations]
status: active
---

# Project Management

## 概述

Astro Studio 的项目管理系统提供两级组织结构：Project → Conversation → Generation。

## 层级关系

```
Project (项目)
    │
    ├── Conversation 1 (对话)
    │       ├── Generation A
    │       └── Generation B
    │
    ├── Conversation 2
    │       └── Generation C
    │
    └── ...
```

- 每个 Project 包含多个 Conversation
- 每个 Conversation 包含多次 Generation
- Generation 自动关联到当前 Conversation

## 项目操作

```rust
create_project(name)           // 创建项目
get_projects(include_archived) // 获取项目列表
rename_project(id, name)       // 重命名
archive_project(id)            // 归档
unarchive_project(id)          // 取消归档
pin_project(id)                // 置顶
unpin_project(id)              // 取消置顶
delete_project(id)             // 软删除
```

### 默认项目

系统自动创建 "Default Project" (id="default")，新对话默认归属此项目。

## 对话操作

```rust
create_conversation(title, projectId)  // 创建对话
get_conversations(query, projectId, includeArchived)  // 获取对话列表
rename_conversation(id, title)         // 重命名
move_conversation_to_project(id, projectId)  // 移动到其他项目
archive_conversation(id)               // 归档
unarchive_conversation(id)             // 取消归档
pin_conversation(id)                   // 置顶
unpin_conversation(id)                 // 取消置顶
delete_conversation(id)                // 软删除
get_conversation_generations(id)       // 获取对话的所有生成记录
```

### 自动创建

在 GeneratePage 中，如果没有当前对话，会自动创建一个新对话。

## 前端路由

```
/projects                          → ProjectsPage (项目列表)
/projects/:projectId               → ProjectHomePage (项目详情)
/projects/:projectId/chat/:convId  → ProjectChatPage (对话页面)
```

## 数据查询

### 项目统计

每个 Project 包含：
- `conversation_count` - 对话数量
- `image_count` - 图片总数

### 对话统计

每个 Conversation 包含：
- `generation_count` - 生成次数
- `latest_generation_at` - 最近生成时间
- `latest_thumbnail` - 最近生成的缩略图

## 关键代码路径

- 后端命令：`src-tauri/src/commands/projects.rs`, `src-tauri/src/commands/conversations.rs`
- 前端 API：`src/lib/api.ts` → `createProject`, `getProjects` 等
- 前端查询：`src/lib/queries/projects.ts`
- 前端页面：`src/pages/ProjectsPage.tsx`, `src/pages/ProjectHomePage.tsx`, `src/pages/ProjectChatPage.tsx`

## 相关页面

- [[Database Schema]] - 数据库设计
- [[Image Generation Flow]] - 图像生成流程
