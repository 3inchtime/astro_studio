# Canvas Generation Rounds And Revision Board Design

Date: 2026-07-10
Status: Approved design; written specification pending user review
Owner: Codex main agent
Roadmap milestone: A, after B1 and C1

## Context

Astro Studio's canvas can import images, draw and erase strokes, manage layers,
autosave documents, export a frame, and invoke image editing. It does not yet
provide a reliable AI iteration loop.

The current page exports a frame, writes it to the canvas export directory, and
passes the resulting internal path into the normal edit command. The edit
command accepts database-managed images or file-picker-authorized paths, so an
internal canvas export can be rejected at the authorization boundary. The page
also discards the edit result, provides no candidate review surface, and cannot
place a generated result back on the canvas.

Canvas documents are overwritten in place. In-memory undo is temporary, the
document `version` field is a format version rather than a creative revision,
and generation metadata does not identify the source canvas state.

## Decision

Introduce a first-class canvas generation round and immutable canvas
revisions. A round starts from a source revision, submits a validated managed
frame export through the persistent generation queue, presents all generated
candidates, and records candidate placement as a new revision.

Revision history is a directed parent chain with branches. Historical
revisions are immutable. Restore and branch operations create new heads instead
of overwriting history.

## Dependencies

This design starts only after:

1. The B1 canvas editor integration is complete and accepted.
2. The C1 persistent generation queue is complete and accepted.

It reuses selection, transforms, clipboard, ordering, history, export, queue,
generation lifecycle, gallery, and image persistence rather than creating
parallel implementations.

## Goals

- Fix the current canvas-export authorization break with a trusted command.
- Show generation queue state and candidates inside the canvas workspace.
- Place one accepted candidate per round back onto the canvas as a first-class
  image object.
- Record reliable provenance from output object to source revision and job.
- Preserve meaningful immutable checkpoints without versioning every drag.
- Show a visible revision chain with branches and current-head state.
- Support restore-as-new, branch-from-revision, and two-revision comparison.
- Link Gallery results back to their source canvas revision.

## Non-Goals

- Pixel-level or semantic visual diffs.
- Merging two branches.
- Multiplayer or cloud revision synchronization.
- Full content-addressed asset deduplication.
- Mask layers, inpaint/outpaint, ControlNet, or regional prompts.
- A node-based workflow editor.
- Automatic placement of every candidate.
- Multiple accepted placements from the same round in the first release.

## Data Model

### `canvas_revisions`

| Field | Purpose |
| --- | --- |
| `id` | Revision UUID |
| `document_id` | Owning canvas document |
| `parent_revision_id` | Parent revision for lineage |
| `restored_from_revision_id` | Historical source for restore-as-new |
| `revision_number` | Monotonic display number within a document |
| `reason` | `pre_generation`, `candidate_placed`, `checkpoint`, `restore`, or `branch` |
| `snapshot_path` | Immutable content JSON snapshot |
| `preview_path` | Immutable frame preview PNG |
| `content_hash` | Snapshot integrity hash |
| `created_at` | Creation timestamp |

Indexes support document/time listing and parent traversal.

### `canvas_generation_rounds`

| Field | Purpose |
| --- | --- |
| `id` | Round UUID |
| `document_id` | Owning document |
| `source_revision_id` | Exact source state used for generation |
| `job_id` | Persistent execution job |
| `generation_id` | Gallery/history generation record |
| `frame_json` | Frame geometry and export metadata |
| `prompt` | Prompt snapshot for the revision board |
| `status` | Derived round state for efficient queries |
| `selected_image_id` | Single candidate accepted from the round |
| `result_revision_id` | Revision produced by placement |
| `created_at` | Round creation time |
| `updated_at` | Last state change |

Generated candidates continue to use the existing `images` table through
`generation_id`; no duplicate candidate table is introduced.

### `canvas_documents`

Add nullable `current_revision_id` and nullable `working_content_hash`. The
current JSON file remains a materialized editable head. Revision snapshots are
immutable files. Existing rows are lazily backfilled from their JSON documents
on first load or save. After backfill, every successful autosave updates
`working_content_hash`, so edits made between immutable checkpoints remain
visible to optimistic concurrency checks.

### Canvas Document Format V2

Newly saved content uses format version 2. `CanvasImageObject` gains optional
provenance:

```ts
origin?: {
  kind: "generated";
  generation_id: string;
  image_id: string;
  round_id: string;
  source_revision_id: string;
};
```

The sanitizer reads version 1 and supplies no origin. Existing documents are
not bulk rewritten. A document becomes v2 the next time it is saved through
the new code.

