# Astro Studio

<p align="center">
  <strong>跨平台 AI 图像生成桌面客户端</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2.0-blue?logo=tauri" alt="Tauri 2.0" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react" alt="React 19" />
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178C6?logo=typescript" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Rust-1.80+-000000?logo=rust" alt="Rust" />
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey" alt="Platform" />
</p>

---

Astro Studio 是一个深度集成系统底层的专业级 AI 图像生成桌面客户端。它聚合 GPT Image 等多引擎推理能力，通过极简通透的原生界面，让创意表达毫不费力。

## 核心特性

- **多引擎聚合** — 同时接入多种 AI 图像生成引擎，一个 Prompt 获取不同风格结果
- **Canvas 对比模式** — 并排展示不同引擎生成结果，拖拽对比，快速锁定最佳方案
- **智能 Prompt 增强** — 本地 NLP 模型自动补全风格化描述，保护隐私且离线可用
- **本地化数据管理** — 所有历史、Prompt 和 API 密钥加密存储于本地 SQLite，零云依赖
- **即时搜索** — 支持对海量历史图片进行全文搜索，秒级定位
- **原生体验** — 毛玻璃界面、物理动效、系统级窗口融合（Windows Mica / macOS Vibrancy）
- **零依赖分发** — macOS `.dmg` / Windows `.msi`，安装即用

## 技术栈

| 层级 | 技术 | 用途 |
|------|------|------|
| 原生运行时 | Tauri 2.0 | 跨平台桌面容器，~10MB 安装包 |
| 前端 | React 19 + TypeScript | UI 渲染 |
| 构建 | Vite 7.0 | 快速 HMR |
| 样式 | Tailwind CSS 4.0 | 原子化设计系统 |
| 动画 | Framer Motion | 物理弹性过渡 |
| 原生效果 | window-vibrancy | 毛玻璃 / Mica / Vibrancy |
| 后端 | Rust | API 网关、文件管理、加密存储 |
| 数据库 | SQLite (rusqlite) | 本地持久化 |

## 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) >= 1.80
- [Tauri CLI](https://tauri.app/start/prerequisites/)

### 安装与开发

```bash
# 克隆仓库
git clone https://github.com/3inchtime/astro_studio.git
cd astro_studio

# 安装前端依赖
npm install

# 启动开发模式（前端 HMR + Rust 编译）
npm run tauri dev
```

### 构建发布

```bash
# 构建生产版本
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

## 项目结构

```
astro_studio/
├── src/                      # 前端源码
│   ├── pages/                # 页面组件
│   │   ├── GeneratePage.tsx  # 图像生成（对话式交互）
│   │   ├── GalleryPage.tsx   # 生成历史画廊
│   │   └── SettingsPage.tsx  # API 密钥与配置
│   ├── components/
│   │   └── layout/
│   │       └── AppLayout.tsx # 三栏布局（图标栏 + 历史 + 内容）
│   ├── styles/
│   │   └── globals.css       # 设计系统 Token
│   ├── lib/
│   │   ├── api.ts            # Tauri IPC 封装
│   │   └── utils.ts          # 工具函数
│   └── types/
│       └── index.ts          # TypeScript 类型定义
├── src-tauri/                # Rust 后端
│   ├── src/
│   │   ├── lib.rs            # Tauri 命令注册与窗口配置
│   │   ├── api_gateway.rs    # 多引擎调度
│   │   ├── db.rs             # SQLite 数据层
│   │   ├── models.rs         # 数据模型
│   │   └── file_manager.rs   # 图片文件管理
│   └── tauri.conf.json       # Tauri 窗口与打包配置
├── docs/                     # 架构文档
└── package.json
```

## 设计系统

界面遵循 **极简主义 + 毛玻璃** 视觉哲学：

- 窗口圆角 12px / 卡片圆角 8px
- Electric Blue → Violet 渐变作为核心行动色
- 多层柔和阴影建立层级
- Framer Motion 物理动效驱动所有交互过渡

## 许可证

MIT License
