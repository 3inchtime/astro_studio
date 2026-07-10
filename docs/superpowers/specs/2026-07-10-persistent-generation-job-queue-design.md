# Persistent Generation Job Queue Design

Date: 2026-07-10
Status: Approved design; written specification pending user review
Owner: Codex main agent
Roadmap milestone: C1, followed by C2 user experience

## Context

Astro Studio currently invokes image generation through long-running Tauri
commands. The Rust generation lifecycle already persists generation records,
emits progress/completion/failure events, retries some provider calls, saves
response recovery files, and recovers responses after restart. Those are
valuable foundations, but they do not form a persistent job queue.

Current limitations include:

- No queued state, queue position, cancellation, or global concurrency limit.
- Configuration failures can occur before a durable generation exists.
- Default automatic retry is effectively disabled, while a naive retry could
  duplicate a paid request.
- Provider profile identity and endpoint are not recorded with the generation.
- Running requests without a complete response have no explicit interrupted
  terminal state after restart.
- Errors are mostly flattened into strings instead of stable codes and
  retryability.

## Decision

Introduce a durable `generation_jobs` table and a single background worker.
New enqueue commands return immediately after atomically persisting a job and
generation. The worker is the only component that invokes the generation
lifecycle.

The initial worker concurrency is fixed at one. This is intentional: it makes
claiming, cancellation, restart reconciliation, provider pressure, and UI
ordering deterministic before configurable parallelism is considered.

## Goals

- Persist every accepted generation request before provider work starts.
- Expose queue and execution state across routes and application restarts.
- Support cancellation and manual retry without mutating terminal history.
- Snapshot public execution configuration consistently.
- Preserve existing provider engines and response-ready recovery behavior.
- Prevent unsafe automatic replay of ambiguous paid requests.
- Provide structured events and errors that the frontend can act on.
- Supply a stable foundation for canvas generation rounds.

## Non-Goals

- A node workflow engine.
- Configurable parallelism or per-provider concurrency in C1.
- Drag-to-reorder, priority scheduling, or recurring jobs.
- Provider load balancing or automatic failover.
- API cost estimation, billing, or quotas.
- Persisting API keys in jobs.
- Distributed workers or a server deployment mode.

## Job State Model

Allowed states:

- `queued`
- `running`
- `completed`
- `failed`
- `cancelled`
- `interrupted`

Allowed transitions:

| From | To | Reason |
| --- | --- | --- |
| queued | running | Worker atomically claims the next job |
| queued | cancelled | User cancels before claim |
| running | completed | Images and metadata are durably saved |
| running | failed | A known terminal error occurs |
| running | cancelled | Worker acknowledges cancellation |
| running | interrupted | App exits or outcome cannot be confirmed |

Terminal jobs never transition back to queued. A retry creates a new queued
job with `parent_job_id` referencing the prior attempt.

## Database Model

Add `generation_jobs`:

| Field | Purpose |
| --- | --- |
| `id` | Stable job UUID |
| `client_request_id` | Unique caller operation ID for idempotent enqueue |
| `generation_id` | One-to-one user-visible generation record |
| `parent_job_id` | Prior job when this is a retry |
| `source_kind` | `generate`, `edit`, or `canvas` |
| `source_ref_json` | Conversation/project/canvas-round references |
| `status` | Durable state enum |
| `request_json` | Canonical public request snapshot |
| `provider_kind` | OpenAI or Gemini routing snapshot |
| `provider_profile_id` | Stable profile identity selected at enqueue |
| `endpoint_snapshot` | Endpoint used for this attempt |
| `chain_attempt` | Manual retry number in the linked job chain |
| `auto_attempt` | Provider-call attempt within this job |
| `max_auto_attempts` | Safe automatic attempt ceiling |
| `queued_at` | Enqueue timestamp |
| `started_at` | Worker start timestamp |
| `finished_at` | Terminal timestamp |
| `cancel_requested_at` | Cooperative cancellation request timestamp |
| `last_heartbeat_at` | Worker liveness marker |
| `error_code` | Stable machine-readable error code |
| `error_message` | Sanitized user-facing detail |
| `retryable` | Whether manual retry is allowed |

Indexes support status/queue order, generation lookup, parent chain, and source
lookup. Because application timestamps have one-second precision, FIFO queries
must order by `queued_at` and then SQLite `rowid`; `queued_at` alone is not a
stable insertion order. A unique constraint on `client_request_id` makes a
repeated enqueue return the original job instead of creating a duplicate paid
request. Migration tests must exercise both unique constraints, both foreign
key delete actions, and exact index columns rather than checking names alone.

The generation record is created in the same transaction and begins in
`queued`. Existing generation status consumers must be updated to recognize
queued, running, completed, failed, cancelled, and interrupted consistently.
Claim and terminal/cancel transitions update the generation and job together;
the two records must not expose contradictory durable lifecycle states.

