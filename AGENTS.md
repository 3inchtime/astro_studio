# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

Astro Studio is a cross-platform AI image generation desktop client (跨平台 AI 图像生成桌面客户端). It uses Tauri 2.0 to wrap a React frontend with a Rust backend, aggregating multiple AI image generation providers (OpenAI GPT Image, Gemini/Nano Banana) through a native-feeling desktop interface. Target platforms: Windows and macOS.

## Commands

```bash
npm ci                   # Install the versions in the frontend lockfile
npm run tauri dev        # Start dev mode (Vite HMR on :1420 + Rust compilation)
npm run tauri build      # Build production bundle (output: src-tauri/target/release/bundle/)
npm run build            # Frontend-only build (tsc && vite build)
npm run dev              # Vite dev server only (no Tauri backend)
npm test                 # Run all vitest tests (jsdom environment)
npx vitest               # Run tests in watch mode
npm run quality:entrypoint # Build, test, check Rust formatting and Clippy, inspect the Tauri toolchain
```

## Architecture

### Frontend-Backend Communication

React calls native commands through the feature adapters in `src/features/*/commands.ts`. `src/lib/api.ts` exposes the shared bridge and the typed generation job API. Rust command handlers in `src-tauri/src/commands/` delegate to domain modules. The desktop event bridge delivers committed job transitions and transient previews. Views also read SQLite-backed history when opening a conversation or canvas and while a task is processing.

### Frontend (`src/`)

- **Router**: react-router-dom v7 with routes: `/generate` (default), `/projects`, `/projects/:projectId`, `/projects/:projectId/chat/:conversationId?`, `/gallery`, `/trash`, `/favorites`, `/settings`
- **Layout**: `AppLayout.tsx` provides a three-column layout — 64px icon sidebar, resizable conversation sidebar, and main content area via `<Outlet />`
- **State management**: `@tanstack/react-query` for server state (API data with caching/invalidation), `zustand` for UI-only state (lightbox, folder selector). React Query hooks are in `src/lib/queries/` — each file wraps a domain's API calls with useQuery/useMutation
- **Styling**: Tailwind CSS v4 with a design system in `src/styles/globals.css` using `@theme` directives. Uses warm stone neutrals with blue-violet primary accent (`#4F6AFF` → `#7C5CFC`). Custom utilities: `glass`, `glass-strong`, `shadow-card`, `gradient-primary`, `shimmer`, `float-in`, `fade-in`, `breathe`
- **Animation**: Framer Motion for component transitions, CSS keyframe animations for ambient effects
- **Font**: Geist Sans loaded from CDN (jsdelivr)
- **Class merging**: `cn()` utility using `clsx` + `tailwind-merge`; `class-variance-authority` for component variants
- **i18n**: i18next with 8 languages (en, de, es, fr, ja, ko, zh-CN, zh-TW), locale files in `src/locales/`, language utilities in `src/lib/languages.ts`
- **Types**: Shared TypeScript interfaces in `src/types/index.ts` — `Generation`, `GeneratedImage`, `Conversation`, `Project`, `LlmConfig`, image parameter types, etc.
- **Tests**: vitest with jsdom, React Testing Library. Config is inline in `vite.config.ts`. Setup file: `src/test/setup.ts`. Run a single test file: `npx vitest run src/pages/GeneratePage.test.tsx`

### Backend (`src-tauri/src/`)

