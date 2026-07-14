# Canvas Generation Rounds And Revision Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the infinite canvas into a traceable AI iteration workspace with managed generation rounds, immutable revisions, candidate placement, branching, restore-as-new, comparison, and Gallery provenance.

**Architecture:** Add a v17 revision/round migration and focused Rust modules for immutable snapshot files and canvas-specific queue submission. The canvas frontend delegates document, selection, generation-round, and job responsibilities to focused hooks; revision history is a visible parent tree, while the current JSON document remains the autosaved materialized head.

**Tech Stack:** Rust, Tauri 2, rusqlite, serde, SHA-compatible content hashing using existing or minimal standard dependencies, React 19, TypeScript, TanStack Query, react-konva, Vitest, React Testing Library.

---

## Dependencies

Do not start this plan until:

- B1 Tasks 4-8 in `2026-06-28-infinite-canvas-editor.md` are accepted.
- C1 queue core Tasks 1-8 in `2026-07-10-persistent-generation-job-queue.md` are accepted.

Complete this plan before C2. Canvas-local task rendering establishes the
shared job-action row that C2 reuses, while both surfaces use the C1 job types
and query keys.

## File Structure

- Modify `src-tauri/src/db.rs`: v17 revision/round schema and migration tests.
- Modify `src-tauri/Cargo.toml` and `src-tauri/Cargo.lock`: SHA-256 content hashing dependency.
- Modify `src-tauri/src/models.rs`: revision, round, placement, restore, compare, and canvas provenance models.
- Create `src-tauri/src/canvas_revisions.rs`: snapshots, previews, hashes, head checks, branch/restore, compare metadata, orphan cleanup.
- Create `src-tauri/src/canvas_generation.rs`: trusted PNG validation, atomic round/job creation, candidate lookup, and idempotent placement.
- Modify `src-tauri/src/commands/canvas.rs`: revision/round commands and working-content hash persistence.
- Modify `src-tauri/src/commands/mod.rs`: expose focused canvas modules if routed through commands.
- Modify `src-tauri/src/lib.rs`: register commands and run orphan cleanup.
- Modify `src/types/index.ts`: canvas document v2 provenance, revisions, rounds, comparisons, and command requests.
- Modify `src/lib/canvas/document.ts`: v1/v2 sanitize and generated-image insertion helper.
- Modify `src/lib/canvas/document.test.ts`: backward compatibility and provenance tests.
- Modify `src/lib/api.ts`: revision, round, enqueue, placement, restore, and compare wrappers.
- Modify `src/lib/api.test.ts`: exact IPC mapping.
- Modify `src/lib/queries/canvasDocuments.ts`: revision/round queries and mutations.
- Create `src/hooks/useCanvasDocumentController.ts`: materialized document, autosave, head/hash, and historical preview mode.
- Create `src/hooks/useCanvasSelectionController.ts`: move existing page editor command ownership into a focused hook.
- Create `src/hooks/useCanvasGenerationRound.ts`: queue submission, result loading, placement, conflict choice.
- Create `src/components/canvas/CanvasVersionBoard.tsx`: visible revision tree and actions.
- Create `src/components/canvas/CanvasRevisionCompareDialog.tsx`: two-preview metadata comparison.
- Create `src/components/canvas/CanvasGenerationResults.tsx`: round status and candidate acceptance.
- Create `src/components/canvas/CanvasJobList.tsx`: document-filtered jobs using shared queue components.
- Create `src/components/jobs/GenerationJobActions.tsx`: source-aware shared cancel/retry action row reused by C2.
- Modify `src/components/canvas/CanvasAssetSidebar.tsx`: document/revision layout host.
- Modify `src/components/canvas/CanvasGenerationPanel.tsx`: compose/results state.
- Modify `src/components/canvas/CanvasStage.tsx`: read-only historical mode.
- Modify `src/pages/CanvasPage.tsx`: compose hooks/components rather than own all behavior.
- Modify `src/pages/CanvasPage.test.tsx`: real round/revision interaction tests.
- Modify `src/components/gallery/GenerationDetailPanel.tsx`: open source canvas revision action.
- Modify `src/components/gallery/GenerationDetailPanel.test.tsx`: provenance navigation.
- Modify `src/locales/*.json` and `src/i18n.test.ts`: version/round/conflict labels.

## Task 1: Canvas V2 Types And Backward-Compatible Sanitization

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/canvas/document.ts`
- Modify: `src/lib/canvas/document.test.ts`
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: Write failing v1/v2 document tests**

Add focused tests:

```ts
it("loads a version 1 image without generated provenance", () => {
  const content = sanitizeCanvasDocumentContent(versionOneDocumentWithImage());
  const image = content.layers[0].objects[0];
  expect(content.version).toBe(2);
  expect(image.type === "image" ? image.origin : undefined).toBeUndefined();
});

