# Persistent Generation Job Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace long-running generation IPC calls with a durable, cancellable single-worker queue and expose consistent job state in normal generation and a global task center.

**Architecture:** Add a v16 SQLite migration, focused job repository and worker modules, and an execution context that lets the existing generation lifecycle complete a pre-created generation. New enqueue commands persist generation and job records atomically, then wake one managed worker; frontend callers observe structured job events rather than awaiting provider completion.

**Tech Stack:** Rust, Tauri 2, rusqlite, Tokio, serde, React 19, TypeScript, TanStack Query, Vitest, React Testing Library.

---

## File Structure

- Modify `src-tauri/src/db.rs`: v16 job migration plus v17 stage,
  recovery-artifact metadata, and fenced worker-lease migration with invariant
  assertions.
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

Do not stop at name existence. Against the migrated real database, insert the
minimum valid generation/job fixtures and prove both UNIQUE constraints reject
duplicates, missing generation/parent references are rejected, deleting a
parent job sets a child's `parent_job_id` to NULL, deleting a generation
cascades its job, and `PRAGMA index_info` reports the exact planned columns for
all three indexes. These assertions prevent a broken queue invariant from
passing merely because a column or index name exists.

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
    pub retryable: bool,
    pub cancel_requested_at: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub queued_at: String,
    pub finished_at: Option<String>,
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
- Create: `src-tauri/src/generation_jobs_tests.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/error.rs`

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
    let tx = begin_generation_job_write_transaction(&mut conn).unwrap();
    insert_job_in_transaction(&tx, &fixture.prepared_job("request-1")).unwrap();
    tx.rollback().unwrap();

    assert_eq!(fixture.job_count(), 0);
    assert_eq!(fixture.generation_count(), 0);
}
```

Also cover two jobs with the same second-level `queued_at`: claim/list order is
stable by `queued_at ASC, rowid ASC`. Verify an idempotent
`client_request_id` lookup occurs before any conversation, generation, log, or
recovery side effect, not merely before the second job insert. Rollback must
also undo a newly created or updated conversation.

Use two WAL connections to hold the first identical enqueue/retry uncommitted
while the second root call starts. Root writers must enter through a shared
bounded-wait `BEGIN IMMEDIATE` helper, then repeat the client-ID lookup inside
that serialized transaction; after the first commit the second returns the
same logical job rather than `SQLITE_BUSY` or a duplicate. Any caller composing
`insert_job_in_transaction` or `insert_retry_job_in_transaction` into a larger
transaction must start it with the same helper (or an equivalent already-held
immediate write transaction) before its first read.

Assert cross-table semantics: normal enqueue explicitly writes both job and
generation `queued` with one timestamp; claim moves both to `running`; queued
cancel moves both to `cancelled`; running cancel only records the request until
worker acknowledgement. Retry accepts only retryable failed/interrupted
generate/edit parents, creates a fresh generation plus child job, resets every
attempt/error/time field, and leaves the parent unchanged. Reusing a retry
client ID for a different parent is an idempotency conflict. Malformed or
unknown-version cursors fail with a stable sanitized error.

Also exercise the existing conversation move operation. The enqueue-time
project ID is an immutable historical execution/source snapshot, while a
conversation's current project membership is mutable. After moving a queued
conversation, get/list/enqueue-result/claim/cancel must remain healthy; after
moving a running conversation, its fenced terminal transition must still
commit. The shared validator verifies that the conversation ID still exists and
that request/metadata/source snapshots agree with each other, but it must not
require their historical project ID to equal the conversation's current
project. Idempotent replay remains keyed to the original requested identity and
the stored canonical snapshot after a move.

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
pub(crate) fn find_enqueue_result_by_client_request_id(
    conn: &rusqlite::Connection,
    client_request_id: &str,
) -> Result<Option<EnqueueGenerationResult>, AppError>;
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
    conn: &mut rusqlite::Connection,
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

Task 1 intentionally defined only the wire-level job/result models. Define the
missing contracts used above before implementing SQL:

- Internal `PreparedGenerationJob` owns the already-normalized, secret-free
  generation fields, requested conversation/project IDs, prompt, canonical
  request draft, profile and endpoint snapshot, source reference, IDs, and
  timestamps needed to insert both `generations` and `generation_jobs` in one
  transaction. It must not require a conversation resolved before that
  transaction.
- Typed `GenerationJobRequestKind`, `GenerationJobOptions`, and
  `GenerationJobRequest` are the only shape serialized into `request_json`.
  Repository code builds the finalized DTO from allowlisted prepared fields
  after conversation resolution; it never persists an arbitrary caller JSON
  object. Row decoding validates against the same DTO before exposing the wire
  `serde_json::Value`.
  Caller-controlled option fields are optional/presence-aware so model
  capability filtering survives persistence; only present fields serialize.
  `PreparedGenerationJob` carries this typed request-options value separately
  from the concrete normalized columns/metadata used by the gallery. Every
  present option must match its normalized concrete counterpart, and manual
  retry copies the exact optional shape.
  The typed request also stores original requested conversation/project IDs
  separately from resolved IDs. They participate in root-enqueue idempotency so
  reusing a client ID for another requested destination conflicts, while an
  ambiguous replay with the same original IDs returns the existing job.
- Typed/validated `GenerationJobSourceRef` permits only documented identity
  fields for generate/edit and the planned canvas document/round/revision IDs;
  unknown or sensitive keys are rejected so future source metadata cannot turn
  into an unbounded secret channel.
- Public `GenerationJobFilter` includes statuses, source kind/reference,
  `generation_id`, bounded limit, and cursor.
- Public `GenerationJobPage` contains items plus an opaque next cursor encoding
  the same `(queued_at, rowid)` order. The cursor is strictly parsed,
  versioned, and documented as a short-lived pagination token rather than a
  VACUUM-stable identifier.
- Internal `GenerationJobTerminalUpdate` carries expected prior status and the
  sanitized terminal fields. Add `finish_job_in_transaction` so lifecycle
  finalization can update generation/recovery/images and job state inside one
  outer transaction.

`PreparedGenerationJob` explicitly supplies every persisted generation field,
and repository code constructs valid source paths/request metadata JSON from
those fields rather than accepting free-form metadata. Normal jobs
insert generation/job as queued with the same `created_at`/`queued_at`.
Syntactically valid requests whose provider configuration cannot resolve insert
both records as failed in that one transaction with `finished_at`, sanitized
stable error fields, and `retryable=false`; do not insert queued and patch it in
a second transaction. When configuration fields are unavailable, use the
documented secret-free sentinels `unresolved` for provider/profile identity and
an empty endpoint snapshot. Workers never claim these terminal rows.
Initial failed rows use exactly one of two public snapshot shapes: both
provider/profile identities are `unresolved` with an empty endpoint, or both
identities are known nonempty values with a valid nonempty public endpoint.
Mixed known-identity/empty-endpoint states are invalid.
Reject invalid mixed states before writing: queued jobs cannot carry terminal
timestamps/errors or unresolved provider sentinels, while initial failed jobs
must carry a finished timestamp plus stable sanitized error fields and must not
create a requesting recovery row.

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

`claim_next_job` must select and update inside one transaction using
`ORDER BY queued_at ASC, rowid ASC`, then return only the row whose
`queued -> running` update succeeded, and update the linked generation to
running in that transaction. Queued cancellation likewise updates job and
generation atomically; running cancellation persists only
`cancel_requested_at`. No network, filesystem, event emission, or await may
occur while the database mutex/transaction is held.

`enqueue_job` may use `find_enqueue_result_by_client_request_id` as a cheap
preflight, but
`insert_job_in_transaction` must repeat that lookup before any write, then call
the existing `resolve_conversation_id_for_generation` through the transaction.
Query that resolved conversation's actual nondeleted project ID, then overwrite
conversation/project identity in the canonical request and every generate,
edit, or canvas source reference before
committing. This protects both ambiguous client retries and concurrent callers:
conversation create/update, generation, recovery, and job changes either all
commit once or all roll back. That canonical project is frozen at enqueue; a
later legitimate `move_conversation_to_project` changes navigation membership,
not the historical job snapshot, and is not persisted corruption.

`enqueue_job` and `create_retry_job` must use the shared bounded-wait
`BEGIN IMMEDIATE` helper. Their cheap outer preflight is only an optimization;
the lookup inside the immediate transaction is authoritative. Larger callers
that invoke the composable insert functions must acquire the same write intent
before their first repository read.

Add a sanitized job error variant/helper in `error.rs` with stable codes for at
least `generation_job_not_found`, `generation_job_invalid_transition`,
`generation_job_not_retryable`, `generation_job_idempotency_conflict`,
`generation_job_source_unsupported`,
`generation_job_corrupt_persisted_data`, `generation_job_invalid_snapshot`,
and `generation_job_corrupt_cursor`. Repository
row/status/JSON failures must not panic or expose SQL, request JSON, endpoint,
or profile secrets.

Treat the repository as a final secret boundary. Parse endpoint snapshots with
`reqwest::Url`, reject URL credentials and decoded sensitive query keys while
allowing public parameters such as `api-version`, and reject credential-like
tokens in persisted public strings. Terminal error text is selected from a
fixed sanitized message table keyed by stable error code; never persist a raw
provider/database message. Tests cover nested `apiKey`/`x-api-key`/
`authorization`, encoded query keys, URL userinfo, token-like text, and safe
ordinary prompts/endpoints.
Apply credential-token detection to every persisted public string, including
prompt/model/profile IDs, source paths, request IDs, and source-reference IDs.
Reject real bearer/prefix/JWT patterns with high-confidence shapes: `Bearer`
requires explicit authorization context, a known credential prefix, or an
opaque token shape/length, while Google keys require the long `AIza` key form.
Authorization Bearer parsing accepts the RFC 6750 b64token alphabet, including
`~+/=`; Google detection is case-sensitive `AIza` plus the real 35-character
suffix, so long `Aizawa-*` names remain ordinary text.
Do not reject ordinary prose such as "ring bearer standing", "Aizawa-kun", or
words beginning with "sk".

Before claim changes either row, load the linked generation plus requesting
recovery and compare conversation/project, prompt/model/kind, normalized
columns, source paths, canonical metadata, and queued status to the job request.
Any mismatch is corrupt persisted data and leaves both rows untouched. Queued
cancel deletes its never-used requesting recovery row in the same transaction.
Use the same linked-state validator for get, list, enqueue-result, and
idempotency projections so contradictory job/generation/recovery rows are never
returned as healthy simply because no claim occurs.

Manual retry permits only failed/interrupted rows with `retryable=true` and
source kind generate/edit. It copies canonical request, resolved conversation,
and public provider/endpoint snapshots into a new generation and child job,
increments `chain_attempt`, resets `auto_attempt`, status, errors, cancellation,
and timestamps, and never mutates the parent. An existing retry client ID is
idempotent only when it belongs to that same parent and logical retry.

- [ ] **Step 4: Register the module and run tests GREEN**

Add `mod generation_jobs;` in `lib.rs`, then run:

```bash
cd src-tauri && cargo test --lib generation_jobs::tests
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/generation_jobs.rs src-tauri/src/generation_jobs_tests.rs src-tauri/src/lib.rs src-tauri/src/models.rs src-tauri/src/error.rs
git commit -m "feat: add generation job repository"
```

## Task 3: Pre-Created Generation Execution Context

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/api_gateway.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/generation_lifecycle.rs`
- Modify: `src-tauri/src/generation_jobs.rs`
- Modify: `src-tauri/src/file_manager.rs`
- Modify: `src-tauri/src/commands/generation.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/api_gateway.rs`
- Test: `src-tauri/src/generation_lifecycle.rs`
- Test: `src-tauri/src/lib.rs`

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
    assert_eq!(rate_limited.retry_after, Some(RetryAfterHint::DelaySeconds(3)));
    assert!(rate_limited.safe_to_retry);

    let unknown = EngineCallError::provider_outcome_unknown("connection reset");
    assert_eq!(unknown.code, "provider_outcome_unknown");
    assert!(!unknown.safe_to_retry);
    assert!(unknown.outcome_ambiguous);
}
```

Add tests that one engine method invocation performs exactly one provider HTTP
submission (including a short provider response), that retryable provider
errors do not terminalize the generation/job before worker policy decides, and
that successful/final failures update generation and job in one transaction.
Prove execution never inserts another generation/conversation/recovery row and
does not resolve a different active provider profile.

Also prove: the synchronous compatibility adapter atomically creates and claims
a specific real job before executing even when an older FIFO job exists; a
successful provider response is atomically
written/verified and marked response-ready before decode/download; gateway code
does not mutate recovery rows; filesystem image staging happens outside the DB
mutex and cleans up on injected final-transaction failure; a short response
completes after one HTTP submission, persists only returned images, and records
requested versus actual count without worker replay.

Add fake `ResponseArtifactStore` and `ImageResponseDecoder` tests. The artifact
store observes a deterministic job/generation-derived path and no DB lock; the
decoder is invoked only after response-ready commit and performs zero provider
submissions. A promoted-file hook must observe `db.conn.try_lock()` success,
proving every write/rename happens before the DB mutex is acquired. Startup
tests prove the legacy cleanup in `lib.rs` preserves requesting/response-ready
rows linked to queued/running jobs and only touches generations for which no
`generation_jobs` row exists.

- [ ] **Step 2: Run lifecycle tests and verify RED**

```bash
cd src-tauri && cargo test --lib generation_lifecycle::tests
```

Expected: FAIL because execution context, serializable request snapshots, and
structured engine errors are missing.

- [ ] **Step 3: Split preparation from execution**

Reuse Task 2's typed `GenerationJobOptions` and `GenerationJobRequest`; do not
redeclare or deserialize a parallel free-form request. Introduce lifecycle
conversion and execution-only types:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GenerationLifecycleKind {
    Generate,
    Edit,
}

impl From<GenerationJobRequestKind> for GenerationLifecycleKind {
    fn from(kind: GenerationJobRequestKind) -> Self {
        match kind {
            GenerationJobRequestKind::Generate => Self::Generate,
            GenerationJobRequestKind::Edit => Self::Edit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EngineCallError {
    pub code: String,
    pub sanitized_message: String,
    pub retry_after: Option<RetryAfterHint>,
    pub safe_to_retry: bool,
    pub outcome_ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryAfterHint {
    DelaySeconds(u64),
    HttpDate(chrono::DateTime<chrono::Utc>),
    Invalid,
}

pub(crate) struct ProviderAttemptBody {
    pub body_text: String,
    pub requested_image_count: u8,
}

pub(crate) struct ProviderAttemptResponse {
    pub body_text: String,
    pub response_file: String,
    pub response_size: u64,
    pub response_sha256: String,
    pub requested_image_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GenerationExecutionError {
    Engine(EngineCallError),
    Local {
        code: String,
        sanitized_message: String,
        stage: String,
    },
}

pub(crate) struct GenerationExecutionContext {
    pub generation_id: String,
    pub job_id: String,
    pub conversation_id: String,
    pub provider_kind: String,
    pub model: String,
    pub endpoint_url: String,
    pub provider_profile_id: String,
}

pub(crate) struct GenerationExecutionSnapshot {
    pub context: GenerationExecutionContext,
    pub request: GenerationJobRequest,
    pub runtime_options: GptImageRequestOptions,
    pub created_at: String,
    pub output_format: String,
}

pub(crate) struct ProviderExecutionCredentials {
    api_key: String,
}

impl ProviderExecutionCredentials {
    pub(crate) fn new(api_key: String) -> Self;
    pub(crate) fn expose_for_engine(&self) -> &str;
}

#[async_trait::async_trait]
pub(crate) trait ResponseArtifactStore: Send + Sync {
    async fn persist_verified_response(
        &self,
        context: &GenerationExecutionContext,
        body: ProviderAttemptBody,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError>;
}

#[async_trait::async_trait]
pub(crate) trait ImageResponseDecoder: Send + Sync {
    async fn decode_and_download(
        &self,
        response: &ProviderAttemptResponse,
        cancellation: &CancellationProbe,
    ) -> Result<Vec<Vec<u8>>, GenerationExecutionError>;
}

pub(crate) async fn perform_provider_attempt(
    engine: &dyn ImageEngine,
    artifact_store: &dyn ResponseArtifactStore,
    snapshot: &GenerationExecutionSnapshot,
    credentials: &ProviderExecutionCredentials,
) -> Result<ProviderAttemptResponse, GenerationExecutionError>;

pub(crate) async fn resume_verified_response(
    decoder: &dyn ImageResponseDecoder,
    file_store: &dyn GenerationFileStore,
    snapshot: &GenerationExecutionSnapshot,
    response: &ProviderAttemptResponse,
    cancellation: &CancellationProbe,
) -> Result<StagedGenerationFiles, GenerationExecutionError>;

pub(crate) fn commit_generation_success_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    context: &GenerationExecutionContext,
    request: &GenerationJobRequest,
    promoted: &mut PromotedGenerationFiles,
) -> Result<GenerateResult, AppError>;

pub(crate) fn promote_verified_response_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    context: &GenerationExecutionContext,
    response: &ProviderAttemptResponse,
) -> Result<(), AppError>;

pub(crate) struct GenerationTerminalDisposition {
    pub status: GenerationJobStatus,
    pub error_code: String,
    pub retryable: bool,
    pub preserve_response_ready: bool,
}

pub(crate) fn finalize_generation_failure_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    context: &GenerationExecutionContext,
    error: &GenerationExecutionError,
    disposition: &GenerationTerminalDisposition,
) -> Result<(), AppError>;
```

