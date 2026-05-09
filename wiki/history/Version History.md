---
type: concept
title: "Version History"
created: 2026-05-09
updated: 2026-05-09
tags: [history, versions, changelog]
status: active
---

# Version History

## 版本记录

### v0.0.6 (当前版本)

**多模型支持与 LLM 集成**

- feat: 多模态提示词优化（图片+文本输入）
- feat: LLM 设置页面重新设计
- feat: i18n 审计与 LLM 设置翻译
- feat: 项目页面马赛克布局重新设计
- feat: 应用图标更新
- fix: 生成完成状态同步修复
- fix: 日期过滤字段组件、i18n 模型标签、源图片映射修复

### v0.0.5

**项目工作区与 UI/UX 改进**

- feat: 项目聊天流程、无限瀑布流加载、React Query 集成
- feat: 版本化数据库迁移追踪
- refactor: 拆分 lib.rs 为模块，类型化错误处理
- feat: 无限瀑布流加载（画廊和收藏夹）
- feat: 完整的项目管理操作
- fix: 项目提示词替换为对话框

### v0.0.4

**提供商配置文件系统**

- feat: 设置页面模型工作区重新设计
- feat: 多提供商配置文件管理
- feat: 提供商配置文件设置 UI
- feat: 提供商配置状态辅助函数
- feat: 从活跃提供商解析图片设置
- feat: 后端提供商配置状态
- fix: 新提供商保持非活跃直到被选择

### v0.0.3

**画廊与交互优化**

- fix: 画廊预览和 lightbox 交互优化
- refactor: 简化对话侧边栏
- feat: 项目首页
- feat: 项目目录页
- fix: 项目路由状态同步

### v0.0.2

**对话与国际化**

- feat: 对话模型与数据库迁移
- feat: 对话命令与自动分组逻辑
- feat: 三栏可调整布局
- feat: MessageBubble、ImageGrid、ConversationTab 组件
- feat: Lightbox 图片查看器（缩放、平移、导航）
- feat: 对话列表侧边栏（日期分组）
- feat: 剪贴板复制和另存为
- feat: 聊天界面重写
- feat: i18next 设置（en、zh-CN）
- feat: 硬编码字符串替换
- feat: 语言设置卡片
- feat: 自动生成标签、新建对话按钮
- fix: 使用 conversation_id 作为标签
- feat: 聊天 UI 重新设计
- feat: 主题切换圆形揭示动画
- feat: 梦幻气泡加载动画
- fix: 图片高度约束
- fix: 图片气泡自适应

### v0.0.1

**初始版本**

- feat: 初始项目结构
- feat: GPT Image 生成
- feat: 基础画廊
- feat: 设置页面

## Git 提交统计

```
总提交数: 80+
主要功能:
- 图像生成/编辑
- 多引擎支持 (OpenAI + Gemini)
- 项目/对话管理
- 画廊/收藏夹/回收站
- LLM 提示词优化
- 国际化 (8 种语言)
- 提供商配置文件
- 生成恢复
```

## 相关页面

- [[Database Migrations]] - 数据库迁移历史
- [[System Architecture]] - 系统架构