it("preserves complete generated image provenance", () => {
  const content = createCanvasDocumentContent();
  const next = insertGeneratedCanvasImage(content, {
    imagePath: "/managed/result.png",
    imageId: "image-1",
    generationId: "generation-1",
    roundId: "round-1",
    sourceRevisionId: "revision-1",
    frame: content.frame,
    targetLayerId: content.layers[0].id,
  });
  const image = next.layers[0].objects[0];
  expect(image.type === "image" ? image.origin : null).toEqual({
    kind: "generated",
    generation_id: "generation-1",
    image_id: "image-1",
    round_id: "round-1",
    source_revision_id: "revision-1",
  });
});
```

Add the Rust round-trip test beside the canvas models:

```rust
#[test]
fn canvas_generated_origin_round_trips_through_rust_model() {
    let value = serde_json::json!({
        "version": 2,
        "viewport": { "x": 0.0, "y": 0.0, "scale": 1.0 },
        "frame": { "x": 0.0, "y": 0.0, "width": 512.0, "height": 512.0, "aspect": "1:1" },
        "layers": [{
            "id": "layer-1",
            "name": "Layer 1",
            "visible": true,
            "locked": false,
            "objects": [{
                "type": "image",
                "id": "object-1",
                "image_path": "/managed/result.png",
                "x": 0.0,
                "y": 0.0,
                "width": 512.0,
                "height": 512.0,
                "original_width": 512.0,
                "original_height": 512.0,
                "rotation": 0.0,
                "opacity": 1.0,
                "origin": {
                    "kind": "generated",
                    "generation_id": "generation-1",
                    "image_id": "image-1",
                    "round_id": "round-1",
                    "source_revision_id": "revision-1"
                }
            }]
        }]
    });
    let content: CanvasDocumentContent = serde_json::from_value(value).unwrap();
    let encoded = serde_json::to_value(content).unwrap();

    assert_eq!(
        encoded
            .pointer("/layers/0/objects/0/origin/round_id")
            .and_then(serde_json::Value::as_str),
        Some("round-1"),
    );
}
```

- [ ] **Step 2: Run document tests and verify RED**

```bash
npx vitest run src/lib/canvas/document.test.ts
cd src-tauri && cargo test --lib canvas_generated_origin_round_trips_through_rust_model
```

Expected: FAIL because v2 origin and insertion helper are missing.

- [ ] **Step 3: Add exact v2 types and sanitizer behavior**

Define:

```ts
export interface CanvasGeneratedImageOrigin {
  kind: "generated";
  generation_id: string;
  image_id: string;
  round_id: string;
  source_revision_id: string;
}

export interface CanvasImageObject {
  type: "image";
  id: string;
  image_path: string;
  x: number;
  y: number;
  width: number;
  height: number;
  original_width: number;
  original_height: number;
  rotation: number;
  opacity: number;
  origin?: CanvasGeneratedImageOrigin;
}
```

Add matching Rust `CanvasGeneratedImageOrigin` and nullable/defaulted
`CanvasObject.origin`. Unknown or incomplete origins are rejected by the v2
sanitizer rather than partially persisted.

Newly created/sanitized content uses `version: 2`; v1 input is accepted without an eager storage migration. `insertGeneratedCanvasImage` must validate target layer visibility/lock state according to the same mutation rules as existing helpers and position the image at the recorded frame.

- [ ] **Step 4: Run document tests GREEN**

```bash
npx vitest run src/lib/canvas/document.test.ts
cd src-tauri && cargo test --lib canvas_generated_origin_round_trips_through_rust_model
```

Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add src/types/index.ts src/lib/canvas/document.ts src/lib/canvas/document.test.ts src-tauri/src/models.rs
git commit -m "feat: add canvas image provenance"
```

## Task 2: Revision And Round Database Migration

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: Write failing v17 migration tests**

Add assertions equivalent to:

```rust
#[test]
fn fresh_database_migrations_create_canvas_revision_tables() {
    let db_path = test_db_path("astro-studio-canvas-revision-migration-test");
    let database = Database::open(&db_path).expect("open test db");
    database.run_migrations().expect("run migrations");
    let conn = database.conn.lock().expect("lock db");
    assert!(table_has_column(&conn, "canvas_documents", "current_revision_id"));
    assert!(table_has_column(&conn, "canvas_documents", "working_content_hash"));
    assert!(table_has_column(&conn, "canvas_revisions", "content_hash"));
    assert!(table_has_column(&conn, "canvas_generation_rounds", "source_revision_id"));
    assert!(table_has_column(&conn, "canvas_generation_rounds", "parent_round_id"));
    assert!(migration_version_exists(&conn, 17));
}
```

- [ ] **Step 2: Run migration test and verify RED**

```bash
cd src-tauri && cargo test --lib fresh_database_migrations_create_canvas_revision_tables
```

Expected: FAIL because v17 is missing.

- [ ] **Step 3: Add v17 schema**

Implement this logical schema:

```sql
ALTER TABLE canvas_documents ADD COLUMN current_revision_id TEXT;
ALTER TABLE canvas_documents ADD COLUMN working_content_hash TEXT;
CREATE TABLE IF NOT EXISTS canvas_revisions (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES canvas_documents(id) ON DELETE CASCADE,
    parent_revision_id TEXT REFERENCES canvas_revisions(id) ON DELETE SET NULL,
    restored_from_revision_id TEXT REFERENCES canvas_revisions(id) ON DELETE SET NULL,
    revision_number INTEGER NOT NULL,
    reason TEXT NOT NULL,
    snapshot_path TEXT NOT NULL,
    preview_path TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(document_id, revision_number)
);
CREATE TABLE IF NOT EXISTS canvas_generation_rounds (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES canvas_documents(id) ON DELETE CASCADE,
    parent_round_id TEXT REFERENCES canvas_generation_rounds(id) ON DELETE SET NULL,
    source_revision_id TEXT NOT NULL REFERENCES canvas_revisions(id),
    job_id TEXT NOT NULL UNIQUE REFERENCES generation_jobs(id),
    generation_id TEXT NOT NULL UNIQUE REFERENCES generations(id),
    frame_json TEXT NOT NULL,
    prompt TEXT NOT NULL,
    status TEXT NOT NULL,
    selected_image_id TEXT REFERENCES images(id) ON DELETE SET NULL,
    result_revision_id TEXT REFERENCES canvas_revisions(id) ON DELETE SET NULL,
    placement_operation_id TEXT UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_canvas_revisions_document
    ON canvas_revisions(document_id, revision_number);
CREATE INDEX IF NOT EXISTS idx_canvas_revisions_parent
    ON canvas_revisions(parent_revision_id);
CREATE INDEX IF NOT EXISTS idx_canvas_rounds_document
    ON canvas_generation_rounds(document_id, created_at);
```

- [ ] **Step 4: Define Rust models**

Add `CanvasRevision`, `CanvasGenerationRound` (including nullable
`parent_round_id`), `CanvasRevisionReason`, `CanvasRevisionComparison`,
enqueue/placement/restore/retry requests, and response types using
serde-compatible snake_case fields that exactly mirror TypeScript names.

- [ ] **Step 5: Run migration tests GREEN**

```bash
cd src-tauri && cargo test --lib fresh_database_migrations_create_canvas_revision_tables
```

Expected: PASS.

- [ ] **Step 6: Commit Task 2**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs
git commit -m "feat: add canvas revision schema"
```

## Task 3: Immutable Revision Repository And Managed Files

**Files:**
- Create: `src-tauri/src/canvas_revisions.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Test: `src-tauri/src/canvas_revisions.rs`

- [ ] **Step 1: Write failing repository/file tests**

Use temporary app-data directories and real SQLite:

```rust
#[test]
fn creates_lazy_first_revision_and_monotonic_children() {
    let fixture = RevisionFixture::new();
    let first = fixture.checkpoint("manual").unwrap();
    let second = fixture.checkpoint("manual").unwrap();

    assert_eq!(first.parent_revision_id, None);
    assert_eq!(first.revision_number, 1);
    assert_eq!(second.parent_revision_id.as_deref(), Some(first.id.as_str()));
    assert_eq!(second.revision_number, 2);
}

#[test]
fn restore_uses_current_head_as_parent_and_records_source() {
    let fixture = RevisionFixture::with_two_revisions();
    let old_head = fixture.current_head();
    let selected = fixture.revision(1);
    let restored = fixture.restore(&selected.id).unwrap();

    assert_eq!(restored.parent_revision_id.as_deref(), Some(old_head.id.as_str()));
    assert_eq!(restored.restored_from_revision_id.as_deref(), Some(selected.id.as_str()));
    assert_eq!(fixture.current_head().id, restored.id);
}

#[test]
fn branch_uses_selected_revision_as_parent_and_keeps_old_head() {
    let fixture = RevisionFixture::with_two_revisions();
    let old_head = fixture.current_head();
    let selected = fixture.revision(1);
    let branch = fixture.branch(&selected.id).unwrap();

    assert_eq!(branch.parent_revision_id.as_deref(), Some(selected.id.as_str()));
    assert_eq!(fixture.revision_by_id(&old_head.id).unwrap(), old_head);
    assert_eq!(fixture.current_head().id, branch.id);
}

#[test]
fn corrupt_snapshot_is_rejected_without_changing_current_document() {
    let fixture = RevisionFixture::with_two_revisions();
    let selected = fixture.revision(1);
    let before = fixture.current_document();
    fixture.overwrite_snapshot(&selected.id, br#"{"version":2,"layers":[]}"#);
    let error = fixture.restore(&selected.id).unwrap_err();

    assert_eq!(error.stable_code(), "canvas_revision_corrupt");
    assert_eq!(fixture.current_document(), before);
}

#[test]
fn failed_transaction_cleans_only_new_final_files() {
    let fixture = RevisionFixture::with_two_revisions();
    let existing_files = fixture.referenced_files();
    fixture.fail_next_revision_insert();
    assert!(fixture.checkpoint("manual").is_err());

    assert_eq!(fixture.referenced_files(), existing_files);
    assert!(existing_files.iter().all(|path| path.exists()));
    assert!(fixture.unreferenced_new_files().is_empty());
}
```

Implement `RevisionFixture` in the module test block with a temporary app-data
directory and migrated SQLite database. `fail_next_revision_insert` installs a
temporary SQLite trigger that aborts the next insert, so cleanup is exercised
against the real file and transaction code.