`ProviderExecutionCredentials` is ephemeral: do not derive `Serialize`, do not
expose the key through `Debug`, events, logs, errors, or snapshots. The worker
resolves exactly the stored profile ID and passes its current secret through
this value while preserving the stored endpoint/model snapshots.
`GenerationExecutionError` preserves retry/ambiguity metadata for worker policy
while classifying local decode/save/database failures separately. The legacy
adapter is the only layer that converts it back to `AppError`.
`GenerationFileStore`, `CancellationProbe`, the RAII
`StagedGenerationFiles`/`PromotedGenerationFiles` guards,
`ResponseArtifactStore`, and `ImageResponseDecoder` are `Send + Sync` test seams
that do not require a Tauri `AppHandle`; production wrappers construct them from
the app data path. The artifact store, not the gateway, owns the response root
and derives the deterministic path from job/generation identity.

Add explicit conversion between `GenerationJobOptions` and
`GptImageRequestOptions`; do not make the runtime options struct the persisted
DTO. The persisted DTO retains `None` for capability-filtered omissions;
execution applies provider/model defaults and sanitization without writing those
omitted fields back into the canonical snapshot. For every omitted option,
execution loads the concrete normalized field already stored on the linked
generation; it never re-applies possibly changed application defaults after
restart. Repository loading returns one `GenerationExecutionSnapshot` carrying
the canonical request, reconstructed `runtime_options`, persisted `created_at`
and `output_format`, and execution context; provider and staging functions take
that snapshot rather than rebuilding values independently. Validate that
snapshotted `provider_kind` agrees with model routing
before resolving credentials or calling the engine. Change
`ImageEngine::generate` and `ImageEngine::edit` to return
`Result<ProviderAttemptBody, EngineCallError>`; `perform_provider_attempt` then
passes the complete raw successful body, including `requested_image_count`, to
`ResponseArtifactStore` and returns the
verified `ProviderAttemptResponse`. Each engine call performs one
provider HTTP submission and exposes
429 `Retry-After`, explicitly retryable 5xx, known connection-before-response,
and ambiguous post-send failures without logging or returning secrets. A
provider returning fewer images does not trigger hidden follow-up paid calls;
the response proceeds to completion with only the returned candidates, while
request metadata records requested and actual counts.

