---
type: concept
title: "Database Schema"
created: 2026-05-09
updated: 2026-05-09
tags: [architecture, database, sqlite]
status: active
---

# Database Schema

## 概述

Astro Studio 使用 SQLite (通过 rusqlite) 作为本地数据库，采用 WAL 模式和外键约束。数据库文件位于 `~/.local/share/astro-studio/astro_studio.db`。

Schema 通过版本化迁移管理，当前共 12 个版本。

## 表结构

### generations - 生成记录

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 生成 ID |
| prompt | TEXT NOT NULL | 提示词 |
| engine | TEXT NOT NULL | 引擎名称 |
| request_kind | TEXT NOT NULL | 请求类型 (generate/edit) |
| size | TEXT NOT NULL | 图片尺寸 |
| quality | TEXT NOT NULL | 质量 |
| background | TEXT NOT NULL | 背景 |
| output_format | TEXT NOT NULL | 输出格式 |
| output_compression | INTEGER NOT NULL | 压缩率 |
| moderation | TEXT NOT NULL | 审核级别 |
| input_fidelity | TEXT NOT NULL | 输入保真度 |
| image_count | INTEGER NOT NULL | 请求图片数 |
| source_image_count | INTEGER NOT NULL | 源图片数 |
| source_image_paths | TEXT NOT NULL | 源图片路径 JSON |
| request_metadata | TEXT | 请求元数据 |
| status | TEXT NOT NULL | 状态 (pending/processing/completed/failed) |
| error_message | TEXT | 错误信息 |
| conversation_id | TEXT FK | 关联对话 |
| created_at | TEXT NOT NULL | 创建时间 |
| deleted_at | TEXT | 软删除时间 |

### images - 生成图片

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 图片 ID |
| generation_id | TEXT FK NOT NULL | 关联生成记录 |
| file_path | TEXT NOT NULL | 文件路径 |
| thumbnail_path | TEXT | 缩略图路径 |
| width | INTEGER NOT NULL | 宽度 |
| height | INTEGER NOT NULL | 高度 |
| file_size | INTEGER NOT NULL | 文件大小 |
| created_at | TEXT NOT NULL | 创建时间 |

### conversations - 对话

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 对话 ID |
| title | TEXT NOT NULL | 标题 |
| project_id | TEXT FK | 所属项目 |
| created_at | TEXT NOT NULL | 创建时间 |
| updated_at | TEXT NOT NULL | 更新时间 |
| archived_at | TEXT | 归档时间 |
| pinned_at | TEXT | 置顶时间 |
| deleted_at | TEXT | 软删除时间 |

### projects - 项目

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 项目 ID |
| name | TEXT NOT NULL | 项目名称 |
| created_at | TEXT NOT NULL | 创建时间 |
| updated_at | TEXT NOT NULL | 更新时间 |
| archived_at | TEXT | 归档时间 |
| pinned_at | TEXT | 置顶时间 |
| deleted_at | TEXT | 软删除时间 |

### settings - 设置

| 列名 | 类型 | 说明 |
|------|------|------|
| key | TEXT PK | 设置键 |
| value | TEXT NOT NULL | 设置值 |

### folders - 图片文件夹

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 文件夹 ID |
| name | TEXT NOT NULL | 文件夹名称 |
| created_at | TEXT NOT NULL | 创建时间 |

### folder_images - 文件夹图片关联

| 列名 | 类型 | 说明 |
|------|------|------|
| folder_id | TEXT FK PK | 文件夹 ID |
| image_id | TEXT FK PK | 图片 ID |
| added_at | TEXT NOT NULL | 添加时间 |

### prompt_favorites - 提示词收藏

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 收藏 ID |
| prompt | TEXT NOT NULL UNIQUE | 提示词内容 |
| created_at | TEXT NOT NULL | 创建时间 |
| updated_at | TEXT NOT NULL | 更新时间 |

### prompt_folders - 提示词文件夹

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 文件夹 ID |
| name | TEXT NOT NULL | 文件夹名称 |
| created_at | TEXT NOT NULL | 创建时间 |

### prompt_folder_favorites - 提示词文件夹关联

| 列名 | 类型 | 说明 |
|------|------|------|
| folder_id | TEXT FK PK | 文件夹 ID |
| prompt_favorite_id | TEXT FK PK | 收藏 ID |
| added_at | TEXT NOT NULL | 添加时间 |

### logs - 日志

| 列名 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 日志 ID |
| timestamp | TEXT NOT NULL | 时间戳 |
| log_type | TEXT NOT NULL | 类型 (api_request/api_response/generation/system) |
| level | TEXT NOT NULL | 级别 (debug/info/warn/error) |
| message | TEXT NOT NULL | 消息 |
| generation_id | TEXT | 关联生成 ID |
| metadata | TEXT | 元数据 JSON |
| response_file | TEXT | 响应文件路径 |

### generation_recoveries - 生成恢复

| 列名 | 类型 | 说明 |
|------|------|------|
| generation_id | TEXT PK FK | 生成 ID |
| request_kind | TEXT NOT NULL | 请求类型 |
| request_state | TEXT NOT NULL | 恢复状态 |
| output_format | TEXT NOT NULL | 输出格式 |
| response_file | TEXT | 响应文件路径 |
| created_at | TEXT NOT NULL | 创建时间 |
| updated_at | TEXT NOT NULL | 更新时间 |

### schema_migrations - 迁移追踪

| 列名 | 类型 | 说明 |
|------|------|------|
| version | INTEGER PK | 迁移版本号 |
| applied_at | TEXT NOT NULL | 应用时间 |

## 索引

关键索引包括：
- `idx_images_generation_id` - 图片按生成 ID 查询
- `idx_generations_created_at` - 生成按时间排序
- `idx_generations_conversation_id` - 生成按对话分组
- `idx_generations_deleted_at` - 软删除过滤
- `idx_conversations_updated_at` - 对话按更新时间排序
- `idx_conversations_project_id` - 对话按项目分组
- `idx_logs_timestamp` - 日志按时间排序
- `idx_prompt_favorites_prompt` - 提示词唯一性约束

## 迁移历史

详见 [[Database Migrations]]

## 相关页面

- [[System Architecture]] - 系统架构总览
- [[Backend Architecture]] - 后端架构详解
- [[Database Migrations]] - 迁移历史
