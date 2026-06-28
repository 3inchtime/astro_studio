# Generation Lifecycle Deepening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the duplicated generate/edit lifecycle out of the Tauri command module into a deeper Rust module while preserving the existing IPC interface and image generation behavior.

**Architecture:** Keep `commands::generation::{generate_image, edit_image}` as thin adapters for Tauri state and source image validation. Add a `generation_lifecycle` module that owns request kind naming, request option normalization, generation metadata, recovery state, database persistence, engine dispatch, file saving, runtime events, and lifecycle logging. The provider adapter seam stays in `api_gateway::ImageEngine`.

**Tech Stack:** Rust 2021, Tauri 2 IPC, rusqlite, async-trait image engine abstraction, existing Cargo tests.

---

### Task 1: Add Lifecycle Request Interface Tests

**Files:**
- Create: `src-tauri/src/generation_lifecycle.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/generation_lifecycle.rs`

- [ ] **Step 1: Write the failing test**

Add `generation_lifecycle` as a crate module and define tests that expect a lifecycle request kind to own the persisted request kind string and source image metadata behavior:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DEFAULT_IMAGE_COUNT;

    #[test]
    fn lifecycle_request_kind_names_generate_and_edit_for_persistence() {
        assert_eq!(GenerationLifecycleKind::Generate.as_str(), "generate");
        assert_eq!(GenerationLifecycleKind::Edit.as_str(), "edit");
    }

    #[test]
    fn lifecycle_metadata_counts_source_images_without_storing_paths() {
        let options = image_request_options(None, None, None, None, None, None, None, Some(DEFAULT_IMAGE_COUNT));
        let metadata = generation_request_metadata_json(
            GenerationLifecycleKind::Edit,
            "conversation-1",
            "gpt-image-2",
            &options,
            &["/Users/example/private.png".to_string()],
        )
        .expect("serialize metadata");

        assert!(metadata.contains("\"request_kind\":\"edit\""));
        assert!(metadata.contains("\"source_image_count\":1"));
        assert!(!metadata.contains("private.png"));
        assert!(!metadata.contains("source_image_paths"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test generation_lifecycle --lib`

Expected: FAIL because `generation_lifecycle`, `GenerationLifecycleKind`, `image_request_options`, and `generation_request_metadata_json` do not exist in the new module yet.

- [ ] **Step 3: Implement the minimal interface**

Create `src-tauri/src/generation_lifecycle.rs` with the request kind enum and move the pure option/metadata helpers from `commands/generation.rs` into it. Add `mod generation_lifecycle;` in `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test generation_lifecycle --lib`

Expected: PASS for the new lifecycle module tests.

### Task 2: Move Shared Lifecycle Execution

**Files:**
- Modify: `src-tauri/src/generation_lifecycle.rs`
- Modify: `src-tauri/src/commands/generation.rs`
- Test: existing Rust generation, API, image engine, and path boundary tests

- [ ] **Step 1: Write the failing integration-shaped test**

Add a unit test in `generation_lifecycle.rs` that uses a fake `ImageEngine` and a temporary database/app fixture only if the existing Tauri test helpers make it straightforward. If not, use the existing command-level tests as the behavior safety net and keep the new module tests focused on the extracted interface.

- [ ] **Step 2: Run targeted Rust tests**

Run: `cargo test commands::generation generation_lifecycle --lib`

Expected before production move: existing tests pass, new execution test fails if added because execution is not implemented in the lifecycle module.

- [ ] **Step 3: Move implementation**

Move these responsibilities from `commands/generation.rs` to `generation_lifecycle.rs`:

- `RECOVERY_STATE_REQUESTING`
- `RECOVERY_STATE_RESPONSE_READY`
- lifecycle request kind constants
- image request option normalization
- source path JSON serialization
- generation request metadata JSON serialization
- `create_processing_generation`
- `update_generation_recovery_response`
- `set_generation_failed`
- `save_generation_images`
- shared generate/edit orchestration

Leave source image selection and managed-path validation in `commands/generation.rs`, because those are command adapter responsibilities shared with clipboard/save commands.

- [ ] **Step 4: Thin the command adapters**

Update `generate_image` and `edit_image` so they build a lifecycle request and call the new module. Keep their IPC parameters and return type unchanged.

- [ ] **Step 5: Run targeted tests**

Run: `cargo test commands::generation generation_lifecycle image_engines api_gateway --lib`

Expected: all targeted tests pass.

### Task 3: Full Verification and Release Preparation

**Files:**
- Modify: release metadata only after refactor verification

- [ ] **Step 1: Run full test suite**

Run: `npm test`

Expected: all Vitest tests pass.

Run: `cargo test` from `src-tauri`

Expected: all Rust tests pass.

- [ ] **Step 2: Run production build checks**

Run: `npm run build`

Expected: TypeScript and Vite production build pass.

- [ ] **Step 3: Commit the refactor**

Run:

```bash
git add docs/superpowers/plans/2026-06-28-generation-lifecycle-deepening.md src-tauri/src/lib.rs src-tauri/src/generation_lifecycle.rs src-tauri/src/commands/generation.rs
git commit -m "refactor: deepen generation lifecycle"
```

- [ ] **Step 4: Merge, tag, and release**

After verification passes, merge `codex/architecture-deepening-refactor` into `main`, create the next version tag, push branch/main/tag, and use the project release workflow expected by the repository.
