# Persistent Generation Job Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace long-running generation IPC calls with a durable, cancellable single-worker queue and expose consistent job state in normal generation and a global task center.

**Architecture:** Add a v16 SQLite migration, focused job repository and worker modules, and an execution context that lets the existing generation lifecycle complete a pre-created generation. New enqueue commands persist generation and job records atomically, then wake one managed worker; frontend callers observe structured job events rather than awaiting provider completion.

**Tech Stack:** Rust, Tauri 2, rusqlite, Tokio, serde, React 19, TypeScript, TanStack Query, Vitest, React Testing Library.

---

## File Structure

- Modify `src-tauri/src/db.rs`: v16 migration and migration assertions.
- Modify `src-tauri/src/models.rs`: serializable job, filter, enqueue, and event models.
- Modify `src-tauri/src/api_gateway.rs`: structured single-attempt provider errors, retry metadata, and fake-engine seams.
- Create `src-tauri/src/generation_jobs.rs`: job persistence, state transitions, idempotent enqueue, list/get/cancel/retry operations.
- Create `src-tauri/src/generation_job_worker.rs`: wake loop, atomic claim, cancellation registry, startup reconciliation, and lifecycle execution.
- Modify `src-tauri/src/generation_lifecycle.rs`: execute a pre-created generation with snapshotted public configuration and structured terminal outcome.
- Modify `src-tauri/src/commands/generation.rs`: enqueue/list/get/cancel/retry commands and compatibility adapters.
- Modify `src-tauri/src/commands/mod.rs`: expose queue command module if a separate command file is used.
- Modify `src-tauri/src/lib.rs`: manage queue state, register commands, reconcile/start worker.
- Modify `src-tauri/src/error.rs`: stable job error classification helpers without exposing secrets.
- Modify `src/types/index.ts`: job, event, filter, and enqueue response types.
- Modify `src/lib/api.ts`: queue commands and `generation-job:updated` subscription.
- Modify `src/lib/api.test.ts`: IPC argument mapping tests.
- Create `src/lib/queries/generationJobs.ts`: list/get/cancel/retry hooks and event invalidation helper.
- Modify `src/lib/generationMessages.ts`: queued/cancelled/interrupted message mapping.
- Modify `src/lib/generationMessages.test.ts`: job-state message behavior.
- Modify `src/pages/GeneratePage.tsx`: enqueue submission and job-event refresh.
- Modify `src/pages/GeneratePage.test.tsx`: queued submission, navigation-safe completion, cancellation, and retry.
- Modify `src/components/generate/GenerationComposer.tsx`: disable only during enqueue IPC, not job execution.
- Create `src/components/jobs/GenerationTaskCenter.tsx`: global job list and actions.
- Create `src/components/jobs/GenerationTaskCenter.test.tsx`: user-visible task behavior.
- Modify `src/components/layout/AppLayout.tsx`: task badge and drawer host.
- Modify `src/components/layout/AppLayout.test.tsx`: badge and drawer behavior.
- Modify `src/locales/*.json`: queue/task/error labels.
- Modify `src/i18n.test.ts`: retain identical key coverage.

## Task 1: Generation Job Migration And Shared Models

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: Add failing v16 migration assertions**

Extend the existing database migration test module with assertions equivalent to:

```rust
#[test]
fn fresh_database_migrations_create_generation_jobs_table() {
    let db_path = test_db_path("astro-studio-generation-jobs-migration-test");
    let database = Database::open(&db_path).expect("open test db");
    database.run_migrations().expect("run migrations");

    let conn = database.conn.lock().expect("lock db");
    assert!(table_has_column(&conn, "generation_jobs", "client_request_id"));
    assert!(table_has_column(&conn, "generation_jobs", "generation_id"));
    assert!(table_has_column(&conn, "generation_jobs", "status"));
    assert!(table_has_column(&conn, "generation_jobs", "auto_attempt"));
    assert!(table_has_column(&conn, "generation_jobs", "cancel_requested_at"));
    assert!(migration_version_exists(&conn, 16));
}
```

- [ ] **Step 2: Run the migration test and verify RED**

Run:

```bash
cd src-tauri && cargo test --lib fresh_database_migrations_create_generation_jobs_table
```

Expected: FAIL because `generation_jobs` does not exist.

- [ ] **Step 3: Add the v16 schema**