Preserve absent, valid seconds, valid HTTP-date, and invalid `Retry-After` as
distinct typed states. An invalid header is not silently treated as absent and
must not authorize automatic retry in Task 4. Treat connect failure known to
precede request acceptance separately from timeout, midstream close, or 2xx body
read failure whose provider outcome is ambiguous.

Derive the final response path deterministically from generation/job ID. On a
successful HTTP response, write one self-describing envelope containing body,
byte size, and SHA-256 using `temp -> fsync/close -> rename -> directory fsync`,
then reread and verify it before returning. Gateway code must not mutate
`generation_recoveries`; lifecycle atomically promotes the row to
response-ready after verification. Task 4 adds a distinct persisted
`expected_response_file` before the paid call, plus size/hash columns, to close
the remaining rename-before-promotion crash window without making a requesting
row look response-ready. Use the separate fakeable `ImageResponseDecoder`
decode/download operation
over `ProviderAttemptResponse`; it performs no provider generation submission.
Lifecycle first commits `response_ready` with the verified path, then
decodes/downloads so a crash can resume locally without a paid replay.
Raw body/artifact types are internal and never derive `Serialize` or expose body
content through `Debug`/errors/events.

Move bounded backoff/jitter ownership to the job worker so every automatic attempt
can be persisted. Remove the gateway-owned `max_retries` loop; the synchronous
compatibility adapter performs one classified attempt while first-party callers
move to the queue.