- [ ] **Step 2: Run repository tests and verify RED**

```bash
cd src-tauri && cargo test --lib canvas_revisions::tests
```

Expected: FAIL because the module is missing.

- [ ] **Step 3: Implement focused revision operations**

Create:

```rust
pub(crate) fn hash_canvas_content(content_json: &[u8]) -> String;
pub(crate) fn create_revision(
    conn: &mut rusqlite::Connection,
    files: &CanvasRevisionFiles,
    request: CreateCanvasRevisionRequest,
) -> Result<CanvasRevision, AppError>;
pub(crate) fn stage_revision_files(
    files: &CanvasRevisionFiles,
    request: &CreateCanvasRevisionRequest,
) -> Result<PreparedCanvasRevision, AppError>;
pub(crate) fn insert_revision_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    prepared: &PreparedCanvasRevision,
) -> Result<CanvasRevision, AppError>;
pub(crate) fn list_revisions(
    conn: &rusqlite::Connection,
    document_id: &str,
) -> Result<Vec<CanvasRevision>, AppError>;
pub(crate) fn load_revision_content(
    conn: &rusqlite::Connection,
    revision_id: &str,
) -> Result<serde_json::Value, AppError>;
pub(crate) fn restore_revision(
    conn: &mut rusqlite::Connection,
    files: &CanvasRevisionFiles,
    request: &RestoreCanvasRevisionRequest,
) -> Result<CanvasDocumentWithContent, AppError>;
pub(crate) fn branch_from_revision(
    conn: &mut rusqlite::Connection,
    files: &CanvasRevisionFiles,
    request: &BranchCanvasRevisionRequest,
) -> Result<CanvasDocumentWithContent, AppError>;
pub(crate) fn compare_revisions(
    conn: &rusqlite::Connection,
    left_revision_id: &str,
    right_revision_id: &str,
) -> Result<CanvasRevisionComparison, AppError>;
```

Add `sha2 = "0.10"` and implement lowercase SHA-256 content hashes. The
`create_revision` wrapper stages/validates files, opens a transaction, delegates
the row insert to `insert_revision_in_transaction`, and commits. The canvas
enqueue/placement code will reuse the transaction primitive inside a wider
transaction; it must not duplicate revision SQL. Use staging files,
validate/hash, move to final paths, then commit DB references. On transaction
failure delete only files created by the operation. Add startup orphan cleanup
for unreferenced revision/export files older than a conservative cutoff.

- [ ] **Step 4: Register module and run tests GREEN**

```bash
cd src-tauri && cargo test --lib canvas_revisions::tests
```

Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/canvas_revisions.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: persist immutable canvas revisions"
```

## Task 4: Working-Content Hash And Revision Commands

**Files:**
- Modify: `src-tauri/src/commands/canvas.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/canvas_revisions.rs`

- [ ] **Step 1: Add failing canvas command tests**

Cover lazy hash backfill, autosave hash updates, expected hash conflict, checkpoint, restore, branch, list/get, and compare:

```rust
#[test]
fn save_canvas_document_updates_working_content_hash() {
    let fixture = CanvasCommandFixture::new();
    let saved = fixture.save_document(document_content("edited")).unwrap();
    let persisted = fixture.get_document();
    let expected_hash = hash_canvas_content(&fixture.document_bytes());

    assert_eq!(saved.working_content_hash, persisted.working_content_hash);
    assert_eq!(
        persisted.working_content_hash.as_deref(),
        Some(expected_hash.as_str()),
    );
}

#[test]
fn checkpoint_rejects_stale_working_hash() {
    let fixture = CanvasCommandFixture::with_checkpoint();
    fixture.save_document(document_content("newer edit")).unwrap();
    let error = fixture
        .checkpoint(fixture.current_revision_id(), "stale-working-hash")
        .unwrap_err();

    assert_eq!(error.stable_code(), "canvas_head_changed");
    assert_eq!(fixture.revision_count(), 1);
}

#[test]
fn historical_restore_creates_new_head_without_mutating_source() {
    let fixture = CanvasCommandFixture::with_two_checkpoints();
    let source = fixture.revision(1);
    let source_bytes = fixture.snapshot_bytes(&source.id);
    let restored = fixture.restore(&source.id).unwrap();

    assert_ne!(restored.current_revision_id.as_deref(), Some(source.id.as_str()));
    assert_eq!(fixture.snapshot_bytes(&source.id), source_bytes);
    assert_eq!(fixture.revision_count(), 3);
}
```

- [ ] **Step 2: Run canvas command tests and verify RED**

```bash
cd src-tauri && cargo test --lib commands::canvas::tests
```

Expected: New tests fail because hash/revision commands are absent.

- [ ] **Step 3: Extend document responses and save semantics**

Return nullable `current_revision_id` and `working_content_hash` in canvas metadata. On load/save of an existing null hash, compute and persist it. Every successful autosave updates the hash in the same logical operation as the materialized JSON update.

- [ ] **Step 4: Add and register commands**

Implement `create_canvas_checkpoint`, `list_canvas_revisions`, `get_canvas_revision`, `restore_canvas_revision`, `branch_canvas_revision`, and `compare_canvas_revisions` using repository operations. All mutating commands require expected head and expected working hash.

- [ ] **Step 5: Run canvas tests GREEN**

```bash
cd src-tauri && cargo test --lib commands::canvas
cargo test --lib canvas_revisions
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