Add an `apply_migration` call after v15 with this logical schema:

```sql
CREATE TABLE IF NOT EXISTS generation_jobs (
    id TEXT PRIMARY KEY,
    client_request_id TEXT NOT NULL UNIQUE,
    generation_id TEXT NOT NULL UNIQUE REFERENCES generations(id) ON DELETE CASCADE,
    parent_job_id TEXT REFERENCES generation_jobs(id) ON DELETE SET NULL,
    source_kind TEXT NOT NULL,
    source_ref_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL,
    request_json TEXT NOT NULL,
    provider_kind TEXT NOT NULL,
    provider_profile_id TEXT NOT NULL,
    endpoint_snapshot TEXT NOT NULL,
    chain_attempt INTEGER NOT NULL DEFAULT 1,
    auto_attempt INTEGER NOT NULL DEFAULT 0,
    max_auto_attempts INTEGER NOT NULL DEFAULT 2,
    queued_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    cancel_requested_at TEXT,
    last_heartbeat_at TEXT,
    error_code TEXT,
    error_message TEXT,
    retryable INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_status_queued
    ON generation_jobs(status, queued_at);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_parent
    ON generation_jobs(parent_job_id);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_source
    ON generation_jobs(source_kind);
```

- [ ] **Step 4: Define shared Rust models**

