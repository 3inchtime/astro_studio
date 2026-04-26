# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## Project Overview

Astro Studio is a cross-platform AI image generation desktop client (跨平台 AI 图像生成桌面客户端). It uses Tauri 2.0 to wrap a React frontend with a Rust backend, aggregating AI image generation engines (currently GPT Image) through a native-feeling desktop interface. Target platforms: Windows and macOS.

## Commands

```bash
npm install              # Install frontend dependencies
npm run tauri dev        # Start dev mode (Vite HMR on :1420 + Rust compilation)
npm run tauri build      # Build production bundle (output: src-tauri/target/release/bundle/)
npm run build            # Frontend-only build (tsc && vite build)
npm run dev              # Vite dev server only (no Tauri backend)
```

No test infrastructure exists yet.

## Architecture

### Frontend-Backend Communication

All communication uses Tauri's IPC via `invoke()` — there is no REST API. The bridge is defined in `src/lib/api.ts`, which wraps `@tauri-apps/api/core` invoke calls. Backend commands are Rust functions annotated with `#[tauri::command]` in `src-tauri/src/lib.rs`. Event-based communication (generation progress/complete/failed) uses Tauri's `Emitter` on the Rust side and `listen()` on the frontend.

### Frontend (`src/`)

- **Router**: react-router-dom v7 with three routes: `/generate` (default), `/gallery`, `/settings`
- **Layout**: `AppLayout.tsx` provides a three-column layout — 64px icon sidebar, 220px history sidebar, and main content area via `<Outlet />`
- **Styling**: Tailwind CSS v4 with a comprehensive design system defined in `src/styles/globals.css` using `@theme` directives. Uses warm stone neutrals (not cold grey) with blue-violet primary accent (`#4F6AFF` → `#7C5CFC`). Custom utilities: `glass`, `glass-strong`, `shadow-card`, `gradient-primary`, `shimmer`, `float-in`, `fade-in`, `breathe`
- **Animation**: Framer Motion for component transitions, CSS keyframe animations for ambient effects
- **Font**: Geist Sans loaded from CDN (jsdelivr)
- **Class merging**: `cn()` utility using `clsx` + `tailwind-merge`

### Backend (`src-tauri/src/`)

- **`lib.rs`**: Entry point (`run()`), all Tauri command handlers, window vibrancy setup (Mica on Windows, HudWindow on macOS)
- **`api_gateway.rs`**: Defines `ImageEngine` async trait with `GptImageEngine` as the sole implementation. Calls `{base_url}/images/generations` with Bearer token, expects `b64_json` responses. Designed for future multi-engine support
- **`db.rs`**: SQLite via `rusqlite` with `Mutex<Connection>` pattern (single-writer). WAL mode, foreign keys enabled. Schema: `generations`, `images`, `settings` tables
- **`file_manager.rs`**: Images organized by date (`YYYY/MM/DD/`), 256px thumbnails generated via `image` crate
- **`models.rs`**: Shared data models and constants (engine names, setting keys, default API URL)

### Key Design Decisions

- Window transparency is enabled (`transparent: true` in tauri.conf.json), requiring platform-specific vibrancy code
- Database access uses `Mutex<Connection>` — all DB operations lock the mutex, making them inherently single-threaded
- Image generation is event-driven: the Rust backend emits `generation:progress`, `generation:complete`, `generation:failed` events that the frontend subscribes to
- The `ImageEngine` trait is the extension point for adding new AI engines
- `convertFileSrc()` is used to convert local file paths to Tauri asset URLs for display in `<img>` tags

## Language

README and code comments are primarily Chinese. UI copy is English.