```bash
git add src-tauri/src/commands/canvas.rs src-tauri/src/canvas_revisions.rs src-tauri/src/lib.rs
git commit -m "feat: expose canvas revision commands"
```

## Task 5: Trusted Canvas Generation Enqueue

**Files:**
- Create: `src-tauri/src/canvas_generation.rs`
- Modify: `src-tauri/src/commands/canvas.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing trusted-export and enqueue tests**

Cover valid PNG, malformed base64, wrong magic, oversized data, traversal-resistant paths, idempotent client request, atomic source revision/job/round creation, and staged-file cleanup:

```rust
#[test]
fn canvas_enqueue_creates_revision_job_generation_and_round_atomically() {
    let fixture = CanvasGenerationFixture::new();
    let result = fixture.enqueue("canvas-request-1", valid_frame_png()).unwrap();

    assert_eq!(fixture.revision_count(), 1);
    assert_eq!(fixture.job_count(), 1);
    assert_eq!(fixture.generation_count(), 1);
    assert_eq!(fixture.round_count(), 1);
    assert_eq!(fixture.round(&result.round_id).job_id, result.job_id);
    assert_eq!(fixture.round(&result.round_id).source_revision_id, result.source_revision_id);
}

#[test]
fn canvas_enqueue_rejects_non_png_before_creating_records() {
    let fixture = CanvasGenerationFixture::new();
    let error = fixture.enqueue("canvas-request-1", jpeg_data_url()).unwrap_err();

    assert_eq!(error.stable_code(), "canvas_export_invalid");
    assert_eq!(fixture.revision_count(), 0);
    assert_eq!(fixture.job_count(), 0);
    assert_eq!(fixture.generation_count(), 0);
    assert_eq!(fixture.round_count(), 0);
}

#[test]
fn repeated_canvas_client_request_returns_original_round() {
    let fixture = CanvasGenerationFixture::new();
    let first = fixture.enqueue("canvas-request-1", valid_frame_png()).unwrap();
    let second = fixture.enqueue("canvas-request-1", valid_frame_png()).unwrap();

    assert_eq!(first.round_id, second.round_id);
    assert_eq!(first.job_id, second.job_id);
    assert_eq!(fixture.job_count(), 1);
}

#[test]
fn failed_round_insert_rolls_back_revision_generation_and_job() {
    let fixture = CanvasGenerationFixture::new();
    fixture.fail_next_round_insert();
    assert!(fixture.enqueue("canvas-request-1", valid_frame_png()).is_err());

    assert_eq!(fixture.revision_count(), 0);
    assert_eq!(fixture.job_count(), 0);
    assert_eq!(fixture.generation_count(), 0);
    assert_eq!(fixture.round_count(), 0);
    assert!(fixture.unreferenced_new_files().is_empty());
}
```

- [ ] **Step 2: Run tests and verify RED**

```bash
cd src-tauri && cargo test --lib canvas_generation::tests
```

Expected: FAIL because the module is missing.

- [ ] **Step 3: Implement managed export validation**

Decode base64 with the same prefix handling as existing canvas preview/export
code. Enforce PNG signature, decoded-byte limit, decodable dimensions,
document ownership, and generated internal UUID filenames. Use the validated
frame PNG both as the provider input and as the immutable source-revision
preview. Never register the export as a user-selected path.

- [ ] **Step 4: Implement enqueue transaction**

Add:

```rust
pub(crate) fn enqueue_canvas_generation(
    app: &tauri::AppHandle,
    db: &Database,
    queue: &GenerationJobQueue,
    request: CanvasGenerationEnqueueRequest,
) -> Result<CanvasGenerationEnqueueResult, AppError>;
```

Generate all IDs first, stage and move only the validated files, then lock the
database and begin one transaction. Within that transaction call
`insert_revision_in_transaction`, call the accepted C1
`insert_job_in_transaction` with `source_kind="canvas"` and a source reference
containing the new round/document/revision IDs, insert the round, and update the
document head. Commit once, clean only operation-created files on failure, and
wake the worker only after commit. Do not call the transaction-owning queue
wrapper and do not duplicate generation/job SQL. Duplicate
`client_request_id` returns the original round/job IDs without new files.

- [ ] **Step 5: Register command and run tests GREEN**

```bash
cd src-tauri && cargo test --lib canvas_generation
cargo test --lib commands::canvas
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

```bash
git add src-tauri/src/canvas_generation.rs src-tauri/src/commands/canvas.rs src-tauri/src/lib.rs
git commit -m "feat: enqueue trusted canvas generation"
```

## Task 6: Idempotent Candidate Placement And Round Queries