Split execution into the explicit phases above. No phase may create a
generation row. Every error returns a structured outcome without terminal
generation/job mutation; only worker policy knows whether automatic attempts
are exhausted. After policy decides, it calls an explicit
`finalize_generation_failure_in_transaction` that updates generation and job
together. The finalizer receives `GenerationTerminalDisposition`, because the
same execution error can become failed or interrupted, retryable or not, and a
verified response-ready recovery may need to be preserved for local resume.

For success, decode and stage/validate image plus thumbnail files, then promote
them into final generation-owned names before taking the database mutex.
`StagedGenerationFiles::promote` returns an RAII
`PromotedGenerationFiles` cleanup guard. Only then acquire the DB mutex and use
one short pure-SQL transaction for image rows, recovery deletion, generation
completion/requested-vs-actual metadata, and `completed` job transition. If the
transaction fails, remove only files created by this attempt; never delete
previously committed user files. No decode, download, image encoding, file write,
or rename occurs while the DB transaction is open.

On SQL failure, end the transaction scope and release the global connection
mutex before explicitly cleaning or dropping `PromotedGenerationFiles`.
Both the promote hook and the failure-cleanup hook must observe
`db.conn.try_lock()` success; cleanup is never filesystem I/O under the DB lock.

Extend typed canonical generation metadata with optional `actual_image_count`.
Queued/running/retry snapshots keep it `None`; completed metadata sets it to the
actual inserted image-row count and the linked projection validator checks the
same count. This remains compatible with `deny_unknown_fields` and records short
provider responses without changing the requested image count.

Only the provider HTTP future is drop-cancel-safe. Decode/download and file
staging poll `CancellationProbe`; if blocking work has started, wait for it to
finish and let the RAII guard clean up instead of dropping its join handle.
`commit_generation_success_in_transaction` conditionally updates a running job only when
`cancel_requested_at IS NULL`. If cancellation committed first, clean staged
files and atomically acknowledge cancelled; if completion committed first,
later cancellation cannot overwrite it.

Retain `run_generation_lifecycle` as a compatibility adapter, but it is not a
jobless path: generate a unique client request ID, atomically create a real
durable job/generation through the repository, claim it, execute exactly one
classified attempt synchronously, and call the same success/failure finalizers.
Creation and claim use one `BEGIN IMMEDIATE` transaction and a
`claim_job_in_transaction(tx, job_id)` compare-and-set for that exact ID. Never
call FIFO `claim_next_job` after enqueue: an older queued job must remain queued
and cannot be accidentally executed by the compatibility command.
Do not make `job_id` optional. The managed worker owns
`generation-job:updated`; only this compatibility adapter translates committed
outcomes into legacy `generation:*` events.

This direct execution exists only during the Task 3 intermediate commit, before
a worker is available. Task 4 must rewire compatibility commands to enqueue,
wake, and await that durable job while the leased worker remains the sole
provider/lifecycle executor; no direct adapter execution survives C1.

For edits, enqueue persists canonical authorized paths. Execution revalidates
those paths for existence, header, and type after restart without consulting
`SelectedImageRegistry`; invalid paths fail as `source_image_invalid` before an
engine call.

Until Task 4 removes the old startup recovery loop, change its cleanup and scan
in `lib.rs` to operate only on legacy generations for which
`NOT EXISTS (SELECT 1 FROM generation_jobs ...)`. It must never delete or process
a queued/running job's requesting or response-ready recovery row.

- [ ] **Step 4: Run lifecycle and command tests GREEN**

```bash
cd src-tauri && cargo test --lib generation_lifecycle
cargo test --lib api_gateway
cargo test --lib commands::generation
cargo test --lib generation_jobs
cargo test --lib
```