## Revision Creation Policy

Autosave continues to update the materialized head every 500 ms but does not
create an immutable revision.

Create revisions only:

- Before enqueueing a canvas generation.
- After placing a generated candidate.
- When the user explicitly creates a checkpoint.
- When restoring a historical revision.
- When branching from a historical revision.

The first revision for an existing document is created lazily at the first
meaningful boundary. Its parent is null.

## Backend Commands

Add commands and API wrappers:

- `create_canvas_checkpoint(document_id, expected_head, content, preview)`
- `enqueue_canvas_generation(request) -> CanvasGenerationEnqueueResult`
- `list_canvas_revisions(document_id)`
- `get_canvas_revision(revision_id)`
- `list_canvas_generation_rounds(document_id)`
- `place_canvas_generation_result(request) -> CanvasDocumentWithContent`
- `restore_canvas_revision(request) -> CanvasDocumentWithContent`
- `compare_canvas_revisions(left_id, right_id) -> CanvasRevisionComparison`

`enqueue_canvas_generation` accepts:

- Document ID, expected current head, and expected working-content hash.
- Canonical current content.
- Frame PNG base64 generated by the existing frontend exporter.
- Prompt and generation parameters.
- A caller-generated client request ID for idempotent enqueue.

The command validates the content, PNG magic bytes, decoded size, maximum
bytes, document ownership, and expected head. It writes staged snapshot/export
files and atomically creates or updates:

- Source revision.
- Generation record.
- Generation job.
- Canvas generation round.

After validation and hashing, staged files move to their final managed paths
before the DB transaction commits references to them. A failed transaction
deletes only the final files created by that operation. A startup orphan sweep
removes files left by a crash between the move and transaction.

The managed frame export is trusted by the canvas-generation command and never
passes through the normal user-selected-path registry. Other edit commands keep
their existing authorization checks.

## Canvas Generation Flow

1. Autosave pending canvas edits.
2. Export the current frame PNG in the frontend.
3. Call `enqueue_canvas_generation` with content and expected head.
4. Receive job, generation, round, and source revision IDs immediately.
5. Display queued/running state in the canvas inspector.
6. On completion, load candidates from the existing generation result.
7. Let the user inspect candidates and choose one to place.
8. Build the next content with the candidate image positioned at the generated
   frame geometry and carrying provenance.
9. Call `place_canvas_generation_result` with expected current head.
10. Save the materialized document, immutable result revision, and round link.

Submitting a round does not block further canvas editing.

## Concurrent Editing And Branch Semantics

The source revision records what the provider saw. If the user continues
editing while a job runs, the materialized head may differ by completion time.

Placement uses both `expected_current_revision_id` and
`expected_working_content_hash`:

- If the revision and working hash still match the source revision, create a
  child revision and place the candidate.
- If either differs, return `canvas_head_changed` with two explicit choices:
  - Place on the latest head.
  - Create a branch from the source revision.

Placing on the latest head makes the latest head the revision parent while the
image provenance and round still point to the original source revision.
The follow-up placement request includes the latest working-content hash so a
second edit racing with that choice is rejected instead of overwritten.

Branching from the source revision creates a new head whose parent is the
source revision. The previous head remains an accessible sibling branch.

## Idempotent Placement

Placement includes a stable operation ID and selected image ID. The backend
must ensure the same placement operation cannot:

- Insert the same generated image object twice.
- Create multiple result revisions.
- Advance the round to inconsistent result revisions.

A repeated successful request returns the previously created result.

The first release accepts exactly one candidate per round. Other candidates
remain available in Gallery, but accepting another canvas candidate requires a
new round. This keeps `selected_image_id` and `result_revision_id` unambiguous.

## Restore And Compare

Historical revisions are read-only.

Restore:

- Validate snapshot hash and document ownership.
- Create a new revision using the restored snapshot.
- Set `restored_from_revision_id`.
- Use the prior current head as the new revision's parent.
- Advance the current head without mutating the historical revision.

Branch:

- Create content from the selected historical snapshot.
- Use the selected historical revision as the new revision's parent.
- Advance the document head while retaining the previous head as a sibling.

Compare:

- Select exactly two revisions from the version board.
- Show immutable preview images side by side.
- Show prompt, reason, timestamp, generation model, and parent metadata.
- Do not compute a pixel or object diff in the first release.

## User Experience

### Left: Documents And Revision Board

`CanvasAssetSidebar` remains the document selector. A new
`CanvasVersionBoard` shows:

- Revision preview and display number.
- Parent connector and branch indentation.
- Current-head badge.
- Reason and generation status.
- Selection for read-only preview and two-item comparison.
- Restore-as-new, branch, and manual-checkpoint actions.

The version board is visible in the normal canvas workspace, not hidden in a
settings page or modal.

### Center: Canvas Stage

The stage remains editable for the current head. Selecting a historical
revision switches the center to read-only preview mode with a clear banner and
actions to restore or branch. Direct historical mutation is impossible.

### Right: Creation Inspector

The inspector contains Generation, Layers, and Tasks tabs.

The Generation tab shows:

- Prompt and model controls.
- Current round queued/running/terminal state.
- Candidate grid after completion.
- Place on Canvas and Open in Gallery actions.
- Retry through the linked job when allowed.

The Tasks tab is filtered to the current canvas document and reuses global job
components rather than implementing a second task model.

## Frontend Boundaries

`CanvasPage.tsx` remains a composition root and delegates behavior:

- `useCanvasDocumentController`
- `useCanvasSelectionController`
- `useCanvasGenerationRound`
- `useGenerationJobs`

New focused components:

- `CanvasVersionBoard`
- `CanvasRevisionPreview`
- `CanvasRevisionCompareDialog`
- `CanvasGenerationResults`
- `CanvasJobList`

`CanvasStage` receives content and read-only state. It does not fetch revisions,
jobs, rounds, or generations.

## Error Handling

Stable canvas error codes include:

- `canvas_export_invalid`
- `canvas_export_too_large`
- `canvas_document_missing`
- `canvas_head_changed`
- `canvas_revision_missing`
- `canvas_revision_corrupt`
- `canvas_round_missing`
- `canvas_candidate_missing`
- `canvas_candidate_not_in_round`
- `canvas_placement_conflict`
- `canvas_snapshot_write_failed`

Autosave failures keep the document dirty and show an actionable error.
Revision corruption blocks preview/restore for that revision but does not
invalidate the current materialized document.

## Transaction And File Rules

- Use staged files for snapshot, preview, and frame export creation.
- Validate and hash before inserting final references.
- Compare expected current head in the same transaction that advances it.
- Compare expected working-content hash in the same transaction that writes
  the new materialized document.
- Never hold the DB mutex while encoding PNG data.
- Clean only files created by the failed operation.
- Never delete an immutable revision snapshot as part of normal restore or
  branch operations.
- Soft-deleted canvas documents retain revisions until permanent cleanup.

## Testing Strategy

### Pure Frontend Tests

- Document v1-to-v2 sanitization.
- Provenance-preserving image insertion.
- Version-board tree projection and branch ordering.
- Read-only historical mode.
- Candidate placement request construction.

### Rust Tests

- Lazy first revision creation.
- Parent chains, branches, restore-as-new, and current-head updates.
- Expected-head conflict handling.
- Working-content conflict handling between immutable checkpoints.
- Snapshot hashes and corrupt/missing snapshot behavior.
- Round/job/generation/image relationship validation.
- Idempotent placement.
- Staged-file cleanup after injected DB and filesystem failures.
- Path traversal, malformed base64, non-PNG, and oversized export rejection.

### Command Integration Tests

- Real canvas enqueue command through a fake image engine and persistent job.
- Queue completion to candidate listing.
- Candidate placement to result revision.
- Concurrent edit followed by latest-head placement.
- Concurrent edit followed by branch placement.
- Gallery result navigation back to source revision.
- App restart while a canvas job is queued/running/completed.

### Frontend Component Tests

- Round status and candidate rendering.
- Place, retry, Gallery, checkpoint, restore, branch, and compare actions.
- Disabled states for running, missing, corrupt, or conflicting data.
- i18n key parity across all supported locales.

### Release Verification

- Targeted canvas, job, and migration suites.
- `cargo test --lib`.
- `npm test`.
- `npm run build`.
- Formatting and `git diff --check`.
- Tauri smoke flow: edit canvas, generate, navigate away, return, place result,
  branch, restore, compare, restart, and verify provenance.

## Acceptance Criteria

- Canvas generation never depends on user-file-picker path authorization.
- Submission creates a durable round and job and returns immediately.
- The exact source canvas revision can always be identified.
- All completed candidates appear in the canvas inspector and Gallery.
- Exactly one accepted candidate per round can be placed without duplicate
  objects or revisions.
- Concurrent edits produce an explicit latest-head or branch choice.
- Historical revisions are immutable and restorable only as new heads.
- Two revision previews and metadata can be compared side by side.
- A placed object traces to image, generation, job, round, and source revision.
- Version 1 canvas documents remain readable and become version 2 only on a
  later save.