**Files:**
- Modify: `src-tauri/src/canvas_generation.rs`
- Modify: `src-tauri/src/commands/canvas.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing placement tests**

Cover candidate ownership, exactly-one acceptance, operation idempotency, working-hash conflict, latest-head placement, branch placement, and provenance persistence:

```rust
#[test]
fn placement_rejects_image_from_another_generation() {
    let fixture = CanvasGenerationFixture::with_completed_round();
    let other_image = fixture.image_for_other_generation();
    let error = fixture.place(&other_image.id, "placement-1").unwrap_err();

    assert_eq!(error.stable_code(), "canvas_candidate_not_in_round");
    assert_eq!(fixture.revision_count(), 1);
}

#[test]
fn repeated_placement_operation_returns_same_revision() {
    let fixture = CanvasGenerationFixture::with_completed_round();
    let image = fixture.round_image();
    let first = fixture.place(&image.id, "placement-1").unwrap();
    let second = fixture.place(&image.id, "placement-1").unwrap();

    assert_eq!(first.current_revision_id, second.current_revision_id);
    assert_eq!(fixture.generated_object_count(&image.id), 1);
    assert_eq!(fixture.revision_count(), 2);
}

#[test]
fn changed_working_hash_requires_latest_or_branch_choice() {
    let fixture = CanvasGenerationFixture::with_completed_round();
    fixture.save_newer_edit();
    let image = fixture.round_image();
    let error = fixture.place(&image.id, "placement-1").unwrap_err();

    assert_eq!(error.stable_code(), "canvas_head_changed");
    assert_eq!(fixture.generated_object_count(&image.id), 0);
}

#[test]
fn placement_rejects_invalid_preview_without_mutation() {
    let fixture = CanvasGenerationFixture::with_completed_round();
    let image = fixture.round_image();
    let error = fixture
        .place_with_preview(&image.id, "placement-1", jpeg_data_url())
        .unwrap_err();

    assert_eq!(error.stable_code(), "canvas_export_invalid");
    assert_eq!(fixture.generated_object_count(&image.id), 0);
    assert_eq!(fixture.revision_count(), 1);
}

#[test]
fn retry_canvas_round_creates_child_job_and_new_round_atomically() {
    let fixture = CanvasGenerationFixture::with_failed_retryable_round();
    let parent_round = fixture.current_round();
    let retried = fixture.retry_round(&parent_round.id, "retry-request-1").unwrap();

    assert_ne!(retried.round_id, parent_round.id);
    assert_eq!(fixture.round(&retried.round_id).parent_round_id.as_deref(), Some(parent_round.id.as_str()));
    assert_eq!(fixture.job(&retried.job_id).parent_job_id.as_deref(), Some(parent_round.job_id.as_str()));
    assert_eq!(fixture.round_count(), 2);
}
```

- [ ] **Step 2: Run placement tests and verify RED**

```bash
cd src-tauri && cargo test --lib canvas_generation::tests::placement
```

Expected: FAIL because placement is missing.

- [ ] **Step 3: Implement round listing and placement**

Add list/get round operations plus `place_canvas_generation_result`. The
placement request contains the canonical post-insertion content and a
`preview_png_base64` produced by `exportCanvasFrame` from that exact content.
Validate the preview PNG with the same managed decoder, validate that the
round completed, the image belongs to its generation, no different image is
already selected, expected head/hash match the selected placement mode, and
origin metadata is complete. Stage the result snapshot and preview, then
persist the materialized document, immutable result revision, selected image,
result revision ID, and new working hash in one transaction.

The same `placement_operation_id` must return the existing result. A different operation after a round already has an accepted image returns `canvas_placement_conflict`.

Add `retry_canvas_generation_round(round_id, client_request_id)`. It creates a
new round with `parent_round_id` pointing to the failed/interrupted round and
reuses the immutable source revision, frame, prompt, and public request
snapshot. In one transaction it calls C1
`insert_retry_job_in_transaction(..., source_ref_override)` so the child job's
source reference points at the new round, inserts the new round, and commits;
the old round/job remain immutable. Reject non-retryable or non-terminal
parents. The generic C1 retry command remains unavailable for `canvas` source
jobs, preventing an orphan child job.

- [ ] **Step 4: Run placement and revision tests GREEN**

```bash
cd src-tauri && cargo test --lib canvas_generation
cargo test --lib canvas_revisions
```

Expected: PASS.

- [ ] **Step 5: Commit Task 6**

```bash
git add src-tauri/src/canvas_generation.rs src-tauri/src/commands/canvas.rs src-tauri/src/lib.rs
git commit -m "feat: place and retry canvas generation results"
```

## Task 7: Frontend Canvas Revision And Round API

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/api.test.ts`
- Modify: `src/lib/queries/canvasDocuments.ts`

- [ ] **Step 1: Write failing IPC mapping tests**

Require exact command arguments for checkpoint, enqueue, placement, canvas
round retry, restore, branch, compare, and list operations. For enqueue assert
content, frame PNG, expected head/hash, and client request ID are forwarded
without source-image-path substitution. For placement assert canonical
post-insertion content and `preview_png_base64` are inside the snake_case
`request` envelope. For retry assert both `round_id` and a new
`client_request_id` are sent.

- [ ] **Step 2: Run API tests and verify RED**

```bash
npx vitest run src/lib/api.test.ts
```