- **`lib.rs`**: App entry point (`run()`), image extension repair, generation recovery on restart, window vibrancy setup (Mica on Windows, HudWindow on macOS), Tauri builder setup registering all commands and managed state
- **`config.rs`**: TOML-based app configuration (`astro_studio.toml`) with sections for logging, API, and storage. Default config is written on first load if missing
- **`api_gateway.rs`**: Defines `ImageEngine` async trait with `GptImageEngine` as the implementation. Provides `decode_images_from_response` for recovery
- **`image_engines/`, `model_catalog.rs`, `model_registry.rs`**: Protocol adapters dispatch image requests. The versioned capability manifest at `shared/model-catalog.v1.json` supplies canonical IDs, aliases, wire models, parameter profiles and endpoint templates to Rust and TypeScript. Add models on an existing protocol through the manifest and exercise the shared fixture. A new wire protocol also needs a Rust adapter and a local HTTP contract test.
- **`llm/`**: LLM client for prompt optimization — `LlmClient` async trait with `chat()` and `chat_with_images()` methods. Implementations: `openai.rs` (OpenAI-compatible APIs), `anthropic.rs` (Claude API). Separate from image generation engine
- **`commands/`**: Tauri command handlers organized by domain — `generation.rs`, `conversations.rs`, `projects.rs`, `prompts.rs`, `settings.rs`, `llm.rs`, `logs.rs`, `mod.rs`
- **`db.rs`**: SQLite via `rusqlite` with `Mutex<Connection>` pattern (single-writer). WAL mode, foreign keys enabled. Schema includes `generations`, `images`, `settings`, `conversations`, `projects`, `folders`, `prompt_favorites`, `logs`, `generation_recoveries` tables with versioned migration tracking
- **`file_manager.rs`**: Images organized by date (`YYYY/MM/DD/`), 256px thumbnails generated via `image` crate
- **`error.rs`**: Typed `AppError` enum using `thiserror` for structured error handling — variants: `ApiKeyNotSet`, `ProviderProfileNotFound`, `Api`, `Network`, `Database`, `FileSystem`, `Validation`
- **`collections/` and `projects/`**: Image and prompt collections, project and conversation queries. The frontend uses `src/features/collections/useCollectionPages.ts` for paginated collection reads.
- **`db/schema.rs` and `db/migrations/`**: Ordered SQLite migrations. Add a numbered migration and register it in the schema runner. Test both upgrading an existing file and creating an empty library.
- **`canvas/`**: Document revisions, generation reconciliation and registered exports. The command adapter resolves the active root and permissions before calling these modules.
- **`storage/assets.rs` and `storage/artifacts.rs`**: Managed asset lookup and portable artifact paths. Use these paths for library reads, backups and deletion checks.
- **`generation_execution.rs` and `generation_lifecycle.rs`**: Durable queue, single-worker dispatch, persistence and response-file recovery.
- **`runtime_logs.rs`**: In-memory ring buffer for real-time log streaming to frontend
- **`models.rs`**: Shared data models, constants (engine names, setting keys, defaults), `LlmConfig` struct

### Key Design Decisions

- Window transparency is enabled (`transparent: true` in tauri.conf.json), requiring platform-specific vibrancy code
- Database access uses `Mutex<Connection>` — all DB operations lock the mutex, making them inherently single-threaded
- Submit with `enqueue_generation` and a stable client request ID. Use the acknowledged job, generation and conversation IDs to reconnect, cancel or wait. The worker emits `generation:job` for state changes and `generation:partial` for live previews.
- The `ImageEngine` trait is the extension point for adding new AI engines; `image_engines/` routes requests to provider-specific implementations
- `convertFileSrc()` is used to convert local file paths to Tauri asset URLs for display in `<img>` tags
- Managed state: `AppConfig`, `Database`, and `GptImageEngine` are injected via Tauri's `.manage()` and accessed in commands via `app.state::<T>()`
- Multi-provider profiles: each engine can have multiple named provider profiles (different API keys/endpoints), managed via `model_provider_profiles` commands
- Generation recovery: interrupted generations (API response received but images not saved) are recovered on next app launch via `generation_recoveries` table
- Generation composer state is scoped by project and conversation. Canvas persistence and generation are coordinated by the hooks in `src/features/canvas/`.

## Language

README and code comments are primarily Chinese. UI copy supports 8 languages via i18next.

## Superpowers 文件

本项目中，任何层级的 `superpowers/` 和 `.superpowers/` 目录都只供本地使用，包括 `docs/superpowers/` 下的设计和计划。使用 Superpowers 技能时，跳过技能要求暂存或提交这些文件的步骤。禁止用 `git add -f` 将它们加入索引。若已误暂存，先取消这些文件的暂存并保留本地副本，再提交其他改动。

新克隆的仓库可运行 `git config --local core.hooksPath .githooks` 启用提交检查。
