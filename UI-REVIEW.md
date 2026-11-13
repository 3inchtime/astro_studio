# Astro Studio 工作台深化重构验收记录

日期：2026-11-13。基线：`68d6e0f` / `v0.0.32`；实现分支：`feat/studio-workbench-v2`。本轮不升级版本或发布。

## 实现内容

- 生成页采用中央会话结果、底部提示词与参考图、右侧参数面板；参数保持挂载，跨断点保留草稿及高级设置状态。提交区内部滚动，操作行始终可达。
- 会话／文档面板在1440px起停靠，生成参数／画布检查器在1200px起停靠；抽屉互斥、管理焦点，Escape恢复触发点。
- 图库／图片收藏按已加载生成记录批量选择，收藏逐图片记录成功检查点，删除确认整条生成记录的影响范围；部分失败仅重试失败项。预览保留列表、筛选、滚动与选择。
- 预览支持图片ID与来源位置过渡，原图加载前保留缩略图；复制、保存、嵌套文件夹及图片切换保持各自反馈。
- 画布右侧改为持续挂载的属性／图层／生成标签页，绘图工具与视图操作分离，保持文档、历史、视口、快捷键、保存和恢复逻辑。
- 项目突出封面、最近活动及会话入口；设置采用扁平分组与待保存／保存中／成功／失败状态，本地字段校验不声称测试连接。
- 八种语言补齐新文案并同步文档语言；改用语义字号变量，最小辅助文字12px。画布与预览背景在所有主题保持中性。

生产Rust代码、IPC、路由、数据库及持久化数据格式、运行时依赖均未改动。设计约束见 [DESIGN.md](DESIGN.md)。

## 自动化与浏览器检查

| 检查 | 结果 |
| --- | --- |
| 完整Vitest | 565项通过，79个文件 |
| TypeScript / Vite | 生产构建通过；入口仍有500kB阈值提示，未提高阈值掩盖 |
| Impeccable静态检测 | `[]`，无发现 |
| 10路由 × 900×600 / 1200×800 / 1440×900 | 30组无页面或主区域横向溢出，无运行异常 |
| 设置／项目页：12主题 × 8语言 × 3状态 | 288组检查通过；大字号下无低于12px文字、未命名控件或遮挡 |
| 生成／图库／画布：12主题 × 8语言 | 288组、576个状态检查通过；192次提交可达性检查通过 |
| 参数抽屉与会话抽屉 | 草稿保留、Escape返回焦点、同时仅一个模态抽屉 |
| 图库批量操作与预览 | 关闭预览后选择保留；局部失败检查点及翻页边界由交互测试覆盖 |

浏览器使用独立IPC模拟数据和本仓库已有文档截图作为图片占位，不调用收费接口、不访问用户项目。普通文字语义色自动验证4.5:1，控件边框3:1；深色主题活动导航实测示例为5.04:1。

## 性能比较

相同设备、相同Chromium、相同数据集，实际滚动或拖拽期间采集`requestAnimationFrame`帧间隔；每场景运行三次，取p95中位数，无CPU／网络节流。

| 场景 | 改造前 | 改造后 | 变化 |
| --- | --- | --- | --- |
| 1000条生成记录图库滚动 | 17.30ms | 17.60ms | +1.73% |
| 200对象画布拖拽 | 17.40ms | 17.70ms | +1.72% |

两项均通过不劣化超过15%的约定门槛。数据仅说明该浏览器场景下的回归情况，不代替真实作品和原生WebView性能验收。

## 原生平台验证边界

macOS已使用独立QA身份与数据库启动真实WKWebView，未读取或复制用户数据库／密钥。忽略目录中的QA源副本仅改动数据库／配置目录、应用身份、标题与窗口尺寸；生产后端未改动。

已验证900×600原生窗口渲染、Enter换行、参数抽屉Tab／Escape与焦点恢复、草稿保留，以及Cmd+Enter触发无供应商配置的真实本地失败状态。隔离数据库确认任务为`provider_configuration_invalid`终态，配置表无密钥；画布创建通过真实IPC完成，文档JSON已写入QA目录。

后续系统锁屏，自动工具无法继续图形操作，已请求用户解锁。未收到解锁回复后，已正常关闭隔离QA程序并重新构建恢复生产调试产物。原生图形编辑及自动保存、设置表单、图片预览、系统保存对话框和剪贴板尚未完成；真实供应商生成未测试。Windows WebView2目标环境不可用，完整Windows原生验收仍待进行。本轮不能宣称两平台原生验收全部完成。

## 独立复核

最终代码／呈现复核为`ship`，范围限于三项修正：旧图加载回调不再覆盖当前图状态；图库移除累积入场延迟；生成结果与详情保存提供失败提示、重复提交拦截及原控件重试。新增测试先复现失败后通过，32项相关测试由复核者独立重跑通过。该结论不代表全部原生平台验收通过。

详情抽屉曾在入场过程中截图，已等待过渡完成后重新截图；30组路由截图及展开状态也已按最终实现更新。

## 本地证据

验收产物位于忽略目录 [output/playwright/v2](output/playwright/v2/)，不进入生产包。

- [生成1440px](output/playwright/v2/review/generate-1440.png) · [参数抽屉900px](output/playwright/v2/review/generate-parameters-900.png)
- [图库](output/playwright/v2/review/gallery-1440.png) · [预览](output/playwright/v2/review/gallery-preview-900.png) · [批量收藏](output/playwright/v2/review/gallery-batch-900.png)
- [画布](output/playwright/v2/review/canvas-1200.png) · [画布检查器](output/playwright/v2/review/canvas-inspector-900.png)
- [测试](output/playwright/v2/vitest.log) · [构建](output/playwright/v2/build.log) · [浏览器检查](output/playwright/v2/final-browser-review.log)
- [核心页面矩阵](output/playwright/v2/core-matrix/audit-summary.json) · [主题语言矩阵](output/playwright/v2/secondary/audit-summary.json) · [性能数据](output/playwright/v2/performance-comparison.json)
- [原生检查](output/playwright/v2/native-verification.json) · [QA隔离说明](output/playwright/v2/native-harness.json)

上一轮2026-11-08的v0.0.32验收记录可从基线提交查看。