Expected: FAIL because revision/round wrappers are missing.

- [ ] **Step 3: Add exact types, wrappers, and query keys**

Mirror Rust revision/round/comparison/request types. Add
`retryCanvasGenerationRound` alongside enqueue/placement wrappers. Add query
keys under `canvas-documents`, `canvas-revisions`, and `canvas-rounds`.
Mutations must invalidate the affected document, revision, round, generation,
and job keys after success.

- [ ] **Step 4: Run API tests GREEN**

```bash
npx vitest run src/lib/api.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 7**

```bash
git add src/types/index.ts src/lib/api.ts src/lib/api.test.ts src/lib/queries/canvasDocuments.ts
git commit -m "feat: add canvas revision client"
```

## Task 8: Canvas Controllers And Generation Round Hook

**Files:**
- Create: `src/hooks/useCanvasDocumentController.ts`
- Create: `src/hooks/useCanvasSelectionController.ts`
- Create: `src/hooks/useCanvasGenerationRound.ts`
- Create: `src/hooks/useCanvasDocumentController.test.tsx`
- Create: `src/hooks/useCanvasSelectionController.test.tsx`
- Create: `src/hooks/useCanvasGenerationRound.test.tsx`
- Modify: `src/pages/CanvasPage.tsx`

- [ ] **Step 1: Write failing hook behavior tests**

Test autosave hash updates, historical read-only mode, selection commands,
enqueue with exported frame, terminal candidate loading, direct placement with
a preview exported from the exact post-insertion content, canvas-round retry,
and head-conflict choice. Use real reducers/helpers and mock only
Tauri/network boundaries.

- [ ] **Step 2: Run hook tests and verify RED**

```bash
npx vitest run src/hooks/useCanvasDocumentController.test.tsx src/hooks/useCanvasSelectionController.test.tsx src/hooks/useCanvasGenerationRound.test.tsx
```

Expected: FAIL because hooks are missing.

- [ ] **Step 3: Extract page responsibilities**

Move document/history/autosave/head/hash behavior into the document controller,
existing B1 selection/clipboard commands into the selection controller, and
round/job/candidate behavior into the generation-round hook. Before placement,
build the generated image object with provenance, call `exportCanvasFrame` on
that exact next content, and submit both content and preview in one placement
request. Retry uses `retryCanvasGenerationRound`, never the generic job retry.
Keep `CanvasPage` as the composition root. Historical preview must expose
`readOnly=true` and never write through autosave.

- [ ] **Step 4: Run hook and CanvasPage tests GREEN**

```bash
npx vitest run src/hooks/useCanvasDocumentController.test.tsx src/hooks/useCanvasSelectionController.test.tsx src/hooks/useCanvasGenerationRound.test.tsx src/pages/CanvasPage.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 8**

```bash
git add src/hooks src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx
git commit -m "refactor: split canvas workspace controllers"
```

## Task 9: Visible Revision Board And Compare Experience

**Files:**
- Create: `src/components/canvas/CanvasVersionBoard.tsx`
- Create: `src/components/canvas/CanvasVersionBoard.test.tsx`
- Create: `src/components/canvas/CanvasRevisionCompareDialog.tsx`
- Create: `src/components/canvas/CanvasRevisionCompareDialog.test.tsx`
- Modify: `src/components/canvas/CanvasAssetSidebar.tsx`
- Modify: `src/components/canvas/CanvasStage.tsx`
- Modify: `src/pages/CanvasPage.tsx`

- [ ] **Step 1: Write failing revision UI tests**

Test current-head badge, parent indentation/connectors, historical read-only preview, exactly-two compare selection, restore-as-new, branch, corrupt revision disabled state, and manual checkpoint.

- [ ] **Step 2: Run component tests and verify RED**

```bash
npx vitest run src/components/canvas/CanvasVersionBoard.test.tsx src/components/canvas/CanvasRevisionCompareDialog.test.tsx src/pages/CanvasPage.test.tsx
```

Expected: FAIL because revision components are missing.

- [ ] **Step 3: Implement visible version board**

Render a compact vertical tree under the selected document list with preview, revision number, reason, timestamp, parent connector, branch indentation, and head badge. Clicking a historical revision loads its immutable preview/content read-only. Expose checkpoint, restore, branch, and compare actions with correct disabled states.

- [ ] **Step 4: Implement side-by-side comparison**

Require exactly two revisions, show previews and metadata (prompt, reason, time, model, parent), and omit pixel/object diff algorithms.

- [ ] **Step 5: Run revision UI tests GREEN**

