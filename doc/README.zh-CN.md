<p align="center">
  <img src="../src/assets/logo.png" alt="Astro Studio" width="360" />
</p>

<h1 align="center">Astro Studio</h1>

<p align="center">
  自由接入第三方图片生成 API 的桌面平台
  <br />
  A desktop platform for connecting the image generation APIs you choose.
</p>

<p align="center">
  <a href="../README.md">English</a> · <strong>简体中文</strong> · <a href="./README.zh-TW.md">繁體中文</a> · <a href="./README.ja.md">日本語</a> · <a href="./README.ko.md">한국어</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey" alt="Platform" />
  <img src="https://img.shields.io/badge/Product-AI%20Image%20Studio-5b7cff" alt="Product" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="License" />
</p>

---

Astro Studio 是一个围绕 AI 图像创作打造的桌面平台，重点不是绑定某一家模型服务，而是让你可以更自由地接入第三方图片生成 API，用同一套界面管理不同服务、不同模型和持续发生的创作过程。

它想解决的不是“如何完成一次生成”，而是“如何把图像生成真正变成稳定、顺手、长期可用的工作流”。

## 产品定位

Astro Studio 不是单一模型的演示壳，也不是只强调一次性出图的网页工具。

它更像一个属于创作者自己的图像生成工作台：

- 你可以自由配置第三方图片生成 API
- 你可以把不同供应商接入同一个桌面入口
- 你可以在同一个空间里完成生成、回看、筛选、收藏和继续编辑
- 你可以把创作历史沉淀为长期可管理的素材资产

如果你不希望自己的工作流被某一个平台绑定，Astro Studio 就是为这种使用方式而设计的。

## 适配进度

- [x] `gpt-image-2`
- [ ] `nano banana` - 适配测试中
- [ ] `nano banana 2` - 适配测试中
- [ ] `nano banana pro` - 适配测试中
- [ ] 更多第三方图片生成 API

## 为什么做这个产品

现在很多 AI 图片产品都在强调模型能力本身，却很少认真处理“创作之后”的问题：

- 历史记录很快变乱
- 喜欢的结果难以整理
- 想继续迭代时，上下文已经断掉
- 更换服务商时，整个使用习惯也要跟着重来

Astro Studio 希望把这些零散环节重新组织起来，让图像生成从一次次试验，变成一条连续的创作流程。

## 核心价值

### 自由接入

Astro Studio 的核心不是预设某个固定平台，而是支持自由配置 API Key、Base URL 与模型入口，让你可以按自己的方式接入第三方图片生成能力。

### 统一入口

无论你使用官方服务、代理网关，还是兼容接口的第三方提供方，都可以尽量在同一个桌面界面里完成操作，而不是在多个网页后台之间反复切换。

### 面向长期创作

它关注的不只是“生成成功”，还包括历史积累、作品回看、收藏整理、继续编辑与后续迭代，让创作结果不是生成完就消失。

### 桌面体验

相比网页工具，Astro Studio 更强调稳定、专注、持续使用的感觉。它更像一个每天都会打开的创作工作台，而不是偶尔使用一次的在线页面。

## 适合谁

- 经常使用 AI 生成图片的设计师、插画师、内容创作者
- 需要同时使用不同图片生成服务的个人用户或小团队
- 希望自由切换 API 供应商、不愿被单一平台绑定的人
- 想把历史、收藏、素材和配置长期沉淀在自己桌面环境里的用户

## 你可以用它做什么

- 输入提示词，快速发起图片生成
- 接入你自己的第三方图片生成 API
- 在不同服务与模型之间建立统一的使用入口
- 浏览完整历史，回看每一次创作结果
- 收藏喜欢的图片，并按文件夹整理
- 基于已有图片继续编辑和迭代
- 通过搜索更快找到过去生成过的内容
- 用更稳定的本地桌面方式管理整个创作过程

## 当前体验重点

- 自定义 API 接入
- 图片生成与继续编辑
- 历史会话沉淀
- 画廊浏览
- 收藏夹整理
- 回收站恢复
- 本地化创作管理

## 我们相信什么

Astro Studio 相信，未来的图像创作工具不应该只是某个模型的附属界面，而应该是一个更中立、更开放、更长期可用的平台。

模型会变化，供应商会变化，接口也会变化，但创作者对工作流的掌控不应该跟着一起丢失。

所以 Astro Studio 想做的，是把“选择权”重新交还给用户：

- 选择接入谁
- 选择怎么用
- 选择如何保留自己的创作历史
- 选择怎样组织自己的素材世界

## 获取方式

前往 [Releases](https://github.com/3inchtime/astro_studio/releases) 下载对应平台的安装包。

首次启动后，只需要完成自己的 API 配置，就可以开始使用 Astro Studio 作为第三方图片生成服务的统一桌面入口。

## 本地构建

如果你想从源码本地运行 Astro Studio，先准备好基础环境：

- Node.js `22+`
- npm `11+`
- Rust stable toolchain
- 当前操作系统所需的 Tauri 依赖

安装项目依赖：

```bash
npm install
```

以桌面应用开发模式启动：

```bash
npm run tauri dev
```

如果你只想单独启动前端页面：

```bash
npm run dev
```

执行生产构建：

```bash
npm run build
npm run tauri build
```

当前 GitHub 自动发布流程只产出 Windows 安装包。如果你在本地构建 macOS 版本自用，应用通常可以在自己的机器上运行；但如果要公开分发，仍然需要额外完成 Apple 签名与公证。

## Roadmap

- [ ] 接入更多第三方图片生成服务
- [ ] 支持更完整的多供应商切换体验
- [ ] 提供更强的项目化创作管理能力
- [ ] 增强图片编辑与参考图工作流
- [ ] 继续完善收藏、筛选与素材组织体验

## 许可证

本项目采用 [MIT License](LICENSE)。
