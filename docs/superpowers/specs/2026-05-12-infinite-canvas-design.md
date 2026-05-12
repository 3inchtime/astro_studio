# Infinite Canvas Design

## Goal

Add an independent infinite canvas workspace where users can sketch, save canvases as project assets, frame part of the canvas, and use that framed sketch as a source image for existing image generation/editing.

## Product Shape

The first version adds a standalone `/canvas` route and a main navigation item labeled Canvas / 画布. The page uses a four-zone workspace:

- Left: project canvas assets, including project selection and canvas document list.
- Center: a large infinite canvas editing area.
- Bottom: drawing and canvas navigation tools.
- Right: generation controls and generated result previews.

Canvas documents belong to projects. They are not generations and they are not conversations. Generated images created from the canvas continue to use the existing generation, conversation, project gallery, runtime log, and recovery systems.

If the user is not working inside a specific project, new canvas documents use the default project. Later iterations can add direct project-home entry points, but the first implementation should make `/canvas` complete on its own.

## Confirmed Scope

In scope:

- Independent Canvas page.
- Project-scoped canvas document CRUD.
- React Konva based infinite canvas.
- Drawing tools: select, brush, eraser, pan, color, brush size, undo, redo, reset zoom, clear current layer, save.
- Layer tools: create, rename, show or hide, lock, delete, reorder.
- Import image into the active layer.
- Movable/resizable generation frame with aspect ratios tied to generation sizes.
- Export generation frame to PNG.
- Use exported PNG as the first `sourceImagePaths` item for existing `edit_image`.
- Show generated results in the right panel and save them through the existing generation/gallery flow.
- Prompt and model parameters aligned with the existing Generate page.

Out of scope for the first version:

- Text objects.
- Shape tools.
- Lasso or rectangular object selection.
- Object transform handles beyond the generation frame and imported-image placement needed for basic use.
- Automatic generated-image placement back onto the canvas.
- Collaborative editing.
- Cloud sync.

## Technical Direction

Use React Konva rather than tldraw or Fabric.js.

React Konva gives Astro Studio direct ownership of the document format, UI layout, generation frame behavior, and project asset model. It also avoids introducing a production editor dependency that requires a separate commercial license key. The tradeoff is that Astro Studio implements more editor behavior itself, but the first-version tool set is intentionally bounded.

## Data Model

Add a `canvas_documents` table:

- `id TEXT PRIMARY KEY`
- `project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE`
- `name TEXT NOT NULL`
- `document_path TEXT NOT NULL`
- `preview_path TEXT`
- `width INTEGER NOT NULL DEFAULT 0`
- `height INTEGER NOT NULL DEFAULT 0`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`
- `deleted_at TEXT`

Indexes:

- `idx_canvas_documents_project_id`
- `idx_canvas_documents_updated_at`
- `idx_canvas_documents_deleted_at`

The database stores metadata and file paths. The canvas JSON stays in app data files to avoid frequent large writes through the SQLite mutex. Preview PNG files also live under app data.

Document JSON version 1:

```json
{
  "version": 1,
  "viewport": { "x": 0, "y": 0, "scale": 1 },
  "frame": {
    "x": 0,
    "y": 0,
    "width": 1024,
    "height": 1024,
    "aspect": "1:1"
  },
  "layers": [
    {
      "id": "layer-1",
      "name": "Sketch",
      "visible": true,
      "locked": false,
      "objects": []
    }
  ]
}
```

Object types:

- `stroke`: id, points, color, width, opacity, blend mode, tool.
- `image`: id, path, x, y, width, height, rotation.

The document format is versioned so later migrations can add text, shapes, generated-image references, or richer transforms.

## Backend Commands

Add `src-tauri/src/commands/canvas.rs` with these commands:

- `create_canvas_document(project_id: Option<String>, name: Option<String>) -> CanvasDocument`
- `list_canvas_documents(project_id: Option<String>) -> Vec<CanvasDocument>`
- `get_canvas_document(id: String) -> CanvasDocumentWithContent`
- `save_canvas_document(id: String, content: String, preview_png_base64: Option<String>) -> CanvasDocument`
- `rename_canvas_document(id: String, name: String) -> CanvasDocument`
- `delete_canvas_document(id: String) -> ()`
- `save_canvas_export(document_id: String, png_base64: String) -> String`

Files are stored under app data:

- `canvas/documents/<document-id>.json`
- `canvas/previews/<document-id>.png`
- `canvas/exports/<document-id>/<timestamp>-frame.png`

`save_canvas_export` returns an absolute file path that the frontend can pass to `edit_image`.

## Frontend Architecture

Route and navigation:

- Add `/canvas` to `src/App.tsx`.
- Add a Canvas nav item to `AppLayout`.
- Hide the existing conversation sidebar on `/canvas`; the Canvas page owns its left asset panel.

Page and components:

- `src/pages/CanvasPage.tsx`: page shell and orchestration.
- `src/components/canvas/CanvasAssetSidebar.tsx`: project selection, canvas document list, create/rename/delete actions.
- `src/components/canvas/CanvasStage.tsx`: Konva stage, viewport, frame, rendered layers, pointer interaction.
- `src/components/canvas/CanvasToolbar.tsx`: bottom tool surface.
- `src/components/canvas/CanvasLayersPanel.tsx`: layer list and layer actions.
- `src/components/canvas/CanvasGenerationPanel.tsx`: prompt, model parameters, frame export, generation submission, result previews.

Hooks and utilities:

- `src/lib/queries/canvasDocuments.ts`: React Query wrappers.
- `src/lib/canvas/document.ts`: document defaults, validation, normalizers.
- `src/lib/canvas/history.ts`: undo/redo reducer helpers.
- `src/lib/canvas/frame.ts`: aspect ratio and image-size mapping.
- `src/lib/canvas/export.ts`: frame export helpers.

Use existing image model catalog utilities for supported parameter options.

## Canvas Interaction

The stage supports:

- Wheel zoom around pointer.
- Space drag or pan tool drag for viewport movement.
- Brush drawing on the active unlocked visible layer.
- Eraser strokes on the active layer.
- Image import into the active layer.
- Layer visibility, lock state, rename, delete, and reorder.
- Undo and redo for document-level edits.
- Reset zoom to fit the generation frame.

The generation frame is a first-class canvas control. It can be moved and resized while preserving its active aspect ratio. Its aspect ratio is synchronized with the selected generation size where possible.

## Generation Flow

When the user clicks Generate:

1. Validate that the prompt is not empty.
2. Validate that the frame has visible content or allow an intentional empty frame only if the user confirms in a later iteration. First version should block empty exports.
3. Render visible, unlocked and locked layers inside the frame into a PNG.
4. Save the PNG with `save_canvas_export`.
5. Call existing `edit_image` with:
   - prompt
   - model
   - size and other supported model parameters
   - sourceImagePaths containing the exported frame PNG
   - projectId set to the active canvas project
   - conversationId set to a Canvas-specific or newly created conversation in that project
6. Show processing and completion state in the right panel.
7. Save outputs through the existing generation flow so project gallery and generation history continue to work.

The first version does not automatically insert generated images back into the canvas.

Generation metadata should include `canvas_document_id` and exported frame geometry where practical. This keeps generated images traceable to their canvas source.

## Error Handling

Autosave failure:

- Keep the dirty state in memory.
- Show a non-blocking error.
- Let the user retry save.

Export failure:

- Block generation.
- Show an export-specific error.
- Common causes: empty frame, source image load failure, file write failure.

Generation failure:

- Reuse the existing failed generation behavior.
- Keep the canvas document unchanged.
- Keep the exported source path available in retry state if possible.

Delete behavior:

- Canvas delete is soft delete via `deleted_at`.
- Deleting a project soft-deletes its canvas documents along with its conversations and generations.

## Internationalization

Add canvas strings to all locale files:

- Navigation label.
- Empty asset state.
- Create, rename, delete, save.
- Tool names.
- Layer actions.
- Generation panel labels.
- Error messages.

Chinese copy should be first-class, matching the existing product tone.

## Testing Strategy

Frontend tests:

- Document default creation and normalization.
- Layer operations.
- History reducer behavior.
- Frame aspect ratio mapping.
- Export parameter construction.
- Canvas page routing and sidebar rendering.
- Generation panel calls `save_canvas_export` then `edit_image` with the exported path.

Konva rendering itself should be verified with focused browser/manual checks because jsdom does not reliably validate canvas drawing.

Backend tests:

- Create/list/get canvas document.
- Save content and preview path.
- Rename validation.
- Soft delete and project filtering.
- Export PNG path creation.
- Project delete updates related canvas documents.

Verification commands:

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

## Future Extensions

- Generated result "Place on canvas" action.
- Object selection and transforms.
- Text and shape tools.
- Named frames and multiple frames per canvas.
- Canvas asset cards on project home.
- Prompt extraction from a canvas frame.
- Better recovery for unsaved local canvas edits after crashes.
