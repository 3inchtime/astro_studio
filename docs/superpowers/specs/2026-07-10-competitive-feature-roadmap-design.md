# Competitive Feature Roadmap Design

Date: 2026-07-10
Status: Approved design; written specification pending user review
Owner: Codex main agent

## Context

Astro Studio is a desktop-first workspace for user-selected image generation
APIs. It already provides multi-provider profiles, generation and edit flows,
projects, conversations, gallery search, favorites, trash recovery, canvas
documents, and a Konva-based infinite canvas.

A code-level comparison against InvokeAI, ComfyUI, Open Generative AI, Cherry
Studio, Fooocus, Easy Diffusion, DiffusionBee, and adjacent canvas projects
identified three product-critical gaps:

1. The infinite-canvas editor helper layer is partially implemented but is not
   wired into the visible editor. Multi-selection, marquee selection,
   clipboard commands, ordering, group movement, shortcuts, and fit actions
   are still missing from the user experience.
2. Canvas generation is not a closed loop. The current canvas export path can
   fail the generation command's authorized-path validation, the returned
   generation result is discarded, and generated candidates cannot be placed
   back on the canvas.
3. Generation is still a long-running IPC call rather than a durable job. The
   app has no persistent queue, cancellation, explicit interruption state,
   safe retry model, or global task center.

The user approved building all three areas. They are intentionally split into
independent milestones so each can have its own design, plan, tests, and review
gate.

## Decision

Use dependency-first delivery rather than a monolithic release:

1. **B1 - Complete the basic canvas editor.**
2. **C1 - Build the persistent generation queue core.**
3. **A - Build canvas generation rounds, revisions, branching, and result
   placement on top of the queue.**
4. **C2 - Complete queue and task-center user experience.**

This order avoids implementing canvas generation against the current
synchronous command contract and then rewriting it immediately for the queue.
It also ensures generated candidates land on an editor that can select, move,
copy, order, and delete them reliably.

## Goals

- Make the canvas a practical object editor before adding higher-level version
  management.
- Make generation durable, cancellable, observable, and recoverable.
- Turn canvas generation into a complete create-review-place workflow.
- Preserve immutable checkpoints at meaningful creative boundaries.
- Expose a visible revision chain that supports branching, restore-as-new, and
  two-revision comparison.
- Preserve provider neutrality and existing OpenAI/Gemini engine boundaries.
- Keep old canvas documents readable and existing generation/gallery data
  usable throughout migrations.

## Non-Goals

- A general node-graph workflow editor.
- Local diffusion-model inference, checkpoint management, LoRA, or ControlNet.
- Multiplayer editing or cloud synchronization.
- Pixel-level revision diffs.
- Full mask, inpaint, outpaint, or regional-guidance tooling.
- Automatic provider failover, weighted routing, or cost accounting in the
  first queue release.
- A replacement of Konva, React Query, Tauri IPC, or SQLite.

## Milestone B1: Basic Canvas Editor Completion

The existing approved design remains the source of truth:

- `docs/superpowers/specs/2026-06-28-infinite-canvas-editor-design.md`
- `docs/superpowers/plans/2026-06-28-infinite-canvas-editor.md`

The remaining implementation scope begins at the current plan's camera-fit,
page wiring, stage selection, locale, and final verification tasks.

Required user-visible capabilities:

- Click selection, Shift toggle selection, and marquee selection.
- Selection of both image and stroke objects.
- Delete, copy, paste, bring forward/backward, and front/back commands.
- Group movement for multi-selection.
- Undo/redo and tool shortcuts that do not fire while typing.
- Temporary spacebar pan, fit frame, fit selection, and zoom status.
- Consistent hidden-layer and locked-layer behavior across every command.

B1 does not change persisted document schema or add revisions.

## Milestone C1: Persistent Generation Queue Core

C1 introduces a durable execution boundary between UI commands and image
engines. A submission returns job and generation identifiers immediately. A
single worker claims queued jobs and calls the existing generation lifecycle.

Required capabilities:

- Durable queued, running, completed, failed, cancelled, and interrupted
  states.
- Atomic creation of the generation record and job record.
- Immutable public request and provider-profile snapshots, excluding secrets.
- Cancellation for queued and running jobs.
- Manual retry as a new child job.
- Limited safe automatic retries for retryable failures.
- Startup recovery without automatically replaying an ambiguous paid request.
- Structured events with job identity, stage, attempt, error code, and
  retryability.

The detailed design is in:

- `docs/superpowers/specs/2026-07-10-persistent-generation-job-queue-design.md`

## Milestone A: Canvas Generation Rounds And Revisions

A canvas generation round begins from an immutable source revision, submits a
managed frame export to the queue, presents the resulting candidates, and lets
the user place one accepted candidate back on the canvas with provenance.

Required capabilities:

- Trusted canvas export ingestion instead of treating an internal path as a
  user-selected source path.
- Immutable revisions created before generation, after candidate placement,
  for manual checkpoints, and for restore/branch actions.
- Candidate result presentation in the canvas inspector.
- Idempotent placement of one accepted candidate per round with optimistic
  working-content checking.