Expected: PASS with no duplicate generation rows.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/api_gateway.rs src-tauri/src/models.rs src-tauri/src/generation_lifecycle.rs src-tauri/src/generation_jobs.rs src-tauri/src/file_manager.rs src-tauri/src/commands/generation.rs src-tauri/src/lib.rs
git commit -m "refactor: execute precreated generations"
```

## Task 4: Managed Queue Worker And Startup Reconciliation

**Files:**
- Create: `src-tauri/src/generation_job_worker.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/generation_jobs.rs`
- Modify: `src-tauri/src/generation_lifecycle.rs`
- Modify: `src-tauri/src/file_manager.rs`
- Modify: `src-tauri/src/commands/generation.rs`
- Modify: `src-tauri/src/updater.rs`

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

Add race and recovery tests: cancellation persisted between claim and token
registration is observed after registration; a late cancel cannot overwrite a
successfully committed completion; one worker error does not terminate the
loop; a response-ready artifact resumes local work without another engine call;
and an event sink sees only already-committed rows after the database lock is
released.

Cover these policies with injected clocks/sleepers and real SQLite state:

- provider future, local decode/download, and blocking staging cancellation,
  including no leaked temporary/final files and cancel-first/completion-first
  transaction races;
- token registration before/during/after durable cancellation, terminal stale
  sender behavior, registry cleanup on every return/panic path, and no DB lock
  while holding the cancellation-map lock;
- exact retry boundaries (`max_auto_attempts=0/1/2`), calls <= `1 + max`, wait
  cancellation without increment, seconds/past/future HTTP-date,
  overflow/huge Retry-After, and crash after an attempt reservation;
- fixed-cadence heartbeat with a fake clock, stop after terminal/cancel, and a
  heartbeat write failure that does not kill the worker;
- notify-before-await, coalesced notifications draining all queued rows,
  fallback discovery without a notify, and no empty-queue busy loop;
- double start, database lease exclusion/takeover, shutdown while idle/backoff/
  provider/local staging, and durable cancellation observed from another queue
  instance;
- lease fencing after takeover: the expired owner's heartbeat, retry,
  response-ready promotion, success, and failure writes are rejected while the
  new owner can recover and commit the only terminal result;
- legacy compatibility command plus an ordinary queued job never exceeds one
  concurrent fake-engine call, and cancelling a running legacy request reaches
  the same worker token and durable terminal state;
- repeated normal Tauri exit requests schedule shutdown once and stay blocked
  until worker join; normal exit and updater restart both perform staging
  cleanup and release the lease before the process exits/restarts, whether idle,
  backing off, awaiting provider I/O, or staging locally;
- missing, tampered, oversized, non-regular, or app-directory-escaping recovery
  artifacts, truncated envelopes, leftover temp files, declared metadata/body
  tampering, repeated crash, rename-before-response-ready-commit,
  response-ready-after-commit crash, and pre-v16 processing generations without
  jobs; a complete deterministic requesting artifact is promoted and recovered
  with zero engine calls, and none of the recovery-only synthetic paths may call
  the provider;
- image/thumbnail artifact-set orphan sweeping after a crash between file
  promotion and the final SQL commit: only strict attempt-UUID names older than
  an injected grace period and absent from both `images.file_path` and
  `images.thumbnail_path` may be removed; a committed set, a current staging
  set, a recently promoted set, or any deterministic response artifact must be
  preserved.

- [ ] **Step 2: Run worker tests and verify RED**

```bash
cd src-tauri && cargo test --lib generation_job_worker::tests
```

Expected: FAIL because the worker module is missing.

- [ ] **Step 3: Implement queue state and worker loop**

Add v17 migration coverage for persisted `generation_jobs.stage`,
`generation_recoveries.expected_response_file`,
`generation_recoveries.response_size`, and
`generation_recoveries.response_sha256`, plus a singleton
`generation_worker_lease` row (`owner_id`, monotonically increasing
`fencing_epoch`, and acquired/heartbeat/expiry timestamps). This database
lease, not process-local state alone, enforces one worker across multiple app
processes and supports fenced expiry takeover.

Create managed state with wake, cooperative cancellation, lifecycle guard, and
task ownership:

```rust
#[derive(Clone)]
pub(crate) struct GenerationJobQueue {
    inner: std::sync::Arc<GenerationJobQueueInner>,
}

