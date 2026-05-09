# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

Astro Studio is a cross-platform AI image generation desktop client (跨平台 AI 图像生成桌面客户端). It uses Tauri 2.0 to wrap a React frontend with a Rust backend, aggregating multiple AI image generation providers (OpenAI GPT Image, Gemini/Nano Banana) through a native-feeling desktop interface. Target platforms: Windows and macOS.

## Commands

```bash
npm install              # Install frontend dependencies
npm run tauri dev        # Start dev mode (Vite HMR on :1420 + Rust compilation)
npm run tauri build      # Build production bundle (output: src-tauri/target/release/bundle/)
npm run build            # Frontend-only build (tsc && vite build)
npm run dev              # Vite dev server only (no Tauri backend)
npm test                 # Run all vitest tests (jsdom environment)
npx vitest               # Run tests in watch mode
```

## Architecture

### Frontend-Backend Communication

All communication uses Tauri's IPC via `invoke()` — there is no REST API. The bridge is defined in `src/lib/api.ts`, which wraps `@tauri-apps/api/core` invoke calls. Backend commands are Rust functions annotated with `#[tauri::command]` organized in `src-tauri/src/commands/`. Event-based communication (generation progress/complete/failed, runtime logs) uses Tauri's `Emitter` on the Rust side and `listen()` on the frontend.

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
- **`image_engines/`**: Multi-provider routing — `openai.rs` (GPT Image) and `gemini.rs` (Nano Banana models). `ImageProvider` enum dispatches requests to the correct provider based on model
- **`model_registry.rs`**: Model normalization (aliases to canonical IDs) and classification (OpenAI vs Gemini). Endpoint URL construction for each provider
- **`llm/`**: LLM client for prompt optimization — `LlmClient` async trait with `chat()` and `chat_with_images()` methods. Implementations: `openai.rs` (OpenAI-compatible APIs), `anthropic.rs` (Claude API). Separate from image generation engine
- **`commands/`**: Tauri command handlers organized by domain — `generation.rs`, `conversations.rs`, `projects.rs`, `prompts.rs`, `settings.rs`, `llm.rs`, `logs.rs`, `mod.rs`
- **`db.rs`**: SQLite via `rusqlite` with `Mutex<Connection>` pattern (single-writer). WAL mode, foreign keys enabled. Schema includes `generations`, `images`, `settings`, `conversations`, `projects`, `folders`, `prompt_favorites`, `logs`, `generation_recoveries` tables with versioned migration tracking
- **`file_manager.rs`**: Images organized by date (`YYYY/MM/DD/`), 256px thumbnails generated via `image` crate
- **`error.rs`**: Typed `AppError` enum using `thiserror` for structured error handling — variants: `ApiKeyNotSet`, `ProviderProfileNotFound`, `Api`, `Network`, `Database`, `FileSystem`, `Validation`
- **`gallery.rs`**: Gallery search, trash/restore/permanent-delete, folder CRUD, favorite images
- **`runtime_logs.rs`**: In-memory ring buffer for real-time log streaming to frontend
- **`models.rs`**: Shared data models, constants (engine names, setting keys, defaults), `LlmConfig` struct

### Key Design Decisions

- Window transparency is enabled (`transparent: true` in tauri.conf.json), requiring platform-specific vibrancy code
- Database access uses `Mutex<Connection>` — all DB operations lock the mutex, making them inherently single-threaded
- Image generation is event-driven: the Rust backend emits `generation:progress`, `generation:complete`, `generation:failed`, `runtime-log:new` events that the frontend subscribes to
- The `ImageEngine` trait is the extension point for adding new AI engines; `image_engines/` routes requests to provider-specific implementations
- `convertFileSrc()` is used to convert local file paths to Tauri asset URLs for display in `<img>` tags
- Managed state: `AppConfig`, `Database`, and `GptImageEngine` are injected via Tauri's `.manage()` and accessed in commands via `app.state::<T>()`
- Multi-provider profiles: each engine can have multiple named provider profiles (different API keys/endpoints), managed via `model_provider_profiles` commands
- Generation recovery: interrupted generations (API response received but images not saved) are recovered on next app launch via `generation_recoveries` table
- Frontend API layer is a thin wrapper — `api.ts` functions map 1:1 to Rust commands. React Query hooks in `queries/` add caching/invalidation on top

## Language

README and code comments are primarily Chinese. UI copy supports 8 languages via i18next.
