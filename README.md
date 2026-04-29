<p align="center">
  <img src="src/assets/logo.png" alt="Astro Studio" width="360" />
</p>

<h1 align="center">Astro Studio</h1>

<p align="center">
  A desktop platform for connecting the image generation APIs you choose
  <br />
  Freedom to plug in the third-party image generation services that fit your workflow.
</p>

<p align="center">
  <strong>English</strong> · <a href="./doc/README.zh-CN.md">简体中文</a> · <a href="./doc/README.zh-TW.md">繁體中文</a> · <a href="./doc/README.ja.md">日本語</a> · <a href="./doc/README.ko.md">한국어</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS-lightgrey" alt="Platform" />
  <img src="https://img.shields.io/badge/Product-AI%20Image%20Studio-5b7cff" alt="Product" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="License" />
</p>

---

Astro Studio is a desktop platform built for AI image creation. Its goal is not to lock you into a single model provider, but to give you a cleaner way to connect third-party image generation APIs and manage different services, models, and ongoing creative work from one place.

It is designed to solve more than a single generation request. The real focus is turning image generation into a stable, usable, long-term creative workflow.

## Product Positioning

Astro Studio is not a single-model demo shell, and it is not just another web tool optimized for one-off image outputs.

It is closer to a creator-owned image generation workspace:

- You can freely configure third-party image generation APIs
- You can bring different providers into one desktop entry point
- You can generate, review, filter, favorite, and continue editing in the same space
- You can turn your creative history into a long-term, manageable asset library

If you do not want your workflow tied to a single platform, Astro Studio is built for exactly that way of working.

## Compatibility Progress

- [x] `gpt-image-2`
- [ ] `nano banana` - adaptation testing in progress
- [ ] `nano banana 2` - adaptation testing in progress
- [ ] `nano banana pro` - adaptation testing in progress
- [ ] More third-party image generation APIs

## Why This Product Exists

Many AI image products focus heavily on model capability, but give much less attention to what happens after the image is generated:

- History becomes messy very quickly
- Good results are hard to organize
- Iteration is awkward once context is lost
- Switching providers often means relearning the whole workflow

Astro Studio tries to reorganize those scattered steps and turn image generation from isolated experiments into a continuous creative process.

## Core Value

### Open Connection

Astro Studio is not centered around a preset platform. It supports configurable API keys, base URLs, and model endpoints so you can connect third-party image generation services on your own terms.

### Unified Entry Point

Whether you use official services, proxy gateways, or compatible third-party providers, the goal is to keep everything inside one desktop interface instead of splitting your work across multiple dashboards.

### Built For Ongoing Creation

It focuses on more than successful generation. History, review, favorites, continued editing, and later iteration all matter, so your output does not disappear the moment it is created.

### Desktop-First Experience

Compared with browser tools, Astro Studio emphasizes stability, focus, and repeat use. It is meant to feel like a creative workspace you open every day, not just a webpage you visit occasionally.

## Who It Is For

- Designers, illustrators, and content creators who generate images frequently
- Individuals or small teams using more than one image generation service
- People who want freedom to switch API providers without platform lock-in
- Users who want their history, favorites, assets, and configuration to live in a long-term desktop environment

## What You Can Do With It

- Start image generation quickly from a prompt
- Connect your own third-party image generation APIs
- Create one consistent entry point across services and models
- Browse your full history and revisit past creative outputs
- Favorite images and organize them into folders
- Continue editing and iterating from existing images
- Search past results more efficiently
- Manage the full creative process in a more stable desktop environment

## Current Experience Highlights

- Custom API connection
- Image generation and continued editing
- Persistent conversation history
- Gallery browsing
- Favorites organization
- Trash and recovery flow
- Local creative management

## What We Believe

Astro Studio believes the future of image creation tools should not be limited to being an accessory for a single model. It should be a more neutral, open, and durable platform.

Models will change. Providers will change. Interfaces will change. But creators should not lose control of their workflow every time the ecosystem shifts.

That is why Astro Studio is built to return choice to the user:

- Choose who to connect
- Choose how to work
- Choose how to preserve creative history
- Choose how to organize your asset world

## Get It

Download the installer for your platform from [Releases](https://github.com/3inchtime/astro_studio/releases).

After first launch, complete your API configuration and Astro Studio is ready to serve as your unified desktop entry point for third-party image generation services.

## Build Locally

If you want to run Astro Studio from source, prepare the local toolchain first:

- Node.js `22+`
- npm `11+`
- Rust stable toolchain
- Platform dependencies required by Tauri on your OS

Install dependencies:

```bash
npm install
```

Start the desktop app in development mode:

```bash
npm run tauri dev
```

If you only want to run the frontend in the browser:

```bash
npm run dev
```

Create production builds:

```bash
npm run build
npm run tauri build
```

Current automated GitHub releases publish Windows artifacts. If you build macOS locally for your own use, the app can run normally on your machine, but public distribution still requires Apple signing and notarization.

## Roadmap

- [ ] Add support for more third-party image generation services
- [ ] Improve the multi-provider switching experience
- [ ] Build stronger project-based creative management
- [ ] Expand image editing and reference-image workflows
- [ ] Continue refining favorites, filtering, and asset organization

## License

This project is licensed under the [MIT License](LICENSE).
