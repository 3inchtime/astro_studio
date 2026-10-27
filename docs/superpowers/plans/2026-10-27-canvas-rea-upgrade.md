# REA canvas upgrade implementation log

Approved scope: retain Konva, research Excalidraw through actual REA CLI, upgrade navigation/selection, drawing annotation, and image-generation roundtrip. Generated results appear to the right of the submission frame.

## Baseline

- Branch: codex/canvas-rea-upgrade, starting f6d0146.
- Frontend: 57 files / 360 tests passing.
- Rust canvas: 6 tests passing.
- Research runtime: isolated `/tmp/astro-rea-research`, Node 24.22.1, REA 6.8.0. Project dependencies and global assistant configuration unchanged.

## Sequence

1. Actual REA browser capture and pinned Excalidraw source cross-check; preserve evidence and limitations.
2. Data/geometry/export foundation: v2 documents, new objects, Rust roundtrip, snapping/alignment, viewport-safe history, export errors.
3. Canvas gestures and UI: shapes/text IME editing, tools, selection transforms, frame controls, snapping/context menu.
4. Canvas generation controller: durable request identity, status reconciliation, results placement/de-duplication, recovery and UI integration.
5. Independent specification/quality review, unit/regression/build checks, real browser and available desktop validation.

## Acceptance

- v1 documents read without loss; all v2 objects and generation associations survive frontend/Rust roundtrip.
- Native editing supports selection, box/line/arrow/text creation, shortcuts, snapping/alignment, one undo transaction per action.
- Pan/zoom never replays during content undo; cancelled edits do not commit.
- Generated inputs are exact frame exports; no silent fallback on failed images.
- Results preserve input, appear to its right, never duplicate, and reconcile only with the originating document.
- Browser editing and persistence work independently of desktop-only generation.

Detailed approved product plan is in the requesting conversation. This file tracks execution; research findings and verification records are delivered separately.

## Completed verification

- Final frontend: 65 test files / 427 passing tests.
- Final Rust: 359 passing tests, including source registry and scoped status lookup.
- Production frontend build and whitespace checks passed.
- Actual Chrome editor smoke passed without page errors.
- Actual Tauri configured-provider generation completed, retained input, placed output32 units right, persisted completed association. Reopening recovered the finished job.
- Independent foundation/editor/generation specification and quality reviews plus final integration review approved after fixes.
- Temporary native test import/proxy removed. No commits, push, merge or release performed. All deliverables remain reviewable on codex/canvas-rea-upgrade.