Syntactically valid requests that cannot resolve provider configuration are
inserted atomically as already-failed generation/job pairs. Missing public
snapshot values use the documented `unresolved` identity sentinel and empty
endpoint, with a sanitized nonretryable configuration error; no worker may
claim these terminal rows.

## Secret Handling

`request_json`, job events, runtime logs, and errors must never contain an API
key. The enqueue transaction snapshots:

- Model and provider kind.
- Provider profile ID.
- Endpoint and non-secret public profile options.
- Generation/edit parameters and source references.

At execution time, the worker resolves the API key by profile ID and passes it
only through a non-serializable, redacted execution context. It must never
re-resolve an active profile or replace the snapshotted endpoint/model. A
missing or deleted profile produces `provider_profile_missing`, marks the job
failed, and does not silently select another profile.

Edit requests persist canonical source paths after enqueue-time authorization.
Execution after restart revalidates those persisted paths for existence and
supported file type; it does not depend on the in-memory selected-image
registry. Invalidated paths produce `source_image_invalid` without a provider
call.

System keychain storage is a separate security project. C1 does not make the
current profile storage problem worse and establishes a secret-free job
contract that can later use a keychain-backed resolver.

## Command Contract

Add commands and matching TypeScript API wrappers:

- `enqueue_generation(request) -> EnqueueGenerationResult`
- `enqueue_edit(request) -> EnqueueGenerationResult`
- `list_generation_jobs(filters) -> GenerationJobPage`
- `get_generation_job(job_id) -> GenerationJob`
- `cancel_generation_job(job_id) -> GenerationJob`
- `retry_generation_job(job_id) -> EnqueueGenerationResult`

`EnqueueGenerationResult` includes `job_id`, `generation_id`, conversation ID,
and initial status. It does not await provider completion.

Every enqueue request includes a caller-generated `client_request_id`. If the
same ID is submitted again, the command returns the previously persisted
result before creating a conversation, generation, log, or recovery side
effect. Syntactically valid requests that fail provider configuration
resolution are persisted as failed jobs so the failure remains visible.

Job list filters support `generation_id` in addition to status/source filters,
so a reloaded conversation can recover job metadata for cancel/retry actions.

Existing synchronous commands remain as compatibility adapters until all
first-party frontend callers migrate. They must not become an independent
second execution path.

## Worker Architecture

The managed worker starts with the Tauri application and owns:

1. Startup reconciliation.
2. Atomic claim of the oldest queued job.
3. Per-job cancellation token registration.
4. Provider profile resolution.
5. Invocation of `generation_lifecycle`.
6. Heartbeats and durable state transitions.
7. Emission of structured events.

The worker must not hold the SQLite mutex while awaiting network or filesystem
operations. Claim and each durable transition use short transactions. A
successful terminal transaction inserts images and updates recovery,
generation, and job state together. A terminal failure transaction likewise
updates generation and job together. Events are built from the committed row,
emitted only after releasing the database lock, and never describe a state that
can still roll back.

An in-process wake signal notifies the worker after enqueue. A bounded fallback
poll ensures jobs are still discovered after a lost wake signal.

## Cancellation

Queued cancellation is immediate and transactional.

Running cancellation is cooperative:

1. Set `cancel_requested_at`.
2. Signal the registered cancellation token.
3. Abort provider HTTP, download, or save stages where safe.
4. Let the worker perform the terminal `cancelled` transition.

The UI displays "Cancelling" from `cancel_requested_at`, but the durable status
remains running until the worker acknowledges cancellation. If a provider has
already completed and local save succeeds, completion wins over a late cancel
request so valid output is not discarded.

The worker registers its token immediately after claim and then re-reads the
durable cancellation timestamp. This closes the race where cancellation is
persisted after claim but before the token enters the in-memory registry.

## Retry Policy

Safe automatic retry is intentionally narrow:

- Retry connection failures known to occur before a provider response.
- Retry HTTP 429 while respecting `Retry-After`.
- Retry explicitly retryable 5xx responses.
- Use bounded exponential backoff with jitter.
- Stop immediately on cancellation.

Do not automatically replay a request when the provider may have accepted it
but the outcome is unknown. Mark the job `interrupted`, set `retryable=true`,
and require user confirmation for a new child job.

Automatic retry increments `auto_attempt` on the same running job. Manual retry
copies the canonical request and public profile snapshot into a new job,
resolves the current secret for the same profile ID, resets `auto_attempt`, and
increments `chain_attempt`.

Manual retry also creates a fresh generation record, is allowed only for
retryable failed/interrupted generate/edit jobs, and never mutates its parent.
Reusing a client request ID is idempotent only for the same parent and logical
retry; using it for another parent is a stable idempotency conflict.

## Startup Reconciliation And Recovery

