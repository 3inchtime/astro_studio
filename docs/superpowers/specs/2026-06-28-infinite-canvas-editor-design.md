# Infinite Canvas Editor Iteration Design

Date: 2026-06-28
Status: Draft for review
Owner: Codex

## Context

Astro Studio already has an infinite canvas route backed by Tauri canvas document
commands, `react-konva`, autosave, history, layers, brush/eraser strokes, image
import, and frame export into image generation. The current implementation is a
good foundation, but it still feels closer to a drawing surface than a full
editor.

This iteration references tldraw's product direction and SDK architecture
without adopting the tldraw SDK. The useful ideas to borrow are:

- A feature-complete infinite canvas engine with shapes, tools, media, snapping,
  image export, and runtime APIs.
- An editor capability model organized around data/store, tools, selection,
  input handling, camera, coordinates, history, locked shapes, and output.
- A command-like editing surface where selection, deletion, creation, ordering,
  and history are treated as editor operations rather than ad hoc UI events.

References:

- https://github.com/tldraw/tldraw
- https://tldraw.dev/docs/editor

## Decision

Use the existing Konva canvas as the production canvas. Do not introduce the
tldraw SDK in this iteration, because production SDK licensing, document model
migration, Tauri packaging risk, and integration with the current generation
workflow would make the change larger than the desired canvas iteration.

Instead, extend the current canvas with a small local editor layer:

- Pure helper modules for bounds, selection, clipboard, ordering, and batch
  transforms.
- UI state for current selection, marquee selection, transient panning, and
  clipboard contents.
- Existing `CanvasDocumentContent` remains the saved document format.
- Existing history and autosave remain the persistence path.

## Goals

1. Make the canvas feel like a practical editor, not only a sketching surface.
2. Support single selection, Shift multi-select, marquee selection, delete,
   copy, paste, layer ordering, and group movement.
3. Improve camera operations with pointer-centered zoom, spacebar temporary pan,
   fit to frame, and fit to selected objects.
4. Preserve the current image-generation workflow: the frame remains the export
   target, and imported images remain first-class canvas objects.
5. Keep the implementation incremental and testable.

## Non-Goals

- Do not replace Konva with tldraw.
- Do not add multiplayer, pages, bindings, rich text, arrows, or sticky notes in
  this iteration.
- Do not migrate existing saved canvas documents.
- Do not persist UI-only selection state.
- Do not implement full freeform stroke scaling if it complicates the first
  pass. Strokes must support selection, deletion, copy/paste, ordering, and
  movement; scaling may remain image-only.

## User Experience

### Tools

The toolbar keeps the current tools:

- Select
- Brush
- Eraser
- Pan
- Import image

The Select tool becomes the primary editing tool:

- Click an object to select it.
- Shift-click objects to add or remove them from the selection.
- Drag from empty canvas space to create a marquee rectangle and select all
  unlocked visible objects whose bounds intersect the marquee.
- Drag selected objects to move them together.
- Press Escape to clear selection.
- Press Delete or Backspace to delete selected objects.
- Press Cmd/Ctrl+C to copy selected objects.
- Press Cmd/Ctrl+V to paste selected objects with a visible offset and select
  the pasted copies.
- Press Cmd/Ctrl+Z to undo and Cmd/Ctrl+Shift+Z or Cmd/Ctrl+Y to redo.

### Camera

Camera interactions should match common infinite canvas expectations:

- Mouse wheel zooms around the pointer position.
- Middle mouse or secondary mouse drag pans the canvas.
- Holding Space temporarily enters pan mode until released.
- Toolbar includes fit-to-frame and fit-to-selection buttons.
- Zoom percentage is visible in a compact stage status chip.

### Selection Chrome

Selection feedback should be quiet and precise:

- Single selected image keeps the existing transformer handles.
- Multiple selected objects show one combined bounding rectangle.
- Moving a multi-selection previews all selected objects moving together.
- Marquee selection uses a translucent primary-accent rectangle.
- Locked layers do not allow selecting or mutating their objects.
- Hidden layers do not participate in hit tests or marquee selection.

### Ordering

Toolbar actions support:

- Bring forward
- Send backward
- Bring to front
- Send to back

Ordering operates inside each object's current layer. If a multi-selection spans
layers, each selected object is reordered only relative to objects in its own
layer.

## Architecture

### Existing Components

- `src/pages/CanvasPage.tsx` remains the owner of document content, history,
  autosave, selected document, active tool, prompt, and generation state.
- `src/components/canvas/CanvasStage.tsx` remains the Konva renderer and pointer
  interaction surface.
- `src/components/canvas/CanvasToolbar.tsx` remains the floating tool surface.
- `src/lib/canvas/document.ts` remains document construction and mutation helper
  territory.
- `src/lib/canvas/frame.ts` remains camera and coordinate math territory.

### New Helper Modules

Add focused pure helper modules under `src/lib/canvas/`:

- `bounds.ts`
  - Compute bounds for image and stroke objects.
  - Compute combined bounds for selected object ids.
  - Test rectangle intersection.
  - Convert bounds to screen-space rectangles.

