---
type: concept
title: "Gallery System"
created: 2026-05-09
updated: 2026-05-09
tags: [modules, gallery, favorites]
status: active
---

# Gallery System

## 概述

Gallery 系统提供图片浏览、搜索、收藏、文件夹管理和回收站功能。

## 功能模块

### 搜索与浏览

```rust
search_generations(query, page, only_deleted, filters, project_id)
```

- 关键词搜索（模糊匹配 prompt）
- 按模型过滤
- 按日期范围过滤
- 按项目过滤
- 分页（每页 20 条）

### 软删除与回收站

```
删除图片
    ↓
UPDATE generations SET deleted_at = now WHERE id = ?
    ↓
图片仍在数据库和文件系统中
    ↓
回收站页面显示 deleted_at IS NOT NULL 的记录
    ↓
用户可恢复或永久删除
```

#### 恢复

```rust
restore_generation(id)
    ├─ UPDATE generations SET deleted_at = NULL
    └─ 如果关联对话也被删除/归档，一并恢复
```

#### 永久删除

```rust
permanently_delete_generation(id)
    ├─ 删除文件系统中的图片和缩略图
    ├─ DELETE FROM images WHERE generation_id = ?
    └─ DELETE FROM generations WHERE id = ?
```

#### 自动清理

启动时按保留天数清理回收站：

```rust
purge_trashed_generations(app, db, retention_days)
    └─ 删除 deleted_at <= (now - retention_days) 的记录
```

默认保留 30 天。

### 文件夹管理

```rust
create_folder(name)        // 创建文件夹
rename_folder(id, name)    // 重命名（default 不可重命名）
delete_folder(id)          // 删除（default 不可删除）
get_folders()              // 获取所有文件夹
```

### 图片收藏

```rust
add_image_to_folders(image_id, folder_ids)     // 添加到文件夹
remove_image_from_folders(image_id, folder_ids) // 从文件夹移除
get_image_folders(image_id)                     // 获取图片所属文件夹
get_favorite_images(folder_id, query, page)     // 获取收藏图片
```

- 默认文件夹 "默认收藏" 不可删除/重命名
- 图片可同时属于多个文件夹
- 收藏查询支持按文件夹和关键词过滤

## 数据模型

```
folders (文件夹)
    │
    ├── folder_images (关联表)
    │       │
    │       └── images (图片)
    │               │
    │               └── generations (生成记录)
    │
    └── 默认文件夹: "default" / "默认收藏"
```

## 前端页面

- **GalleryPage** - 所有图片浏览，支持搜索/过滤/分页
- **FavoritesPage** - 收藏图片，支持文件夹切换
- **TrashPage** - 回收站，支持恢复/永久删除

## 关键代码路径

- 后端：`src-tauri/src/gallery.rs`
- 前端 API：`src/lib/api.ts` → `searchGenerations`, `getFavoriteImages` 等
- 前端查询：`src/lib/queries/favorites.ts`, `src/lib/queries/generations.ts`
- 前端页面：`src/pages/GalleryPage.tsx`, `src/pages/FavoritesPage.tsx`, `src/pages/TrashPage.tsx`

## 相关页面

- [[Database Schema]] - 数据库设计
- [[Image Generation Flow]] - 图像生成流程