```bash
npx vitest run src/components/canvas/CanvasVersionBoard.test.tsx src/components/canvas/CanvasRevisionCompareDialog.test.tsx src/pages/CanvasPage.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Commit Task 9**

```bash
git add src/components/canvas src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx
git commit -m "feat: add canvas revision board"
```

## Task 10: Canvas Results, Local Jobs, And Gallery Provenance

**Files:**
- Create: `src/components/canvas/CanvasGenerationResults.tsx`
- Create: `src/components/canvas/CanvasGenerationResults.test.tsx`
- Create: `src/components/canvas/CanvasJobList.tsx`
- Create: `src/components/canvas/CanvasJobList.test.tsx`
- Create: `src/components/jobs/GenerationJobActions.tsx`
- Create: `src/components/jobs/GenerationJobActions.test.tsx`
- Modify: `src/components/canvas/CanvasGenerationPanel.tsx`
- Modify: `src/pages/CanvasPage.tsx`
- Modify: `src/components/gallery/GenerationDetailPanel.tsx`
- Modify: `src/components/gallery/GenerationDetailPanel.test.tsx`

- [ ] **Step 1: Write failing result/provenance tests**

Test queued/running/failed/completed round states, candidate grid, one accepted
candidate, placement conflict choices, retryable terminal round retry, Gallery
navigation, canvas-local job filtering, source-aware cancel/retry actions, and
source-canvas navigation from Gallery.

- [ ] **Step 2: Run tests and verify RED**

```bash
npx vitest run src/components/canvas/CanvasGenerationResults.test.tsx src/components/canvas/CanvasJobList.test.tsx src/components/jobs/GenerationJobActions.test.tsx src/components/gallery/GenerationDetailPanel.test.tsx src/pages/CanvasPage.test.tsx
```

Expected: FAIL because result/job/provenance UI is missing.

- [ ] **Step 3: Implement inspector results and jobs**

Add Generation, Layers, and Tasks tabs. The Generation tab composes prompt
controls, current round status, candidate grid, Place on Canvas, Open in
Gallery, and Retry for retryable failed/interrupted rounds. Tasks filters
shared generation jobs by current document source reference. Implement
`GenerationJobActions` as the source-agnostic shared row: callers supply cancel
and retry callbacks, so `CanvasJobList` can route retry through the round-aware
command and the later C2 task center can reuse the same disabled-state UI.

- [ ] **Step 4: Add Gallery source navigation**

When generation request metadata/source references contain a canvas round, show Open Source Canvas. Navigate with document and revision identifiers so `CanvasPage` enters the exact historical preview.

- [ ] **Step 5: Run result/provenance tests GREEN**

```bash
npx vitest run src/components/canvas/CanvasGenerationResults.test.tsx src/components/canvas/CanvasJobList.test.tsx src/components/jobs/GenerationJobActions.test.tsx src/components/gallery/GenerationDetailPanel.test.tsx src/pages/CanvasPage.test.tsx
```

Expected: PASS.

- [ ] **Step 6: Commit Task 10**

```bash
git add src/components/canvas src/components/jobs/GenerationJobActions.tsx src/components/jobs/GenerationJobActions.test.tsx src/components/gallery src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx
git commit -m "feat: close canvas generation loop"
```

## Task 11: Locales, Failure Injection, And Full Verification

**Files:**
- Modify: `src/locales/*.json`
- Modify: `src/i18n.test.ts`
- Modify production files only when a new failing test proves a gap.

- [ ] **Step 1: Add all locale keys and verify parity**

Add revision, round, checkpoint, restore, branch, compare, candidate, conflict, corruption, read-only, and canvas task labels to all eight locales.

Run:

```bash
npx vitest run src/i18n.test.ts
```

Expected: PASS only when key sets are identical.

- [ ] **Step 2: Add end-to-end fake-engine command tests**

Cover canvas enqueue through queue completion through candidate placement,
invalid result-preview rejection, failed-round retry creating a linked child
round/job, concurrent edit with latest placement, concurrent edit with branch
placement, restart in queued/running/completed states, corrupt revision, DB
failure after final file move, and orphan cleanup. Run them RED before changing
production behavior.

- [ ] **Step 3: Fix only behavior exposed by failing integration tests**

Preserve exactly-one candidate acceptance per round, immutable revisions, secret-free jobs, and no automatic ambiguous replay.

- [ ] **Step 4: Run targeted suites GREEN**

```bash
npx vitest run src/lib/canvas src/hooks src/components/canvas src/pages/CanvasPage.test.tsx src/components/gallery/GenerationDetailPanel.test.tsx src/i18n.test.ts
cd src-tauri && cargo test --lib canvas_
```

Expected: PASS.

- [ ] **Step 5: Run full verification**

```bash
npm test
npm run build
cd src-tauri && cargo test --lib && cargo fmt --check
cd .. && git diff --check
```

Expected: all commands exit 0 and baseline warning count does not increase.

- [ ] **Step 6: Commit Task 11**

```bash
git add src src-tauri
git commit -m "test: verify canvas generation rounds"
```

## Self-Review

- Spec coverage: v2 provenance, v17 migration, immutable revisions, working hash, trusted export, queue round, idempotent placement, branch/restore, compare, visible version board, local jobs, and Gallery provenance all map to tasks.
- Scope: exactly one accepted candidate per round; no branch merge, pixel diff, local inference, masks, or node workflow.
- Type consistency: `current_revision_id`, `working_content_hash`, `source_revision_id`, `placement_operation_id`, and `client_request_id` remain stable.
- Dependency discipline: B1 and C1 gates precede every A implementation task.
- TDD: each production change is preceded by a focused RED test and followed by targeted GREEN verification.