struct GenerationJobQueueInner {
    wake: tokio::sync::Notify,
    cancellations: std::sync::Mutex<
        std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>,
    >,
    started: std::sync::atomic::AtomicBool,
    shutdown: tokio::sync::watch::Sender<bool>,
    task: tokio::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    owner_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerTransitionAuthority {
    pub owner_id: String,
    pub fencing_epoch: i64,
}

impl GenerationJobQueue {
    pub(crate) fn wake(&self) { self.inner.wake.notify_one(); }
    pub(crate) async fn start(&self, deps: WorkerDeps) -> Result<(), AppError>;
    pub(crate) async fn cancel(&self, job_id: &str) -> bool;
    pub(crate) async fn shutdown(&self);
}
```

The worker must:

1. Acquire/renew the database lease before reconciliation or claim. Acquisition
   atomically increments `fencing_epoch` and returns
   `WorkerTransitionAuthority { owner_id, fencing_epoch }`. A second in-process
   `start()` fails deterministically; another process remains passive until
   lease expiry/takeover.
2. Reconcile startup states in one short transaction before accepting work.
3. Claim one FIFO queued job with an atomic queued-to-running update that also
   sets persisted stage and heartbeat.
4. Register cancellation by creating sender/receiver, inserting the sender,
   releasing the map lock, then re-reading durable
   `cancel_requested_at` to close the claim/registration race.
5. Resolve the profile secret by stored profile ID without changing endpoint/model snapshots.
6. Before each provider call, derive the deterministic app-owned final response
   path and persist it as `expected_response_file` with a fenced transaction;
   `response_file`, size, and hash remain NULL while state is `requesting`.
7. Drop-cancel only the provider HTTP future. Local decode/download/staging
   observes a cancellation probe; once blocking work starts, await completion
   and RAII cleanup. Never abandon a blocking file task.
8. For `safe_to_retry` errors only, retain the typed
   `RetryAfterHint::{DelaySeconds, HttpDate, Invalid}` through worker policy,
   never derive a retry delay from `Invalid`, calculate
   the delay with the injected clock, wait cancellably, recheck cancellation
   and lease, reserve the next automatic attempt with a conditional
   transaction, then call the engine again on the same job.
9. Convert ambiguous outcomes to `interrupted` with `provider_outcome_unknown`; never replay them automatically.
10. Persist generation/job terminal state in one transaction, release the
   database lock, then emit one structured event built from the committed row.
11. Update `last_heartbeat_at` and renew the lease on an injected interval while
    provider/backoff/recovery/local staging is active. Conditional heartbeat
    failures are logged and do not emit UI events or kill the loop.
12. Drain claims until no queued rows remain. Before sleeping, create/enable a
    `Notify::notified()` future, recheck the DB, then select wake, bounded poll,
    lease timing, or shutdown so wake coalescing cannot strand work.
13. Stop new claims on shutdown, signal the active stage, await safe local
    cleanup, release the lease if still owner, remove the cancellation sender,
    and join the owned task. A single job/event/heartbeat error never stops the
    loop.

`auto_attempt` is the number of automatic retries reserved/started after the
initial call; `max_auto_attempts` is the maximum such retries, so total provider
submissions are at most `1 + max_auto_attempts`. Cancellation during backoff
does not increment it. Immediately before a retry, a conditional transaction
increments `n -> n+1`; a crash after reservation is conservatively reconciled
as interrupted rather than replayed. Use saturating exponential backoff with
injected jitter. The worker's injected clock maps a past HTTP-date to zero delay;
overflow, an unrepresentable future date, or any delay above the configured safe
cap stops auto retry and leaves the terminal job manually retryable instead of
calling early.

Every worker-owned claim, startup transition, stage update, expected-response
path write, retry reservation, heartbeat, response-ready promotion,
cancellation acknowledgement, success, or failure transaction accepts the same
`WorkerTransitionAuthority` and verifies its unexpired `owner_id` plus exact
`fencing_epoch` inside that transaction. Task 4 therefore adds this authority
to the Task 3 response-ready, success, and failure finalizers in
`generation_lifecycle.rs`; checking only the current lease row or owner string
is insufficient. Losing the lease prevents further durable worker transitions,
including a stale owner's terminal commit; the old owner cleans local staging
and exits while the new owner reconciles the still-durable state.

Cancellation uses `send_replace(true)`. The short cancellation registry uses
`std::sync::Mutex`, so a synchronous RAII scope guard deterministically
unregisters the sender on every return and panic path without spawning async
cleanup. Never hold the cancellation-map mutex while acquiring the database
mutex or across an await. Success SQL requires running status and
`cancel_requested_at IS NULL`; zero rows trigger a re-read and cancel
acknowledgement. Cancel-first cleans staged files and commits cancelled;
completion-first remains completed.

Extract a fakeable worker core below Tauri setup. `WorkerDeps` owns
`Arc<dyn Trait + Send + Sync>` engine, file store/decoder, event sink, clock,
sleeper, jitter, and diagnostics seams. Async traits return `Send` futures and
the event sink is object-safe over a concrete DTO. Production wrappers own the
`AppHandle`; core traits do not. Every repository call returns owned values and
releases the `std::sync::Mutex<Connection>` guard in a lexical scope before an
await. Add compile-time Send/Sync/static and `tokio::spawn(worker.run())` tests.

Define `reconcile_startup` as a short repository transaction returning owned
`RecoveryCandidate` values with job/request/recovery metadata. Queued stays
queued. Running with response-ready metadata remains running and becomes a
local recovery candidate; running with a cancellation request but no possible
response becomes cancelled; other unknown running becomes interrupted. A
requesting recovery may contain only its distinct deterministic
`expected_response_file`; its `response_file`, size, and hash remain NULL. If a
self-describing envelope exists after the rename-before-DB-commit crash window,
validate its regular canonical path, bounded size, envelope version/body,
declared length, and SHA-256, then atomically promote it to response-ready with
persisted `response_file`/size/hash and resume locally with no provider call.

Convert pre-v16 `processing` generations without jobs into synthetic
secret-free jobs in the same transaction: verified response-ready rows become
candidates, while unknown requesting rows become interrupted. Such rows use a
dedicated persisted `legacy_response_recovery` stage/marker and a strict
linked-validator branch: unresolved provider sentinels are valid only for a
running recovery-only job backed by a verified response-ready legacy artifact
with persisted size/hash. Read legacy candidates in a short transaction,
validate artifacts outside the DB mutex, then use a fenced transaction to
recheck state and create the running synthetic job. Unknown requesting or
invalid legacy artifacts create a terminal interrupted/failed synthetic job,
never a running unresolved row. Generic FIFO claim, profile resolution, and
provider execution reject the recovery-only stage, so a synthetic job can never
call a provider.

Validate candidate files asynchronously before decode: canonical app-owned
response directory, regular file, bounded size, and parseable supported provider
response. Missing/tampered/escaping files terminalize visibly without engine
calls. A response-ready candidate with cancellation still follows
completion-versus-cancel rules based on whether verified local completion can
commit; do not blindly replay or discard a known provider result. Repeated
crashes leave enough durable metadata for the same local-only recovery.

Image and thumbnail publication uses one random `artifact_set_id` per local
attempt, embedded in both final basenames, so a stale lease owner never shares
or deletes another attempt's pathname. A crash after publication but before the
terminal SQL transaction can bypass RAII cleanup. Startup reconciliation must
therefore run a DB-aware managed-file sweep outside the database mutex: read the
complete referenced image/thumbnail path set in a short transaction, release
the lock, scan only the app-owned image roots, group files by a strict
generation/index/artifact-set naming parser, and delete only whole unreferenced
sets older than an injected grace period. Reacquire a short transaction and
recheck that a candidate set is still unreferenced immediately before deletion
authorization. Never scan arbitrary paths, follow symlinks, remove current
`.generation-staging` contents, or treat deterministic response artifacts as
image orphans. Sweep failure is diagnostic and cannot block the worker loop.

Move the concrete `GenerationJobEvent`, persisted `stage`, committed
job+generation conversation projection, and object-safe `JobEventSink` into
Task 4. Claim, retry ordinal, startup transition, cancel acknowledgement, and
terminal transactions update stage and produce the projection; emit only after
commit/unlock. Sink failure is diagnostic only. Heartbeat does not change stage
or emit. This makes Task 4 independently compilable before command wrappers in
Task 5.

- [ ] **Step 4: Start the worker from Tauri setup**

Manage exactly one `GenerationJobQueue`, perform only short database
reconciliation during setup, and use `tauri::async_runtime::spawn` after the
application handle and managed states are available. Replace the old blocking
startup generation-recovery loop instead of running both systems. Verified
response-ready artifacts are resumed asynchronously by the managed worker;
other unknown running jobs become interrupted without a provider replay. Do
not block setup on recovery, network, filesystem downloads, or the long-running
loop.

Delete the old startup recovery implementation and its setup `block_on` path.
After the worker starts, rewire every legacy synchronous generation command to
enqueue a durable job, wake the queue, and await that job's committed terminal
result for response compatibility. It must neither claim nor invoke lifecycle
or provider code itself. The leased worker is the only provider/lifecycle
executor, including for legacy callers.

For normal `RunEvent::ExitRequested`, use an atomic shutdown state: the first
event calls `ExitRequestApi::prevent_exit()` and schedules shutdown exactly
once; duplicate events while shutdown is pending are also prevented but do not
reschedule it. The task awaits `queue.shutdown()` (including local cleanup,
owner/epoch lease release, and task join), switches the guard to released, and
then calls `AppHandle::exit(original_code)`; only the resulting event is allowed
through. Change `updater::relaunch_app` to receive the queue, await the same
shutdown, and only then call `app.restart()`. Never hold app/database state
locks while awaiting shutdown. Test both paths through a fake exit/restart
coordinator rather than terminating the test process.

The database lease is renewed by the active worker and released only by its
exact owner/epoch; a passive process can take over after expiry. Durable
cancellation and heartbeat polling make cancellation work even when the command
is issued from a different process whose in-memory registry has no sender.

- [ ] **Step 5: Run worker and lifecycle tests GREEN**

```bash
cd src-tauri && cargo test --lib generation_job_worker
cargo test --lib generation_jobs
cargo test --lib generation_lifecycle
```

Expected: PASS.

- [ ] **Step 6: Commit Task 4**

```bash
git add src-tauri/src/generation_job_worker.rs src-tauri/src/db.rs src-tauri/src/generation_jobs.rs src-tauri/src/generation_lifecycle.rs src-tauri/src/file_manager.rs src-tauri/src/commands/generation.rs src-tauri/src/models.rs src-tauri/src/lib.rs src-tauri/src/updater.rs
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

Also test that a repeated `client_request_id` returns before conversation/log/
recovery side effects, edit enqueue persists canonical source paths that remain
usable after clearing `SelectedImageRegistry`, `generation_id` filtering finds
the matching job, and every mutating command emits only after its transaction
has committed. Enqueue/retry wake the worker; cancel signals the cancellation
registry after durable persistence. Read-only list/get neither emit nor wake.
Serialize the full event DTO and prove endpoint/API-key/body-preview secrets
cannot appear.

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
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    job_id: String,
) -> Result<GenerationJob, AppError>;

