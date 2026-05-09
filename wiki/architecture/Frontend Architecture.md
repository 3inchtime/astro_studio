---
type: concept
title: "Frontend Architecture"
created: 2026-05-09
updated: 2026-05-09
tags: [architecture, frontend, react]
status: active
---

# Frontend Architecture

## 技术栈

- **React 19.1** - UI 框架
- **TypeScript 5.8** - 类型安全
- **Tailwind CSS 4.2** - 样式系统，使用 `@theme` 指令定义设计系统
- **React Router 7.x** - 路由管理
- **React Query 5.x** - 服务端状态管理（缓存/失效）
- **Zustand 5.x** - 客户端 UI 状态
- **Framer Motion 12.x** - 组件动画
- **i18next 26.x** - 国际化 (8 种语言)
- **Lucide React** - 图标库

## 路由结构

| 路径 | 页面 | 说明 |
|------|------|------|
| `/generate` | GeneratePage | 默认页，AI 图像生成 |
| `/projects` | ProjectsPage | 项目列表 |
| `/projects/:projectId` | ProjectHomePage | 项目详情 |
| `/projects/:projectId/chat/:conversationId?` | ProjectChatPage | 项目对话 |
| `/gallery` | GalleryPage | 图片画廊 |
| `/trash` | TrashPage | 回收站 |
| `/favorites` | FavoritesPage | 收藏夹 |
| `/settings` | SettingsPage | 设置 |

## 布局系统

`AppLayout.tsx` 提供三栏布局：

```
┌──────┬──────────────────┬────────────────────────┐
│ 64px │  Resizable       │   Main Content         │
│ Icon │  Conversation    │   <Outlet />           │
│ Side │  Sidebar         │                        │
│ bar  │                  │                        │
└──────┴──────────────────┴────────────────────────┘
```

- 左侧：64px 图标导航栏
- 中间：可调整宽度的对话侧边栏
- 右侧：主内容区，通过 `<Outlet />` 渲染路由组件

## 状态管理

### React Query (服务端状态)
- 位置：`src/lib/queries/`
- 每个领域一个文件：`generations.ts`、`projects.ts`、`settings.ts`、`favorites.ts`、`llm.ts`
- 封装 API 调用，添加缓存/失效策略
- 默认 `staleTime: 30_000`，不重试

### Zustand (UI 状态)
- 位置：`src/lib/store.ts`
- 管理 lightbox、folder selector 等纯 UI 状态

## API 层

`src/lib/api.ts` 是前端与后端通信的唯一入口：

```typescript
// 每个函数映射到一个 Rust 命令
export async function generateImage(params: GenerationParams): Promise<GenerateResponse> {
  return invoke("generate_image", { ... });
}
```

### 事件监听

```typescript
onGenerationProgress(handler)  // generation:progress
onGenerationComplete(handler)  // generation:complete
onGenerationFailed(handler)    // generation:failed
onRuntimeLog(handler)          // runtime-log:new
```

## 样式系统

- 设计系统定义在 `src/styles/globals.css`，使用 Tailwind `@theme` 指令
- 主色调：暖石中性色 + 蓝紫主色 (`#4F6AFF` → `#7C5CFC`)
- 自定义工具类：`glass`、`glass-strong`、`shadow-card`、`gradient-primary`、`shimmer`、`float-in`、`fade-in`、`breathe`
- 字体：Geist Sans (CDN 加载)
- 类合并：`cn()` = `clsx` + `tailwind-merge`
- 组件变体：`class-variance-authority`

## 组件结构

```
src/components/
├── common/          # 通用组件
├── favorites/       # 收藏夹相关
├── gallery/         # 画廊相关
├── generate/        # 生成页相关
├── layout/          # 布局组件 (AppLayout)
├── lightbox/        # 图片查看器
├── projects/        # 项目管理
├── settings/        # 设置页
└── sidebar/         # 侧边栏
```

## 测试

- 框架：Vitest + React Testing Library
- 环境：jsdom
- 配置：内联于 `vite.config.ts`
- 设置文件：`src/test/setup.ts`
- 运行：`npm test` 或 `npx vitest run src/pages/GeneratePage.test.tsx`

## 相关页面

- [[System Architecture]] - 系统架构总览
- [[Backend Architecture]] - 后端架构详解
- [[IPC Communication]] - IPC 通信机制
