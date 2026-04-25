# 图片收藏功能设计

## 概述

用户可以将单张图片加入收藏文件夹，并可通过侧边栏「收藏」标签浏览和管理所有收藏图片。

## 数据模型

### 数据库表

**folders**
- `id TEXT PRIMARY KEY`
- `name TEXT NOT NULL`
- `created_at TEXT NOT NULL DEFAULT (datetime('now'))`

**folder_images** (多对多关联表)
- `folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE`
- `image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE`
- `added_at TEXT NOT NULL DEFAULT (datetime('now'))`
- PRIMARY KEY (folder_id, image_id)

**默认文件夹**：系统自动创建一个名为"默认收藏"的文件夹（id=`default`），不可删除。

### 设计规则

- 图片可以属于多个文件夹（多对多）
- 图片不属于任何文件夹时视为"未收藏"
- 删除文件夹时，自动从 `folder_images` 移除关联记录（CASCADE）
- 删除图片时，自动从 `folder_images` 移除关联记录（CASCADE）

## API（Rust 命令）

### 文件夹管理
- `create_folder(name: String) -> Folder`
- `rename_folder(id: String, name: String) -> ()`
- `delete_folder(id: String) -> ()` （id=`default` 不可删除）
- `get_folders() -> Vec<Folder>`

### 图片收藏
- `add_image_to_folders(image_id: String, folder_ids: Vec<String>) -> ()`
- `remove_image_from_folders(image_id: String, folder_ids: Vec<String>) -> ()`
- `get_image_folders(image_id: String) -> Vec<String>` 返回文件夹 ID 列表
- `get_favorite_images(page: i64, query: Option<String>) -> SearchResult` 分页 + 模糊搜索 prompt

### 实现要点
- 复用现有 `search_generations` 的分页逻辑
- 搜索通过 SQL LIKE 实现：`WHERE g.id IN (SELECT image_id FROM folder_images WHERE folder_id = ?) AND g.prompt LIKE ?`
- 不需要新增 `is_favorited` 字段，通过 `folder_images` 表判断

## 前端

### 页面：FavoritesPage
- 路由：`/favorites`
- 入口：侧边栏「收藏」图标标签
- 布局：网格视图 + 顶部搜索框 + 分页
- 网格卡片：与 GalleryPage 风格一致，悬停显示 prompt
- 点击卡片：打开右侧详情面板（复用画廊详情面板模式）
- 详情面板：显示图片信息 + prompt，支持管理所属文件夹

### 组件：FolderSelector
- 触发：ImageGrid 收藏按钮、画廊卡片浮层、Lightbox 工具栏
- 交互：弹窗显示所有文件夹（多选 checkbox），底部「新建文件夹」输入框 + 确认按钮
- 确认后调用 `add_image_to_folders` / `remove_image_from_folders`

### 三处收藏入口
1. **ImageGrid** — 图片下方按钮栏，Sparkles 图标改为星形，onClick 打开 FolderSelector
2. **画廊网格** — 卡片右上角星形浮层（hover 显示），onClick 打开 FolderSelector
3. **Lightbox** — 工具栏增加星形按钮，onClick 打开 FolderSelector

### 侧边栏
- 现有三个标签：Generate / Gallery / Settings
- 新增「收藏」标签（Heart/Star 图标）
- 点击切换到 `/favorites` 路由

### 状态管理
- `useFavoriteFolders(imageId)` hook：管理单个图片的收藏状态（所在文件夹列表）
- `useFolders()` hook：管理文件夹列表（create/rename/delete）
- FolderSelector 内部组合这两个 hook

## UI 细节

### 收藏状态图标
- 未收藏：空心星形（outline star icon）
- 已收藏：实心星形（filled star icon），颜色 primary
- hover：scale 1.1

### FolderSelector 弹窗
- 标题：「加入收藏夹」
- 列表：每个文件夹一行，checkbox + 文件夹名称
- 底部：「新建文件夹」输入框 + 加号按钮
- 确认/取消按钮

### FavoritesPage 网格
- 与 GalleryPage 相同的网格布局（grid-cols-2 sm:grid-cols-3 lg:grid-cols-4）
- 卡片悬停：-translate-y-0.5 + shadow-float
- 右上角显示收藏标记（实心星形）

## 实施步骤

1. 数据库迁移：新增 `folders` 和 `folder_images` 表
2. Rust：实现文件夹管理 API + 收藏 API
3. 前端：api.ts 包装函数
4. 前端：FolderSelector 弹窗组件
5. 前端：useFavoriteFolders / useFolders hooks
6. 前端：ImageGrid 收藏按钮
7. 前端：画廊卡片收藏浮层
8. 前端：Lightbox 收藏按钮
9. 前端：侧边栏收藏标签
10. 前端：FavoritesPage
11. 前端：右侧详情面板支持管理文件夹

## 已知约束

- 滚轮缩放功能已在 Lightbox 实现，不在本功能范围内
- 删除/下载功能不在本功能范围内（已在之前实现了复制按钮）