- A visible revision board with current-head and branch relationships.
- Restore-as-new, branch-from-revision, and side-by-side preview comparison.
- Links among canvas revision, generation round, generation job, generation,
  and generated image.
- Gallery navigation back to the source canvas revision.

The detailed design is in:

- `docs/superpowers/specs/2026-07-10-canvas-generation-rounds-design.md`

## Milestone C2: Queue And Task-Center Experience

C2 exposes the queue as a consistent product surface after both normal and
canvas generation use the same execution layer.

Required capabilities:

- A global task badge and task-center drawer.
- Queue order, state, source, provider, attempt, and elapsed-time display.
- Cancel and retry actions with explicit disabled states.
- Direct navigation from a canvas-origin task to its document and round.
- Direct navigation from a normal generation task to its conversation or
  gallery result.
- Canvas-local task filtering in the canvas inspector.

Concurrency configuration, priorities, costs, and queue reordering remain
future extensions. The initial worker concurrency is fixed at one.

## Cross-Cutting Architecture

### Frontend

`CanvasPage.tsx` remains the composition root but delegates stateful behavior:

- `useCanvasDocumentController`: document load, autosave, and current revision.
- `useCanvasSelectionController`: selection, clipboard, and editor commands.
- `useCanvasGenerationRound`: submission, candidate state, and placement.
- `useGenerationJobs`: job queries, events, cancellation, and retry.

`CanvasStage` remains a renderer and pointer-interaction surface. It must not
own persisted selection, generation, revision, or job state.

The canvas layout keeps three clear responsibilities:

- Left: documents and the visible revision board.
- Center: editable or read-only canvas stage and editor toolbar.
- Right: generation, layers, and canvas-related task tabs.

The global task center belongs to `AppLayout` so jobs remain visible across
route changes.

### Backend

New modules keep execution and revision responsibilities out of the existing
large command files:

- `generation_jobs.rs`: job persistence, transitions, commands, and events.
- `generation_job_worker.rs`: claim, execute, cancel, retry delay, and startup
  reconciliation.
- `canvas_revisions.rs`: immutable snapshots, heads, branches, and restore.
- `canvas_generation.rs`: managed frame exports, generation rounds, and result
  placement validation.

`generation_lifecycle.rs` continues to own provider invocation, response
recovery, image saving, and generation terminal updates. The worker calls it;
UI commands do not.

## Data Model Evolution

Migrations are additive and sequenced:

1. Add `generation_jobs` and supporting indexes.
2. Add `canvas_revisions` and `canvas_generation_rounds`.
3. Add nullable `current_revision_id` and `working_content_hash` to
   `canvas_documents`; lazily backfill hashes for existing documents.
4. Upgrade newly saved canvas content to document format v2 with optional image
   provenance. Continue reading v1 without an eager bulk migration.

Existing canvas JSON remains the materialized current document. Immutable
revision snapshots are separate files referenced by the revision table.

## Revision Policy

The 500 ms document autosave continues to update the materialized head but
does not create an immutable revision for every pointer movement.

Create a revision only at meaningful boundaries:

- Immediately before a canvas generation is enqueued.
- After a generated candidate is placed.
- When the user explicitly creates a checkpoint.
- When restoring or branching from a historical revision.

This keeps the version board useful instead of flooding it with incidental
edits.

## Reliability Rules

- Never persist API keys in job snapshots or events.
- Never automatically replay a request whose provider outcome is ambiguous.
- A retry creates a new job linked through `parent_job_id`.
- Historical canvas revisions are immutable.
- Restore creates a new head; it never overwrites history.
- Candidate placement is idempotent and checks both the expected revision head
  and working-content hash.
- File writes use staging paths, validation, hashes, and cleanup on failed DB
  transactions.
- Autosave failures retain dirty state and show an actionable error.

## Testing And Release Gates

Each milestone has an independent plan and review gate. Later milestones do not
start until the current milestone's semantics and edge cases are accepted.

Every milestone must pass, as applicable:

- Targeted Vitest and Rust tests.
- `npm test`.
- `npm run build`.
- `cargo test --lib`.
- Formatting and `git diff --check`.
- A Tauri desktop smoke flow covering the milestone's real IPC boundaries.

The queue and canvas-generation milestones additionally require fake-engine
integration tests, injected failure/restart tests, migration tests, and
security tests for managed exports and secret-free snapshots.

## Definition Of Done

The roadmap is complete when:

- B1 canvas editor commands are visible, consistent, and tested.
- All normal and canvas generation submissions use durable jobs.
- Users can cancel and manually retry jobs and understand terminal failures.
- Canvas generation produces visible candidates and one accepted candidate per
  round can be placed back onto the canvas.
- Every placed candidate can be traced to its source revision, round, job,
  generation, and image.
- Users can view the revision chain, branch or restore without destroying
  history, and compare two revision previews.
- Global and canvas-local task surfaces remain consistent after navigation and
  app restart.

## Documentation And Implementation Handoff

This document controls dependency order and release gates. Each detailed spec
controls its subsystem. After written-spec review, create separate
implementation plans and execute them in this order:

1. Complete the existing B1 plan.
2. Write and execute the C1 queue implementation plan.
3. Write and execute the A canvas-generation implementation plan.
4. Extend the C plan for the C2 task-center milestone.