- `selection.ts`
  - Resolve selectable objects from visible unlocked layers.
  - Hit-test objects in reverse visual order.
  - Select by marquee rectangle.
  - Toggle object ids in a selection.
  - Filter stale selected ids after content changes.

- `clipboard.ts`
  - Copy selected objects from content.
  - Paste copied objects into the active layer or original layers when possible.
  - Generate new ids.
  - Offset pasted objects so repeated paste is visible.

- `ordering.ts`
  - Move selected objects forward/backward within their layers.
  - Move selected objects to front/back within their layers.

- `transforms.ts`
  - Translate selected objects.
  - Keep image resize logic image-specific.
  - Keep stroke translation and image translation as pure data operations.

These modules do not know about React, Konva, Tauri, React Query, or i18n.

### UI State

`CanvasPage` or `CanvasStage` should own these UI-only states:

- `selectedObjectIds: string[]`
- `marqueeRect: CanvasRect | null`
- `isSpacePanning: boolean`
- `clipboard: CanvasClipboard | null`

Selection state is not written into `CanvasDocumentContent`, because it is an
editor instance concern rather than document content.

### Data Flow

1. User interaction occurs in `CanvasStage` or via global keyboard shortcuts.
2. Components call pure canvas helpers to compute next selection or next content.
3. Content changes flow through `updateContent`.
4. `updateContent` pushes or replaces history depending on the action:
   - Persistent object mutations push history.
   - Live drag previews replace history or stay transient until drag end.
   - Viewport/camera changes replace history and should not create noisy undo
     entries.
5. Autosave continues through the existing debounced save path.

## Error Handling

- If copy/paste is attempted with no selection, no-op.
- If paste is attempted with no active layer, no-op.
- If selected ids no longer exist after document load or deletion, filter them.
- If a selection includes objects on locked or hidden layers, exclude them from
  mutation.
- If fit-to-selection is requested with no selection, fit to frame instead.
- If bounds cannot be computed for an object, exclude that object from marquee
  and fit calculations.

## Accessibility And Shortcuts

Keyboard shortcuts should be attached while the canvas page is active. Avoid
triggering canvas shortcuts when focus is inside text inputs or textareas,
especially the generation prompt editor.

Expected shortcuts:

- `v`: Select
- `b`: Brush
- `e`: Eraser
- `h`: Pan
- `Space` held: temporary pan
- `Escape`: clear selection
- `Delete` / `Backspace`: delete selection
- `Cmd/Ctrl+C`: copy selection
- `Cmd/Ctrl+V`: paste
- `Cmd/Ctrl+Z`: undo
- `Cmd/Ctrl+Shift+Z` or `Cmd/Ctrl+Y`: redo

## Testing Plan

Use test-first implementation for behavior changes.

Pure helper tests:

- Bounds for images and strokes.
- Combined bounds for multiple object ids.
- Hit testing respects visual order and hidden/locked layers.
- Marquee selection selects intersecting visible unlocked objects.
- Deleting selected objects removes only selected objects.
- Copy/paste creates new ids, offsets pasted objects, and selects pasted ids.
- Ordering helpers move selected objects within their layers.
- Translating selected images and strokes updates all relevant coordinates.

Page/component tests:

- Canvas page registers shortcuts only when focus is not in the prompt textarea.
- Delete removes selected objects and marks the document dirty.
- Copy/paste calls content update with newly inserted objects.
- Undo/redo shortcuts call existing history transitions.
- Toolbar enables or disables ordering/delete/copy actions based on selection.

Manual verification:

- Create a canvas, import images, draw strokes.
- Select one image, resize it, reset aspect.
- Shift-select multiple objects and move them together.
- Marquee-select several objects.
- Copy/paste and verify pasted objects are offset and selected.
- Delete selected objects.
- Reorder objects and confirm visual order changes.
- Hold Space to pan.
- Fit to frame and fit to selection.
- Generate from the frame to confirm export workflow still works.

## Implementation Notes

- Keep helper function names explicit and small. Prefer several simple helpers
  over one broad editor class for this iteration.
- Avoid saving selection state to disk.
- Preserve current document version unless a persisted field is added. This
  design does not require a document version bump.
- Prefer existing Tailwind design tokens and lucide icons.
- Keep toolbar compact. Use icon buttons with `title` and `aria-label`.
- Keep locked and hidden layer behavior consistent across selection, mutation,
  and ordering.

## Open Decisions Resolved

- Direction: keep Konva and borrow tldraw interaction patterns.
- Scope: complete basic editor, including marquee multi-select, group movement,
  ordering, shortcuts, and camera improvements.
- Abstraction depth: add focused pure helper modules, not a full local Editor
  Controller yet.

## Acceptance Criteria

- Existing canvas documents load without migration.
- Existing brush, eraser, image import, image transform, layers, autosave, undo,
  redo, and generation export behavior still work.
- Selection supports click, Shift-click, marquee, clear, delete, copy, paste,
  and reorder.
- Multi-selected objects can move together.
- Camera supports pointer zoom, temporary spacebar pan, fit to frame, and fit to
  selection.
- Tests cover pure helper behavior and critical shortcut/page behavior.
- `npm test` passes before the implementation is considered complete.