#[tauri::command]
pub fn retry_generation_job(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    queue: tauri::State<'_, GenerationJobQueue>,
    job_id: String,
    client_request_id: String,
) -> Result<EnqueueGenerationResult, AppError>;
```

`EnqueueGenerationRequest` mirrors all current `generate_image` generation
parameters plus `client_request_id`. `EnqueueEditRequest` mirrors all current
`edit_image` parameters plus `client_request_id`. `GenerationJobFilter`
contains optional `statuses`, `source_kind`, `source_ref_id`, `generation_id`,
`limit`, and `cursor`; `GenerationJobPage` contains `items` and `next_cursor`.
The canonical nested request preserves capability filtering from the existing
caller: parameters omitted because the selected model does not support them
remain omitted instead of being reintroduced as frontend defaults.

Resolve and authorize edit paths at enqueue, persist their canonical values,
and revalidate them at execution. The worker must not depend on
`SelectedImageRegistry`, which is intentionally process-local. Check
`client_request_id` before resolving/creating a conversation or writing logs.

Provider configuration errors after syntactic acceptance must create a visible failed job. Validation errors that make the request unserializable may reject before enqueue.
`EnqueueGenerationResult` returns the raw status plus sanitized retry,
cancellation, error, and timestamp metadata defined in Task 1, so an initial
failed acknowledgement is renderable without a second lookup.

- [ ] **Step 4: Emit a single typed job event**

Reuse the Task 4 `generation-job:updated` DTO/projection and verify it contains
`job_id`, `generation_id`, `conversation_id`, `source_kind`, `source_ref`,
`status`, `stage`, optional `queue_position`, `chain_attempt`, `auto_attempt`,
`max_auto_attempts`, `cancel_requested_at`, `error_code`, sanitized
`error_message`, `retryable`, `queued_at`, `started_at`, and `finished_at`.
Ensure tests serialize the complete shape and assert no secret fields. Build the
event from the committed row, release the database lock, then emit; legacy
generation events must not describe terminal success/failure before the job
transaction commits.

Define a repository/event projection that joins the committed job to its
generation's `conversation_id`. Do not infer conversation identity from
`source_ref` or request JSON. Enqueue, retry, queued cancel, and running
cancel-request all emit this projection; enqueue/retry then wake the queue, and
running cancel signals its registered token after the durable event state
exists.

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

Add exact tests for the complete Rust event shape (including
`conversation_id`, `cancel_requested_at`, source, attempts, retryability, and
timestamps), lookup by `generation_id`, and an enqueue acknowledgement helper.
The helper applies the exhaustive raw-status mapping (so an initial failed ack
is failed, not processing), stores `jobId`, raw `jobStatus`, sanitized
error/retry/cancellation metadata, and replaces optimistic user, assistant, and
source-image generation IDs with the persisted generation ID.

Lock every wrapper's IPC shape: enqueue uses one nested `request` with
snake_case DTO fields; list uses `filters`; get/cancel use top-level `jobId`;
retry uses top-level `jobId` and `clientRequestId`. Verify event unlisten cleanup
and rejected invoke propagation as well as successful mapping.

- [ ] **Step 2: Run tests and verify RED**

```bash
npx vitest run src/lib/api.test.ts src/lib/generationMessages.test.ts
```

Expected: FAIL because job APIs/types do not exist.

- [ ] **Step 3: Add exact frontend types and wrappers**

Define status and job types mirroring Rust. Extend `Message` with optional
`jobId`, `jobStatus`, `jobRetryable`, `jobCancelRequestedAt`, and
`clientRequestId`; the coarse message status is not sufficient for actions.
Add `enqueueGeneration`, `enqueueEdit`, list/get/cancel/retry wrappers, and add:

```ts
export function onGenerationJobUpdated(
  handler: (event: GenerationJobEvent) => void,
) {
  return onGenerationEvent("generation-job:updated", handler);
}
```

Create a stable `generationJobKeys` factory rooted at `generation-jobs`, with
cursor-aware list, detail, and by-generation keys. Hooks must support a
`generation_id` lookup so reloaded conversation messages regain job metadata.
Invalidate both job and generation/conversation query roots on events and
mutations.

Expose one shared event-bridge binding that updates job caches for every event
and can be hosted once by the current page, then moved to `AppLayout` for C2.
Do not create per-message/per-row listeners or a second event family. React
Query invalidation does not replace the active page's manual conversation
reload; Task 7 handles that separately.

Add a dedicated enqueue-ack message helper rather than reusing
`completeGenerationMessage`, which assumes images already exist. Status mapping
is exhaustive: queued/running -> processing; completed -> complete;
failed/cancelled/interrupted -> failed, while raw job state remains available.
Populate its retry/cancel/error metadata directly from the expanded
`EnqueueGenerationResult`.

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
- Modify: `src/components/generate/GenerationFeed.tsx`
- Modify: `src/components/generate/MessageBubble.tsx`
- Modify: `src/locales/*.json`
- Modify: `src/i18n.test.ts`

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

Also add tests for:

- capability-filtered omitted parameters stay omitted in the nested queue
  request;
- an ambiguous enqueue retry reuses the original `client_request_id`;
- a late enqueue acknowledgement after navigation updates caches only and does
  not replace messages, increment the new view epoch, or navigate back;
- matching terminal events reload the active conversation and refresh the
  conversation list, while nonmatching/nonterminal events only update caches;
- queued/running jobs expose Cancel, the persisted cancellation timestamp shows
  Cancelling, and only retryable failed/interrupted jobs expose Retry;
- retry calls `retryGenerationJob(parentJobId, newClientRequestId)` and creates
  a child job instead of resubmitting the original client request;
- a reloaded conversation recovers job metadata by `generation_id`.
- enqueue rejection always releases the composer IPC lock; each cancel/retry
  mutation has per-job pending state, disables only that job's action, restores
  it after failure, and `cancel_requested_at` prevents duplicate Cancel clicks.
- retry-enqueue reuses the existing optimistic user/assistant pair and does not
  increase message count; if an idempotent replay returns `completed`, the
  active matching conversation reloads immediately to hydrate persisted images;
  terminal-job retry success renders exactly one queued child pair tied to the
  returned child job/generation without waiting for a terminal event.

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
shared job queries, and refresh the conversation list for matching terminal
events. Replace the two legacy first-party lifecycle subscriptions with this
single shared bridge. Remove the existing lifecycle-wide `isGenerating` submit
lock: the composer may be disabled only while its enqueue IPC is pending, not
while any persisted job is queued or running.

Build the existing capability-filtered generate/edit payload first, then wrap
it in the queue request; do not re-add unsupported fields. Store the generated
`clientRequestId` on optimistic state so an ambiguous enqueue failure/retry
reuses it. Apply the enqueue-ack helper rather than the completion helper.

Keep two retry paths explicit. A retry-enqueue after an IPC failure with no
acknowledged job resends the identical payload and identical client request ID
to discover/complete the idempotent enqueue and reconciles the existing
optimistic pair rather than appending another. Because the ack has no image
payload, an already-completed replay reloads the active matching conversation
to hydrate its persisted images. A terminal-job retry is available
only with a known retryable parent job and calls the backend retry command with
that parent ID plus a new client request ID; it never resubmits as a root job.
On success, reconcile one new queued child pair immediately (or reload the
matching active conversation once) so the child is visible before any terminal
job event. Navigation epoch rules still prevent stale-view mutation.

Capture the active conversation/view epoch before awaiting enqueue. If the
acknowledgement is stale, invalidate durable data only; do not mutate the new
conversation's messages/version or navigate back. Job-event cache updates are
global, but direct conversation reload is limited to a matching active terminal
event.

Hydrate `jobId`/raw state for persisted messages through generation-ID lookup.
Cancel persists first and then shows Cancelling; Retry is a backend child-job
operation and is offered only when `retryable` is true. Track cancel/retry
pending state per job, prevent repeated cancellation once
`cancel_requested_at` exists, and restore actions after mutation failure. The
enqueue IPC lock must clear in `finally` on success or rejection. Replacing the
lifecycle-wide lock must preserve every existing disable condition for empty
prompt, prompt optimization, validation, or another enqueue IPC. Add localized queued,
running, cancelling, cancelled, interrupted, cancel, and retry labels with
eight-locale parity. `MessageBubble` must expose these states instead of hiding
all processing jobs behind the generic loading scene.

Do not create a parallel local queue. SQLite is the source of truth.

- [ ] **Step 4: Run page tests GREEN**

```bash
npx vitest run src/pages/GeneratePage.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit Task 7**

```bash
git add src/pages/GeneratePage.tsx src/pages/GeneratePage.test.tsx src/components/generate/GenerationComposer.tsx src/components/generate/GenerationFeed.tsx src/components/generate/MessageBubble.tsx src/locales src/i18n.test.ts
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

Also cover same-second FIFO ordering, the claim/token-registration cancellation
race, late-cancel completion precedence, generation/job terminal transaction
rollback injection, a short provider response without hidden extra submissions,
worker continuation after one job fails, event-after-commit ordering, canonical
edit paths after clearing the in-memory registry, missing/changed source files,
and startup replacement of the old blocking recovery path. Track exact fake
engine call counts so no ambiguous or response-ready path can replay a paid
request.

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

Commit only if the injected-failure work changed tests or production files; do
not create an empty aggregate commit after earlier focused commits.

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