Add models with snake_case serde output:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationJobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationJob {
    pub id: String,
    pub client_request_id: String,
    pub generation_id: String,
    pub parent_job_id: Option<String>,
    pub source_kind: String,
    pub source_ref: serde_json::Value,
    pub status: GenerationJobStatus,
    pub request: serde_json::Value,
    pub provider_kind: String,
    pub provider_profile_id: String,
    pub endpoint_snapshot: String,
    pub chain_attempt: i32,
    pub auto_attempt: i32,
    pub max_auto_attempts: i32,
    pub queued_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub cancel_requested_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub retryable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueGenerationResult {
    pub job_id: String,
    pub generation_id: String,
    pub conversation_id: String,
    pub status: GenerationJobStatus,
}
```

- [ ] **Step 5: Run migration and model tests**

Run:

```bash
cd src-tauri && cargo test --lib fresh_database_migrations_create_generation_jobs_table
```

Expected: PASS.

- [ ] **Step 6: Commit Task 1**

```bash
git add src-tauri/src/db.rs src-tauri/src/models.rs
git commit -m "feat: add generation job schema"
```

## Task 2: Job Repository And State Machine

**Files:**
- Create: `src-tauri/src/generation_jobs.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing repository tests**

Create tests beside the repository covering:

```rust
#[test]
fn job_transition_matrix_rejects_terminal_reactivation() {
    assert!(can_transition(GenerationJobStatus::Queued, GenerationJobStatus::Running));
    assert!(can_transition(GenerationJobStatus::Running, GenerationJobStatus::Completed));
    assert!(!can_transition(GenerationJobStatus::Completed, GenerationJobStatus::Queued));
    assert!(!can_transition(GenerationJobStatus::Failed, GenerationJobStatus::Running));
}

#[test]
fn repeated_client_request_returns_the_existing_job() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1");
    let second = fixture.enqueue("request-1");
    assert_eq!(first.id, second.id);
    assert_eq!(fixture.job_count(), 1);
}

#[test]
fn retry_creates_a_child_job_without_mutating_the_parent() {
    let fixture = JobFixture::new();
    let failed = fixture.failed_retryable_job();
    let retry = fixture.retry(&failed.id, "retry-request-1");
    assert_eq!(retry.parent_job_id.as_deref(), Some(failed.id.as_str()));
    assert_eq!(retry.chain_attempt, failed.chain_attempt + 1);
    assert_eq!(retry.auto_attempt, 0);
    assert_eq!(fixture.get(&failed.id).status, GenerationJobStatus::Failed);
}

#[test]
fn transaction_composable_enqueue_rolls_back_with_outer_scope() {
    let fixture = JobFixture::new();
    let mut conn = fixture.open_connection();
    let tx = conn.transaction().unwrap();
    insert_job_in_transaction(&tx, &fixture.prepared_job("request-1")).unwrap();
    tx.rollback().unwrap();

    assert_eq!(fixture.job_count(), 0);
    assert_eq!(fixture.generation_count(), 0);
}
```

The fixture must use a real temporary SQLite database and v16 migration, not a mocked repository.

- [ ] **Step 2: Run repository tests and verify RED**

Run:

```bash
cd src-tauri && cargo test --lib generation_jobs::tests
```

Expected: FAIL because the repository does not exist.

- [ ] **Step 3: Implement focused repository operations**

Create `generation_jobs.rs` with:

```rust
pub(crate) fn can_transition(from: GenerationJobStatus, to: GenerationJobStatus) -> bool;
pub(crate) fn enqueue_job(
    conn: &mut rusqlite::Connection,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError>;
pub(crate) fn insert_job_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    request: &PreparedGenerationJob,
) -> Result<EnqueueGenerationResult, AppError>;
pub(crate) fn get_job(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<GenerationJob, AppError>;
pub(crate) fn list_jobs(
    conn: &rusqlite::Connection,
    filter: &GenerationJobFilter,
) -> Result<GenerationJobPage, AppError>;
pub(crate) fn claim_next_job(
    conn: &rusqlite::Connection,
) -> Result<Option<GenerationJob>, AppError>;
pub(crate) fn request_cancel(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<GenerationJob, AppError>;
pub(crate) fn finish_job(
    conn: &rusqlite::Connection,
    update: &GenerationJobTerminalUpdate,
) -> Result<GenerationJob, AppError>;
pub(crate) fn create_retry_job(
    conn: &mut rusqlite::Connection,
    parent_id: &str,
    client_request_id: &str,
) -> Result<EnqueueGenerationResult, AppError>;
pub(crate) fn insert_retry_job_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    parent_id: &str,
    client_request_id: &str,
    source_ref_override: Option<&serde_json::Value>,
) -> Result<EnqueueGenerationResult, AppError>;
```

Every transition must use an expected prior status in SQL. `enqueue_job` and
`create_retry_job` open and commit their own transaction, delegating the actual
generation/job inserts to the matching `*_in_transaction` primitive. The
canvas milestone will call those primitives inside a wider transaction that
also inserts a revision and round; do not nest transactions or duplicate queue
SQL. A repeated `client_request_id` returns the existing row. No repository
function may serialize an API key. Generic retry accepts only `generate` and
`edit` source kinds; future source kinds must use a source-aware transaction so
they cannot create orphan jobs. The public wrapper passes no source-reference
override; a future source-aware transaction may pass a new reference while
preserving the parent request/profile snapshot.

- [ ] **Step 4: Register the module and run tests GREEN**

Add `mod generation_jobs;` in `lib.rs`, then run:

```bash
cd src-tauri && cargo test --lib generation_jobs::tests
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/generation_jobs.rs src-tauri/src/lib.rs
git commit -m "feat: add generation job repository"
```

## Task 3: Pre-Created Generation Execution Context

**Files:**
- Modify: `src-tauri/src/api_gateway.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/generation_lifecycle.rs`
- Modify: `src-tauri/src/commands/generation.rs`
- Test: `src-tauri/src/api_gateway.rs`
- Test: `src-tauri/src/generation_lifecycle.rs`

- [ ] **Step 1: Write failing lifecycle-context tests**

Add tests proving that execution uses supplied identity/configuration and does not insert another generation:

```rust
#[test]
fn execution_context_preserves_precreated_identity() {
    let context = fixture_execution_context("generation-1", "conversation-1");
    assert_eq!(context.generation_id, "generation-1");
    assert_eq!(context.conversation_id, "conversation-1");
    assert_eq!(context.endpoint_url, "https://example.test/images/generations");
}

#[test]
fn job_request_snapshot_round_trips_without_api_key() {
    let snapshot = fixture_job_snapshot();
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("secret-key"));
    assert_eq!(serde_json::from_str::<GenerationJobRequest>(&json).unwrap(), snapshot);
}

#[test]
fn provider_errors_preserve_safe_retry_and_ambiguity() {
    let rate_limited = EngineCallError::from_http_status(429, Some(3));
    assert_eq!(rate_limited.code, "rate_limited");
    assert_eq!(rate_limited.retry_after_seconds, Some(3));
    assert!(rate_limited.safe_to_retry);

    let unknown = EngineCallError::provider_outcome_unknown("connection reset");
    assert_eq!(unknown.code, "provider_outcome_unknown");
    assert!(!unknown.safe_to_retry);
    assert!(unknown.outcome_ambiguous);
}
```

- [ ] **Step 2: Run lifecycle tests and verify RED**

```bash
cd src-tauri && cargo test --lib generation_lifecycle::tests
```

Expected: FAIL because execution context, serializable request snapshots, and
structured engine errors are missing.

- [ ] **Step 3: Split preparation from execution**

Introduce:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct GenerationJobOptions {
    pub size: String,
    pub quality: String,
    pub background: String,
    pub output_format: String,
    pub output_compression: u8,
    pub moderation: String,
    pub input_fidelity: String,
    pub stream: bool,
    pub partial_images: u8,
    pub image_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct GenerationJobRequest {
    pub kind: GenerationLifecycleKind,
    pub prompt: String,
    pub model: String,
    pub source_image_paths: Vec<String>,
    pub options: GenerationJobOptions,
    pub conversation_id: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EngineCallError {
    pub code: String,
    pub sanitized_message: String,
    pub retry_after_seconds: Option<u64>,
    pub safe_to_retry: bool,
    pub outcome_ambiguous: bool,
}

pub(crate) struct GenerationExecutionContext {
    pub generation_id: String,
    pub job_id: String,
    pub conversation_id: String,
    pub model: String,
    pub endpoint_url: String,
    pub provider_profile_id: String,
}

pub(crate) async fn execute_generation_lifecycle(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &dyn ImageEngine,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
) -> Result<GenerateResult, AppError>;
```

Add explicit conversion between `GenerationJobOptions` and
`GptImageRequestOptions`; do not make the runtime options struct the persisted
DTO. Change `ImageEngine::generate` and `ImageEngine::edit` to return
`EngineCallError`. Each engine call performs one provider attempt and exposes
429 `Retry-After`, explicitly retryable 5xx, known connection-before-response,
and ambiguous post-send failures without logging or returning secrets. Move
bounded backoff/jitter ownership to the job worker so every automatic attempt
can be persisted. Remove the gateway-owned `max_retries` loop; the synchronous
compatibility adapter performs one classified attempt while first-party callers
move to the queue.

Move provider invocation, response recovery, image saving, completion, and
failure emission into `execute_generation_lifecycle`. It must update the
supplied generation ID and must not create a generation row. Retain
`run_generation_lifecycle` temporarily as a compatibility adapter that
prepares and executes synchronously through the same internal functions.

- [ ] **Step 4: Run lifecycle and command tests GREEN**

```bash
cd src-tauri && cargo test --lib generation_lifecycle
cargo test --lib api_gateway
cargo test --lib commands::generation
```

Expected: PASS with no duplicate generation rows.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/api_gateway.rs src-tauri/src/models.rs src-tauri/src/generation_lifecycle.rs src-tauri/src/commands/generation.rs
git commit -m "refactor: execute precreated generations"
```

## Task 4: Managed Queue Worker And Startup Reconciliation

**Files:**
- Create: `src-tauri/src/generation_job_worker.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/generation_jobs.rs`

- [ ] **Step 1: Write failing worker-policy tests**

Cover claim ordering, cancellation, and startup decisions with pure policies plus a real repository:

```rust
#[test]
fn startup_reconciliation_keeps_queued_and_interrupts_unknown_running() {
    assert_eq!(startup_action(GenerationJobStatus::Queued, false), StartupAction::KeepQueued);
    assert_eq!(startup_action(GenerationJobStatus::Running, false), StartupAction::Interrupt);
    assert_eq!(startup_action(GenerationJobStatus::Running, true), StartupAction::RecoverResponse);
}

#[tokio::test]
async fn worker_claims_only_one_fifo_job() {
    let fixture = WorkerFixture::with_jobs(["first", "second"]);
    fixture.run_one().await;
    assert_eq!(fixture.status("first"), GenerationJobStatus::Completed);
    assert_eq!(fixture.status("second"), GenerationJobStatus::Queued);
}

#[test]
fn automatic_retry_policy_retries_only_known_safe_failures() {
    let limited = EngineCallError::from_http_status(429, Some(3));
    assert_eq!(automatic_retry_delay(&limited, 0, 2, 1), Some(Duration::from_secs(3)));

    let unknown = EngineCallError::provider_outcome_unknown("connection reset");
    assert_eq!(automatic_retry_delay(&unknown, 0, 2, 1), None);
}
```

- [ ] **Step 2: Run worker tests and verify RED**

```bash
cd src-tauri && cargo test --lib generation_job_worker::tests
```

Expected: FAIL because the worker module is missing.

- [ ] **Step 3: Implement queue state and worker loop**

Create a managed state with an in-process wake signal and cooperative cancellation registry:

```rust
#[derive(Clone)]
pub(crate) struct GenerationJobQueue {
    wake: std::sync::Arc<tokio::sync::Notify>,
    cancellations: std::sync::Arc<tokio::sync::Mutex<
        std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>,
    >>,
}

impl GenerationJobQueue {
    pub(crate) fn wake(&self) { self.wake.notify_one(); }
    pub(crate) async fn cancel(&self, job_id: &str) -> bool;
}
```

The worker must:

1. Reconcile startup states before accepting work.
2. Claim one FIFO queued job with an atomic queued-to-running update.
3. Resolve the profile secret by stored profile ID without changing endpoint/model snapshots.
4. Use `tokio::select!` between execution and the watch cancellation signal so dropping the provider future cancels the HTTP request.
5. For `safe_to_retry` errors only, persist the incremented `auto_attempt`, wait with bounded exponential backoff plus injected/testable jitter while still selecting on cancellation, honor a longer `Retry-After`, and call the engine again on the same job.
6. Convert ambiguous outcomes to `interrupted` with `provider_outcome_unknown`; never replay them automatically.
7. Persist terminal job status and emit one structured event.
8. Wait on `Notify` with a bounded fallback poll when no work exists.

- [ ] **Step 4: Start the worker from Tauri setup**

Manage `GenerationJobQueue`, reconcile before spawning, and use `tauri::async_runtime::spawn` after the application handle and managed states are available. Do not block setup on the long-running loop.

- [ ] **Step 5: Run worker and lifecycle tests GREEN**

```bash
cd src-tauri && cargo test --lib generation_job_worker
cargo test --lib generation_jobs
cargo test --lib generation_lifecycle
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

```bash
git add src-tauri/src/generation_job_worker.rs src-tauri/src/generation_jobs.rs src-tauri/src/lib.rs
git commit -m "feat: run persistent generation worker"
```

## Task 5: Queue Commands And Structured Events

**Files:**
- Modify: `src-tauri/src/commands/generation.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/error.rs`

- [ ] **Step 1: Write failing command tests**

Add command-layer tests for:

```rust
#[test]
fn enqueue_snapshot_excludes_api_key_and_keeps_profile_identity() {
    let fixture = JobCommandFixture::new();
    let result = fixture.enqueue_generation("request-1", "profile-1");
    let job = fixture.get_job(&result.job_id);

    assert_eq!(job.provider_profile_id, "profile-1");
    assert!(!serde_json::to_string(&job.request).unwrap().contains("secret-key"));
}

#[test]
fn cancel_queued_job_is_immediately_terminal() {
    let fixture = JobCommandFixture::new();
    let queued = fixture.queued_job("request-1");
    let cancelled = fixture.cancel(&queued.id).unwrap();

    assert_eq!(cancelled.status, GenerationJobStatus::Cancelled);
    assert!(cancelled.cancel_requested_at.is_some());
    assert!(cancelled.finished_at.is_some());
}

#[test]
fn retry_rejects_non_retryable_completed_job() {
    let fixture = JobCommandFixture::new();
    let completed = fixture.completed_job("request-1");
    let error = fixture.retry(&completed.id, "retry-request-1").unwrap_err();

    assert_eq!(error.stable_code(), "generation_job_not_retryable");
    assert_eq!(fixture.job_count(), 1);
}
```

Implement `JobCommandFixture` inside the command test module with a temporary
v16 SQLite database, a profile containing the literal API key `secret-key`, and
helpers that invoke the same preparation/repository functions as the Tauri
commands. Do not bypass the real transaction layer.

- [ ] **Step 2: Run command tests and verify RED**

```bash
cd src-tauri && cargo test --lib commands::generation::job_tests
```

Expected: FAIL because queue commands do not exist.

- [ ] **Step 3: Add Tauri commands**

Implement and register:

```rust
#[tauri::command]
pub async fn enqueue_generation(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    request: EnqueueGenerationRequest,
) -> Result<EnqueueGenerationResult, AppError>;

#[tauri::command]
pub async fn enqueue_edit(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    selected_images: tauri::State<'_, SelectedImageRegistry>,
    request: EnqueueEditRequest,
) -> Result<EnqueueGenerationResult, AppError>;

#[tauri::command]
pub fn list_generation_jobs(
    db: tauri::State<'_, Database>,
    filters: GenerationJobFilter,
) -> Result<GenerationJobPage, AppError>;

#[tauri::command]
pub fn get_generation_job(
    db: tauri::State<'_, Database>,
    job_id: String,
) -> Result<GenerationJob, AppError>;

#[tauri::command]
pub async fn cancel_generation_job(
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    job_id: String,
) -> Result<GenerationJob, AppError>;

#[tauri::command]
pub fn retry_generation_job(
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    job_id: String,
    client_request_id: String,
) -> Result<EnqueueGenerationResult, AppError>;
```

`EnqueueGenerationRequest` mirrors all current `generate_image` generation
parameters plus `client_request_id`. `EnqueueEditRequest` mirrors all current
`edit_image` parameters plus `client_request_id`. `GenerationJobFilter`
contains optional `statuses`, `source_kind`, `source_ref_id`, `limit`, and
`cursor`; `GenerationJobPage` contains `items` and `next_cursor`.

Provider configuration errors after syntactic acceptance must create a visible failed job. Validation errors that make the request unserializable may reject before enqueue.

- [ ] **Step 4: Emit a single typed job event**

Use `generation-job:updated` and a serialized `GenerationJobEvent` containing job/generation/source/status/stage/queue-position/attempts/cancel/error/retry/timestamps. Ensure tests serialize the complete shape and assert no secret fields.

- [ ] **Step 5: Run command tests GREEN**

```bash
cd src-tauri && cargo test --lib commands::generation::job_tests
cargo test --lib generation_jobs
```

Expected: PASS.

- [ ] **Step 6: Commit Task 5**

```bash
git add src-tauri/src/commands/generation.rs src-tauri/src/lib.rs src-tauri/src/models.rs src-tauri/src/error.rs
git commit -m "feat: expose generation queue commands"
```

## Task 6: Frontend Queue API, Queries, And Message States

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/api.test.ts`
- Create: `src/lib/queries/generationJobs.ts`
- Modify: `src/lib/generationMessages.ts`
- Modify: `src/lib/generationMessages.test.ts`

- [ ] **Step 1: Write failing TypeScript API and state tests**

Add tests that require:

```ts
expect(invoke).toHaveBeenCalledWith("enqueue_generation", {
  request: expect.objectContaining({
    client_request_id: expect.any(String),
    prompt: "test prompt",
  }),
});

expect(generationStatusToMessageStatus("queued")).toBe("processing");
expect(generationStatusToMessageStatus("cancelled")).toBe("failed");
expect(generationStatusToMessageStatus("interrupted")).toBe("failed");
```

- [ ] **Step 2: Run tests and verify RED**

```bash
npx vitest run src/lib/api.test.ts src/lib/generationMessages.test.ts
```

Expected: FAIL because job APIs/types do not exist.

- [ ] **Step 3: Add exact frontend types and wrappers**

Define status and job types mirroring Rust, add `enqueueGeneration`, `enqueueEdit`, list/get/cancel/retry wrappers, and add:

```ts
export function onGenerationJobUpdated(
  handler: (event: GenerationJobEvent) => void,
) {
  return onGenerationEvent("generation-job:updated", handler);
}
```

Create TanStack Query hooks with query keys rooted at `generation-jobs`, and invalidate both job and generation/conversation queries on events and mutations.

- [ ] **Step 4: Run API and state tests GREEN**

```bash
npx vitest run src/lib/api.test.ts src/lib/generationMessages.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit Task 6**

```bash
git add src/types/index.ts src/lib/api.ts src/lib/api.test.ts src/lib/queries/generationJobs.ts src/lib/generationMessages.ts src/lib/generationMessages.test.ts
git commit -m "feat: add generation queue client"
```

## Task 7: Migrate Generate Page To Enqueue Semantics

**Files:**
- Modify: `src/pages/GeneratePage.tsx`
- Modify: `src/pages/GeneratePage.test.tsx`
- Modify: `src/components/generate/GenerationComposer.tsx`

- [ ] **Step 1: Add failing page tests**

Cover immediate queued response, route navigation, and event refresh:

```tsx
it("renders a queued assistant message after enqueue returns", async () => {
  enqueueGeneration.mockResolvedValue({
    job_id: "job-1",
    generation_id: "generation-1",
    conversation_id: "conversation-1",
    status: "queued",
  });
  const user = userEvent.setup();
  renderGeneratePage();
  await user.type(screen.getByRole("textbox"), "first prompt");
  await user.click(screen.getByRole("button", { name: /send/i }));

  expect(await screen.findByText(/queued/i)).toBeInTheDocument();
  expect(generateImage).not.toHaveBeenCalled();
  await user.type(screen.getByRole("textbox"), "second prompt");
  expect(screen.getByRole("button", { name: /send/i })).toBeEnabled();
});

it("refreshes the active conversation for a matching terminal job event", async () => {
  const fixture = renderGeneratePageWithJobEvents();
  fixture.emitJobEvent(completedJobEvent({ conversation_id: "conversation-1" }));

  await waitFor(() => expect(fixture.refetchConversation).toHaveBeenCalledWith("conversation-1"));
});
```

Add `renderGeneratePage` and `renderGeneratePageWithJobEvents` to the existing
test fixture layer so these tests drive the real composer and subscription
effect while mocking only the API/event boundary.

- [ ] **Step 2: Run page tests and verify RED**

```bash
npx vitest run src/pages/GeneratePage.test.tsx
```

Expected: FAIL because the page still awaits synchronous generation.

- [ ] **Step 3: Replace synchronous submission**

Generate a `clientRequestId` before optimistic messages, call enqueue
generate/edit, replace the temporary generation ID with the persisted ID,
render queued/running states, and allow navigation immediately. Subscribe to
`generation-job:updated`; refresh only the affected active conversation and
shared job queries. Remove the existing lifecycle-wide `isGenerating` submit
lock: the composer may be disabled only while its enqueue IPC is pending, not
while any persisted job is queued or running.

Do not create a parallel local queue. SQLite is the source of truth.

- [ ] **Step 4: Run page tests GREEN**

```bash
npx vitest run src/pages/GeneratePage.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 7**

```bash
git add src/pages/GeneratePage.tsx src/pages/GeneratePage.test.tsx src/components/generate/GenerationComposer.tsx
git commit -m "feat: enqueue image generation requests"
```

## Task 8: C1 Queue Recovery, Security, And Verification

**Files:**
- Modify: `src-tauri/src/api_gateway.rs` when injected provider failures expose classification gaps.
- Modify: `src-tauri/src/generation_job_worker.rs` when retry/cancellation/recovery tests expose state gaps.
- Modify: `src-tauri/src/generation_jobs.rs` and `src-tauri/src/generation_lifecycle.rs` when durable transitions expose gaps.
- Modify frontend queue files only when the new integration tests prove a user-state gap.
- Test: Rust queue/lifecycle/recovery suites and frontend queue/GeneratePage suites.

- [ ] **Step 1: Add injected-failure integration tests**

Use a fake engine and temporary database/files to cover 429 with Retry-After,
explicitly retryable 5xx, pre-response network failure, ambiguous outcome,
queued cancel, cancellation during provider/download/save stages,
response-ready restart recovery, unknown running restart interruption,
image-save failure, and missing profile at execution. Assert every path reaches
a durable terminal/recoverable state and no event/snapshot contains a secret.

- [ ] **Step 2: Run integration tests and verify RED for uncovered behavior**

```bash
cd src-tauri && cargo test --lib generation_job
```

Expected: New tests expose any missing recovery/cancellation behavior.

- [ ] **Step 3: Implement only the missing behavior**

Fix production code according to each failing test. Do not widen auto-retry beyond the approved safe policy.

- [ ] **Step 4: Run targeted suites GREEN**

```bash
cd src-tauri && cargo test --lib generation_job
cd .. && npx vitest run src/lib/api.test.ts src/lib/generationMessages.test.ts src/pages/GeneratePage.test.tsx src/i18n.test.ts
```

Expected: PASS.

- [ ] **Step 5: Run full verification**

```bash
npm test
npm run build
cd src-tauri && cargo test --lib && cargo fmt --check
cd .. && git diff --check
```

Expected: all commands exit 0. Known baseline warnings must not increase.

- [ ] **Step 6: Commit Task 8**

```bash
git add src src-tauri
git commit -m "test: harden generation queue lifecycle"
```

## Milestone Gate Before C2

After Tasks 1-8 are accepted, execute all tasks in
`2026-07-10-canvas-generation-rounds.md`. Return to Task 9 only after the canvas
round/revision milestone is accepted. This preserves the approved delivery
order B1 -> C1 -> A -> C2.

## Task 9: C2 Global Task Center

**Files:**
- Create: `src/components/jobs/GenerationTaskCenter.tsx`
- Create: `src/components/jobs/GenerationTaskCenter.test.tsx`
- Modify: `src/components/layout/AppLayout.tsx`
- Modify: `src/components/layout/AppLayout.test.tsx`
- Modify: `src/locales/*.json`
- Modify: `src/i18n.test.ts`

- [ ] **Step 1: Write failing task-center tests**

Test real component behavior:

```tsx
it("shows active job count and permits valid cancellation", async () => {
  const user = userEvent.setup();
  render(<GenerationTaskCenter jobs={[queuedJob(), runningJob()]} />);
  expect(screen.getByText("2 active tasks")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Cancel queued task" }));
  expect(cancelGenerationJob).toHaveBeenCalledWith("job-queued");
});

it("offers retry only for retryable terminal jobs", () => {
  render(<GenerationTaskCenter jobs={[failedRetryableJob(), completedJob()]} />);
  expect(screen.getAllByRole("button", { name: "Retry" })).toHaveLength(1);
});

it("routes a canvas retry through its round-aware callback", async () => {
  const user = userEvent.setup();
  render(<GenerationTaskCenter jobs={[failedCanvasJob("round-1")]} />);
  await user.click(screen.getByRole("button", { name: "Retry" }));

  expect(retryCanvasGenerationRound).toHaveBeenCalledWith(
    "round-1",
    expect.any(String),
  );
  expect(retryGenerationJob).not.toHaveBeenCalled();
});
```

- [ ] **Step 2: Run component/layout/i18n tests and verify RED**

```bash
npx vitest run src/components/jobs/GenerationTaskCenter.test.tsx src/components/layout/AppLayout.test.tsx src/i18n.test.ts
```

Expected: FAIL because the task center and labels are missing.

- [ ] **Step 3: Implement task center and layout host**

Render active and recent terminal jobs, source/provider/attempt/elapsed time,
cancellation state, sanitized error, retry, and source navigation. Add an
`AppLayout` button with an active-count badge and a drawer/dialog host. Reuse
the `GenerationJobActions` component delivered by the canvas milestone and the
shared query hooks; do not duplicate event subscriptions in every row. Route
normal generate/edit retries through `retryGenerationJob` and canvas retries
through `retryCanvasGenerationRound` using the round ID in `source_ref`.

- [ ] **Step 4: Add all locale keys**

Add identical `jobs` key shapes to all eight locales for task title, states,
stages, actions, attempts, retryability, elapsed time, empty state, and errors.
Use English fallback text only where a translation is not yet available, while
preserving exact key parity.

- [ ] **Step 5: Run component/layout/i18n tests GREEN**

```bash
npx vitest run src/components/jobs/GenerationTaskCenter.test.tsx src/components/layout/AppLayout.test.tsx src/i18n.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run full post-C2 verification**

```bash
npm test
npm run build
cd src-tauri && cargo test --lib && cargo fmt --check
cd .. && git diff --check
```

Expected: all commands exit 0 and baseline warning count does not increase.

- [ ] **Step 7: Commit Task 9**

```bash
git add src/components/jobs src/components/layout/AppLayout.tsx src/components/layout/AppLayout.test.tsx src/locales src/i18n.test.ts
git commit -m "feat: add generation task center"
```

## Self-Review

- Spec coverage: schema, idempotency, state machine, single worker, cancellation, retry, startup reconciliation, secret handling, events, GeneratePage migration, and C2 task center are mapped to tasks.
- Dependency order: Tasks 1-8 complete C1, then the canvas plan completes A, and Task 9 completes C2.
- Scope: no node editor, provider failover, cost model, priority scheduling, or configurable concurrency.
- Type consistency: `client_request_id`, `chain_attempt`, `auto_attempt`, `GenerationJobStatus`, and `generation-job:updated` are stable throughout.
- TDD: every production task begins with a failing focused test and records the RED/GREEN command.