On startup:

- `queued` jobs remain queued.
- `running` jobs with a valid response-ready recovery artifact resume local
  decode/save work without calling the provider again.
- Other `running` jobs become `interrupted` and are not automatically replayed.
- Stale cancellation requests become cancelled only when no provider outcome
  or recovery artifact can exist; otherwise they become interrupted.
- Recovery failures end in a visible failed or interrupted state rather than
  remaining processing forever.

Existing generation recovery artifacts remain the source of truth for
response-ready recovery. Artifact writes must be atomic and verified before a
recovery row is marked response-ready. The job table supplies the missing
execution state. Startup reconciliation replaces the old blocking recovery
loop: setup performs only short database reconciliation, then one managed
worker resumes local recovery asynchronously without a second provider call.

## Structured Errors

Introduce stable job error codes, including:

- `provider_profile_missing`
- `provider_configuration_invalid`
- `source_image_invalid`
- `request_rejected`
- `rate_limited`
- `provider_unavailable`
- `network_before_response`
- `provider_outcome_unknown`
- `response_decode_failed`
- `image_save_failed`
- `cancelled_by_user`
- `recovery_failed`

Each terminal event contains code, sanitized message, stage, and retryability.
Provider raw responses remain in protected diagnostic logs or recovery files,
not in user events.

## Event Contract

Use one job event family with a consistent payload:

- `generation-job:updated`

Payload fields:

- `job_id`
- `generation_id`
- `conversation_id`
- `source_kind`
- `source_ref`
- `status`
- `stage`
- `queue_position` when queued
- `chain_attempt`
- `auto_attempt`
- `cancel_requested_at`
- `error_code`
- `error_message`
- `retryable`
- timestamps

Existing generation progress/complete/failed events may be emitted during the
migration, but first-party UI state must converge on the job event contract.

## Frontend Experience

C1 provides reusable query and mutation hooks plus minimal status integration
in existing generation messages. Submitting no longer disables generation for
the duration of provider execution; it creates a queued message and allows the
user to navigate away.

The enqueue acknowledgement has its own message transition: it keeps the
assistant message processing, records `job_id` plus the raw job status, and
replaces optimistic IDs with the persisted generation identity. Raw job status,
retryability, and cancellation timestamp remain available separately from the
coarse message status. Acknowledgements and terminal events are guarded by the
conversation/view epoch so a late result cannot navigate back to or overwrite
another conversation. All job events update shared caches; only a matching
active terminal event reloads the visible conversation.

C2 adds:

- A task badge in `AppLayout`.
- A global task-center drawer.
- Queue state, source, elapsed time, provider, attempt, cancel, and retry.
- Navigation to conversation, gallery, or canvas source.
- Canvas-local filtering in the canvas inspector.

## Transaction And File Rules

- Enqueue inserts generation and job atomically.
- State transitions compare the expected prior state.
- Completion updates job, generation, images, and recovery state consistently.
- Long-running network and file work happens outside the DB mutex.
- Response and image files are staged before final DB references are committed.
- Failed transactions clean staged files without deleting previously committed
  user data.

## Testing Strategy

### Rust Unit Tests

- Every legal and illegal state transition.
- Atomic enqueue and duplicate command protection.
- Queue ordering and single-worker claim.
- Secret-free snapshots and events.
- Profile deletion between enqueue and execution.
- Cancellation before claim and during each execution stage.
- Retry chain and backoff decisions.

### Fake-Engine Integration Tests

Inject a fake image engine to cover:

- Enqueue through completion.
- 429 with `Retry-After`.
- Retryable 5xx and pre-response network failure.
- Ambiguous provider outcome.
- Cancellation during provider call, download, and save.
- Response-ready restart recovery.
- Restart without a response artifact.
- Image-save and recovery failures.

### Frontend Tests

- Queued/running/terminal message rendering.
- Event-driven query updates.
- Cancel and retry disabled states.
- Navigation away and back without losing job state.
- Task-center filtering and source navigation in C2.

### Release Verification

- `cargo test --lib`
- Targeted Vitest suites
- `npm test`
- `npm run build`
- Formatting and `git diff --check`
- Tauri smoke test: enqueue, navigate, cancel, retry, restart, and recover

## Acceptance Criteria

- Every accepted request has a durable queued job before provider work begins.
- IPC enqueue returns without waiting for generation completion.
- Exactly one worker executes jobs in FIFO order.
- Queued and running jobs can be cancelled with correct terminal semantics.
- Failed/interrupted jobs can be manually retried as linked child jobs.
- Safe retries respect limits and `Retry-After`.
- Ambiguous outcomes are never automatically replayed.
- Restart produces a correct terminal or recoverable state for every prior
  running job.
- Job snapshots and events contain no API keys.
- Normal generation and canvas generation share the same worker and lifecycle.
