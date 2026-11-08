# Astro Studio 前端重构验收记录

日期：2026-11-08。实现源于 `codex/studio-ui-refresh`，纳入 `v0.0.32` 发布。

## 已落地

- 统一工作台导航、可折叠/键盘调宽侧栏、排版、表单、按钮、提示和主题语义色；保留 12 个主题 ID 与 8 种语言。
- 重排生成编辑器：参考图、输入区和操作区分开；常用参数直接可见；高级参数折叠；Enter 换行，Cmd/Ctrl+Enter 发送，并处理输入法组合态。
- 优化生成记录、图片操作、提示词提取、项目及项目详情、画廊、收藏、回收站、画布和设置。
- 统一加载、失败重试、空结果、保存中反馈；失败保留输入；拦截重复操作；防止过期请求覆盖当前页面状态。
- 抽屉、图片预览、文件夹选择、确认框、主题和日期选择器改善焦点进入、Tab、Escape 和关闭后的焦点恢复。嵌套弹窗仅关闭顶层。
- 画布在窄窗口使用文档/属性抽屉，打开抽屉时暂停画布快捷键；集合详情在窄窗口使用抽屉。

设计依据见 [DESIGN.md](DESIGN.md)，产品边界见 [PRODUCT.md](PRODUCT.md)。生产 IPC、路由地址、数据库结构和生成业务协议保持兼容。

## Skills

项目内安装并应用了 [Impeccable](.agents/skills/impeccable/SKILL.md)、[UI UX Pro Max](.agents/skills/ui-ux-pro-max/SKILL.md)、[Web Design Guidelines](.agents/skills/web-design-guidelines/SKILL.md)，结合现有 frontend-design。固定来源版本记录在 DESIGN.md。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| `npm test` | 73 个文件、529 项测试通过 |
| `npm run build` | TypeScript 和 Vite 生产构建通过 |
| `git diff --check` | 通过 |
| 10 个路由 × 900×600 / 1200×800 / 1440×900 | 30 组均无页面/主内容横向溢出，无可见未命名表单控件，无浏览器运行异常 |
| 12 套主题 | 切换及主题存储状态检查通过；基础文字和控件边框对比度有自动测试 |
| 8 种语言 + 大字号 | 设置页均无横向溢出；语言键与插值完整性测试通过 |
| 10 个路由暗色模式 | 已截图检查 |
| 展开状态 | 设置分区、主题弹层、集合详情、嵌套文件夹、图片预览、侧栏、画布属性和日期选择已实测 |
| Impeccable 静态扫描 | 无主要问题；提示的废弃 loading 动画已连同未使用样式移除 |

回归测试覆盖 IME 发送、弹窗忙碌状态、焦点循环、嵌套 Escape、失败保留草稿、局部保存重试、过期请求隔离、已应用筛选的分页、复制失败与图片切换后的异步反馈隔离等行为。

## 本地截图和日志

[浏览截图索引](output/playwright/index.html)。该目录是本地验收产物，已由仓库原有规则忽略，不进入生产包。

- [生成工作台](output/playwright/generate-1440.png) · [900px 生成页](output/playwright/generate-900.png)
- [画布](output/playwright/canvas-1440.png) · [窄窗口属性抽屉](output/playwright/canvas-inspector-900.png)
- [项目概览](output/playwright/projects-qa-project-1200.png) · [画廊详情](output/playwright/gallery-detail-900.png)
- [嵌套文件夹弹窗](output/playwright/gallery-folder-nested-900.png) · [模型设置](output/playwright/settings-model-900.png)
- [测试输出](output/playwright/vitest.log) · [构建输出](output/playwright/build.log)
- [页面检查数据](output/playwright/browser-metrics.txt) · [主题/语言检查数据](output/playwright/theme-locale-metrics.txt)

浏览器使用独立的 [IPC 演示数据脚本](output/playwright/browser-fixture.js)，未调用收费生成接口、未更改真实项目数据。图片占位来自仓库现有文档截图。可在 Vite 本地服务器运行时通过 Playwright CLI 的 `run-code` 注入该脚本，再运行 [页面检查脚本](output/playwright/review-pages.js)。

## 验证边界

本轮完成前端构建、组件/交互回归和 Chromium 浏览器实测。未完成 Windows/macOS 原生安装包内的 WKWebView/WebView2、系统文件对话框、剪贴板、窗口材质和真实供应商生成联调；这些仍需在对应平台发布前验收。

Vite 仍提示入口压缩后约 537 kB 超过默认 500 kB 阈值。构建成功；本轮未通过提高阈值掩盖提示，也未引入运行时依赖。
