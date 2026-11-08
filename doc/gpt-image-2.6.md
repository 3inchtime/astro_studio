# GPT Image 2.6 支持说明

版本：v0.0.31。依据 2026-11-08 查阅的 OpenAI 官方文档实现。

| 能力 | 应用支持与限制 |
| --- | --- |
| 双模型 | `gpt-image-2.6-flare`、`gpt-image-2.6-sunburst`；新安装默认 Flare，已有模型选择与 2.5 配置保留 |
| 质量与复杂度 | auto/low/medium/high/xhigh/max；独立 complexity low/medium/high，未指定时使用模型默认值 |
| 尺寸 | auto 或自定义 WIDTHxHEIGHT；像素与比例约束按输出格式校验 |
| 背景 | auto/opaque/transparent；透明输出使用 PNG/WebP/SVG |
| 输出 | PNG/JPEG/WebP/SVG/GIF；JPEG/WebP 压缩率 0–100 |
| 批量与参考图 | 单次 1–10 个输出；编辑最多 16 张参考图 |
| SVG | 仅文生图，一次一张；可选作者模型及 reasoning effort，需账号授权；不支持流式和压缩率 |
| GIF | 文生图或参考图编辑；2–16 帧，每帧至少 20ms，支持 u32 整数请求，服务端按 10ms 舍入并将编码延时封顶于 655,350ms；不支持流式、mask、压缩率 |
| 编辑 | 上传透明 PNG 标注层（markup），或 alpha 遮罩（mask），两者互斥；作用于首张参考图，尺寸必须一致 |
| 流式 | PNG/JPEG/WebP 可接收 SSE，0–3 个部分预览；生成页及画布显示预览；仅最终结果写入资产 |
| 内容审核 | auto/low；上游错误按类别返回，不自动重试用户可纠正的错误 |
| 恢复与导出 | 高级选项保留在任务快照；失败/中断后可显式重试；画布重试复用原始导出源图；SVG/GIF 保存原始字节，缩略图和复制使用栅格预览 |
| 国际化 | 八种界面语言均包含新增控件和限制提示 |

## 使用

在设置中为 2.6 模型配置提供商。新模型使用独立配置，旧模型和自定义端点不会被覆盖。生成页选择 2.6 后，展开“高级图像设置”。选择 GIF 或 SVG 会显示相应控件。标注和遮罩通过“选择…PNG”上传，修改首张参考图会清除旧标注，避免误用。画布修改源内容也会清除旧标注；失败任务的重试使用原始源图与参数。

## 实际边界

- GIF preview 和 SVG 作者模型选择由账号/服务商授权决定。未开启作者模型选择时保持作者模型为空；推理等级是否支持取决于所选作者模型。第三方兼容网关需自行支持新增字段与格式。
- GIF 的每帧尺寸、帧数及张数共同受上游总像素预算限制；`auto` 为推荐默认值。超额时减少尺寸、帧数或张数。帧延时按 API reference 接受 20–4,294,967,295ms 的整数请求（u32 传输范围），编码后的延时最多为 655,350ms；不将指南示例的 2000ms 当作接口上限。
- SVG 原始文件可直接导出；本地预览采用静态安全子集（路径、形状、文字、渐变、clipPath、mask），拒绝脚本、外部资源、嵌入位图、动画、DTD、`use`、`pattern` 和 `filter`。预览限制为 4MiB XML、20,000 节点、64 层、8192 单边及 16,777,216 像素；超出限制明确失败，不转成假 SVG。
- GIF 在画廊/大图查看器打开原图时播放；Konva 画布仅显示静帧，但导出原文件保留动画。
- SVG 不是图像编辑的栅格参考输入。需要继续编辑时先将其导出为 PNG。
- 验证包含本地 mock HTTP/SSE、参数/恢复/文件格式测试和前端交互回归。未进行收费官方账号实图调用，因此不把账号权限或真实生成效果视为已验证。

## 官方依据

- [图像生成指南](https://developers.openai.com/api/docs/guides/image-generation)
- [2.6 提示与参数指南](https://developers.openai.com/api/docs/guides/image-prompting)（明确包含 xhigh/max 和 SVG 透明背景）
- [创建图像 API](https://developers.openai.com/api/reference/resources/images/methods/generate)
- [编辑图像 API](https://developers.openai.com/api/reference/resources/images/methods/edit)
- [Flare 模型](https://developers.openai.com/api/docs/models/gpt-image-2.6-flare)、[Sunburst 模型](https://developers.openai.com/api/docs/models/gpt-image-2.6-sunburst)
