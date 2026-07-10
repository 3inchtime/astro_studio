# Infinite Canvas Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Astro Studio's existing Konva infinite canvas into a tldraw-inspired basic editor with selection, marquee multi-select, clipboard, ordering, group movement, shortcuts, and fit camera actions.

**Architecture:** Keep `CanvasDocumentContent` as the saved document model and add focused pure helpers under `src/lib/canvas/` for editor operations. `CanvasPage.tsx` owns document/history/autosave plus editor UI state, while `CanvasStage.tsx` remains the Konva pointer and rendering surface.

**Tech Stack:** React 19, TypeScript, Vitest, React Testing Library, react-konva/Konva, lucide-react, Tailwind CSS v4, Tauri IPC wrappers.

**2026-07-10 execution checkpoint:** Tasks 1-3 are implemented and reviewed in
commits through `4d60b51`. Resume at Task 4. Do not repeat or rewrite the helper
foundation unless a failing integration test proves a defect.

---

## File Structure

- Create `src/lib/canvas/bounds.ts`: object bounds, combined bounds, rectangle intersection, screen-space projection.
- Create `src/lib/canvas/bounds.test.ts`: pure tests for image/stroke bounds and combined rectangles.
- Create `src/lib/canvas/selection.ts`: selectable object collection, hit-testing, marquee selection, toggle/reconcile selection.
- Create `src/lib/canvas/selection.test.ts`: pure tests for visible/unlocked selection behavior.
- Create `src/lib/canvas/transforms.ts`: translate images and strokes by selected ids.
- Create `src/lib/canvas/transforms.test.ts`: pure tests for group movement.
- Create `src/lib/canvas/clipboard.ts`: copy selected objects, paste copies with new ids and offsets.
- Create `src/lib/canvas/clipboard.test.ts`: pure tests for copy/paste behavior.
- Create `src/lib/canvas/ordering.ts`: bring forward/back/front/back inside each layer.
- Create `src/lib/canvas/ordering.test.ts`: pure tests for layer-local ordering.
- Modify `src/lib/canvas/document.ts`: add `removeCanvasObjects`.
- Modify `src/lib/canvas/document.test.ts`: deletion helper coverage.
- Modify `src/lib/canvas/frame.ts`: add fit-to-rect camera helper.
- Modify `src/lib/canvas/frame.test.ts`: fit-to-frame and fit-to-selection camera tests.
- Modify `src/components/canvas/CanvasToolbar.tsx`: add delete/copy/paste/order/fit actions and disabled states.
- Modify `src/components/canvas/CanvasStage.tsx`: add selected ids, marquee, group movement, space-pan support, combined selection chrome.
- Modify `src/pages/CanvasPage.tsx`: own selection/clipboard state, shortcuts, editor command handlers, toolbar wiring.
- Modify `src/pages/CanvasPage.test.tsx`: page-level shortcut and toolbar wiring tests using the existing mocked `CanvasStage`.
- Modify `src/locales/*.json`: add toolbar/status labels for all supported languages.
- Modify `src/i18n.test.ts`: keep existing locale coverage passing after adding keys.

---

## Task 1: Bounds Helpers

**Files:**
- Create: `src/lib/canvas/bounds.ts`
- Test: `src/lib/canvas/bounds.test.ts`

- [ ] **Step 1: Write the failing bounds tests**

Create `src/lib/canvas/bounds.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import {
  canvasRectToScreenRect,
  getCanvasObjectBounds,
  getCombinedCanvasBounds,
  rectsIntersect,
} from "./bounds";

describe("canvas bounds helpers", () => {
  it("computes image bounds from position and dimensions", () => {
    const image = createImageObject({
      id: "image-1",
      image_path: "/tmp/a.png",
      x: 20,
      y: 30,
      width: 200,
      height: 120,
    });

    expect(getCanvasObjectBounds(image)).toEqual({
      x: 20,
      y: 30,
      width: 200,
      height: 120,
    });
  });

  it("computes stroke bounds with stroke size padding", () => {
    const stroke = createStrokeObject({
      id: "stroke-1",
      size: 12,
      points: [10, 40, 60, 20, 90, 80],
    });

    expect(getCanvasObjectBounds(stroke)).toEqual({
      x: 4,
      y: 14,
      width: 92,
      height: 72,
    });
  });

  it("combines selected object bounds across layers", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "layer-1",
          name: "Sketch",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-1",
              image_path: "/tmp/a.png",
              x: 20,
              y: 30,
              width: 200,
              height: 120,
            }),
            createStrokeObject({
              id: "stroke-1",
              size: 10,
              points: [300, 250, 360, 290],
            }),
          ],
        },
      ],
    });

    expect(getCombinedCanvasBounds(content, ["image-1", "stroke-1"])).toEqual({
      x: 20,
      y: 30,
      width: 345,
      height: 265,
    });
    expect(getCombinedCanvasBounds(content, ["missing"])).toBeNull();
  });

  it("checks rectangle intersection and projects to screen space", () => {
    expect(
      rectsIntersect(
        { x: 0, y: 0, width: 100, height: 100 },
        { x: 90, y: 90, width: 20, height: 20 },
      ),
    ).toBe(true);
    expect(
      rectsIntersect(
        { x: 0, y: 0, width: 100, height: 100 },
        { x: 120, y: 120, width: 20, height: 20 },
      ),
    ).toBe(false);
    expect(
      canvasRectToScreenRect(
        { x: 10, y: 20, width: 30, height: 40 },
        { x: 100, y: 200, scale: 2 },
      ),
    ).toEqual({ x: 120, y: 240, width: 60, height: 80 });
  });
});
```

- [ ] **Step 2: Run the bounds tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/bounds.test.ts
```

Expected: FAIL because `src/lib/canvas/bounds.ts` does not exist.

- [ ] **Step 3: Implement bounds helpers**

Create `src/lib/canvas/bounds.ts`:

```ts
import type { CanvasDocumentContent, CanvasObject, CanvasViewport } from "../../types";

export interface CanvasRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function getCanvasObjectBounds(object: CanvasObject): CanvasRect | null {
  if (object.type === "image") {
    return {
      x: object.x,
      y: object.y,
      width: object.width,
      height: object.height,
    };
  }

  if (object.points.length < 2) {
    return null;
  }

  const xs: number[] = [];
  const ys: number[] = [];
  for (let index = 0; index < object.points.length; index += 2) {
    xs.push(object.points[index]);
    ys.push(object.points[index + 1]);
  }

  const padding = object.size / 2;
  const minX = Math.min(...xs) - padding;
  const minY = Math.min(...ys) - padding;
  const maxX = Math.max(...xs) + padding;
  const maxY = Math.max(...ys) + padding;

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

export function getCombinedCanvasBounds(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasRect | null {
  const selected = new Set(objectIds);
  const bounds: CanvasRect[] = [];

  content.layers.forEach((layer) => {
    layer.objects.forEach((object) => {
      if (!selected.has(object.id)) return;
      const objectBounds = getCanvasObjectBounds(object);
      if (objectBounds) bounds.push(objectBounds);
    });
  });

  return combineCanvasRects(bounds);
}

export function combineCanvasRects(rects: CanvasRect[]): CanvasRect | null {
  if (!rects.length) return null;

  const minX = Math.min(...rects.map((rect) => rect.x));
  const minY = Math.min(...rects.map((rect) => rect.y));
  const maxX = Math.max(...rects.map((rect) => rect.x + rect.width));
  const maxY = Math.max(...rects.map((rect) => rect.y + rect.height));

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

export function rectsIntersect(first: CanvasRect, second: CanvasRect): boolean {
  return (
    first.x <= second.x + second.width &&
    first.x + first.width >= second.x &&
    first.y <= second.y + second.height &&
    first.y + first.height >= second.y
  );
}

export function canvasRectToScreenRect(rect: CanvasRect, viewport: CanvasViewport): CanvasRect {
  return {
    x: rect.x * viewport.scale + viewport.x,
    y: rect.y * viewport.scale + viewport.y,
    width: rect.width * viewport.scale,
    height: rect.height * viewport.scale,
  };
}
```

- [ ] **Step 4: Run bounds tests and verify GREEN**

Run:

```bash
npx vitest run src/lib/canvas/bounds.test.ts
```

Expected: PASS.

---

## Task 2: Selection Helpers

**Files:**
- Create: `src/lib/canvas/selection.ts`
- Test: `src/lib/canvas/selection.test.ts`

- [ ] **Step 1: Write failing selection tests**

Create `src/lib/canvas/selection.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import {
  getSelectableCanvasObjects,
  hitTestCanvasObjectId,
  reconcileSelectedObjectIds,
  selectCanvasObjectsInRect,
  toggleSelectedObjectId,
} from "./selection";

const content = createCanvasDocumentContent({
  layers: [
    {
      id: "top",
      name: "Top",
      visible: true,
      locked: false,
      objects: [
        createImageObject({
          id: "image-top",
          image_path: "/tmp/top.png",
          x: 20,
          y: 20,
          width: 120,
          height: 80,
        }),
      ],
    },
    {
      id: "bottom",
      name: "Bottom",
      visible: true,
      locked: false,
      objects: [
        createImageObject({
          id: "image-bottom",
          image_path: "/tmp/bottom.png",
          x: 30,
          y: 30,
          width: 120,
          height: 80,
        }),
        createStrokeObject({
          id: "stroke-1",
          size: 8,
          points: [260, 260, 310, 300],
        }),
      ],
    },
    {
      id: "locked",
      name: "Locked",
      visible: true,
      locked: true,
      objects: [
        createImageObject({
          id: "locked-image",
          image_path: "/tmp/locked.png",
          x: 0,
          y: 0,
          width: 500,
          height: 500,
        }),
      ],
    },
  ],
});

describe("canvas selection helpers", () => {
  it("returns selectable objects from visible unlocked layers only", () => {
    expect(getSelectableCanvasObjects(content).map((entry) => entry.object.id)).toEqual([
      "image-top",
      "image-bottom",
      "stroke-1",
    ]);
  });

  it("hit-tests in reverse visual order", () => {
    expect(hitTestCanvasObjectId(content, { x: 40, y: 40 })).toBe("image-top");
    expect(hitTestCanvasObjectId(content, { x: 285, y: 280 })).toBe("stroke-1");
    expect(hitTestCanvasObjectId(content, { x: 480, y: 480 })).toBeNull();
  });

  it("selects objects whose bounds intersect a marquee rectangle", () => {
    expect(
      selectCanvasObjectsInRect(content, { x: 15, y: 15, width: 150, height: 120 }),
    ).toEqual(["image-top", "image-bottom"]);
  });

  it("toggles and reconciles selected ids", () => {
    expect(toggleSelectedObjectId(["image-top"], "image-bottom")).toEqual([
      "image-top",
      "image-bottom",
    ]);
    expect(toggleSelectedObjectId(["image-top", "image-bottom"], "image-top")).toEqual([
      "image-bottom",
    ]);
    expect(reconcileSelectedObjectIds(content, ["image-top", "missing", "locked-image"])).toEqual([
      "image-top",
    ]);
  });
});
```

- [ ] **Step 2: Run selection tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/selection.test.ts
```

Expected: FAIL because `src/lib/canvas/selection.ts` does not exist.

- [ ] **Step 3: Implement selection helpers**

Create `src/lib/canvas/selection.ts`:

```ts
import type { CanvasDocumentContent, CanvasLayer, CanvasObject } from "../../types";
import type { CanvasRect } from "./bounds";
import { getCanvasObjectBounds, rectsIntersect } from "./bounds";

export interface SelectableCanvasObject {
  layer: CanvasLayer;
  object: CanvasObject;
}

export function getSelectableCanvasObjects(content: CanvasDocumentContent): SelectableCanvasObject[] {
  return content.layers.flatMap((layer) => {
    if (!layer.visible || layer.locked) return [];
    return layer.objects.map((object) => ({ layer, object }));
  });
}

export function hitTestCanvasObjectId(
  content: CanvasDocumentContent,
  point: { x: number; y: number },
): string | null {
  const selectable = getSelectableCanvasObjects(content);
  for (let index = selectable.length - 1; index >= 0; index -= 1) {
    const bounds = getCanvasObjectBounds(selectable[index].object);
    if (
      bounds &&
      point.x >= bounds.x &&
      point.x <= bounds.x + bounds.width &&
      point.y >= bounds.y &&
      point.y <= bounds.y + bounds.height
    ) {
      return selectable[index].object.id;
    }
  }
  return null;
}

export function selectCanvasObjectsInRect(
  content: CanvasDocumentContent,
  rect: CanvasRect,
): string[] {
  return getSelectableCanvasObjects(content)
    .filter(({ object }) => {
      const bounds = getCanvasObjectBounds(object);
      return bounds ? rectsIntersect(rect, bounds) : false;
    })
    .map(({ object }) => object.id);
}

export function toggleSelectedObjectId(selectedObjectIds: string[], objectId: string): string[] {
  return selectedObjectIds.includes(objectId)
    ? selectedObjectIds.filter((selectedId) => selectedId !== objectId)
    : [...selectedObjectIds, objectId];
}

export function reconcileSelectedObjectIds(
  content: CanvasDocumentContent,
  selectedObjectIds: string[],
): string[] {
  const selectableIds = new Set(getSelectableCanvasObjects(content).map(({ object }) => object.id));
  return selectedObjectIds.filter((objectId) => selectableIds.has(objectId));
}
```

- [ ] **Step 4: Run selection tests and verify GREEN**

Run:

```bash
npx vitest run src/lib/canvas/selection.test.ts
```

Expected: PASS.

---

## Task 3: Document Mutation, Transform, Clipboard, And Ordering Helpers

**Files:**
- Modify: `src/lib/canvas/document.ts`
- Modify: `src/lib/canvas/document.test.ts`
- Create: `src/lib/canvas/transforms.ts`
- Create: `src/lib/canvas/transforms.test.ts`
- Create: `src/lib/canvas/clipboard.ts`
- Create: `src/lib/canvas/clipboard.test.ts`
- Create: `src/lib/canvas/ordering.ts`
- Create: `src/lib/canvas/ordering.test.ts`

- [ ] **Step 1: Write failing document deletion test**

Append to `src/lib/canvas/document.test.ts`:

```ts
import { removeCanvasObjects } from "./document";

it("removes selected objects without touching other layers", () => {
  const content = createCanvasDocumentContent({
    layers: [
      {
        id: "layer-1",
        name: "Layer 1",
        visible: true,
        locked: false,
        objects: [
          createImageObject({ id: "image-1", image_path: "/tmp/1.png", width: 100, height: 100 }),
          createImageObject({ id: "image-2", image_path: "/tmp/2.png", width: 100, height: 100 }),
        ],
      },
      {
        id: "layer-2",
        name: "Layer 2",
        visible: true,
        locked: false,
        objects: [
          createStrokeObject({ id: "stroke-1", points: [0, 0, 10, 10] }),
        ],
      },
    ],
  });

  const next = removeCanvasObjects(content, ["image-1", "stroke-1"]);

  expect(next.layers[0].objects.map((object) => object.id)).toEqual(["image-2"]);
  expect(next.layers[1].objects).toEqual([]);
});
```

- [ ] **Step 2: Run document test and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/document.test.ts
```

Expected: FAIL because `removeCanvasObjects` is not exported.

- [ ] **Step 3: Implement deletion helper**

Add to `src/lib/canvas/document.ts`:

```ts
export function removeCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasDocumentContent {
  const selected = new Set(objectIds);
  return {
    ...cloneCanvasDocumentContent(content),
    layers: content.layers.map((layer) => ({
      ...layer,
      objects: layer.objects.filter((object) => !selected.has(object.id)),
    })),
  };
}
```

- [ ] **Step 4: Write failing transform tests**

Create `src/lib/canvas/transforms.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import { translateCanvasObjects } from "./transforms";

describe("canvas transforms", () => {
  it("translates selected images and strokes", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "layer-1",
          name: "Layer 1",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-1",
              image_path: "/tmp/1.png",
              x: 10,
              y: 20,
              width: 100,
              height: 100,
            }),
            createStrokeObject({
              id: "stroke-1",
              points: [0, 0, 10, 10],
            }),
          ],
        },
      ],
    });

    const next = translateCanvasObjects(content, ["image-1", "stroke-1"], { dx: 5, dy: -3 });

    expect(next.layers[0].objects[0]).toMatchObject({ x: 15, y: 17 });
    expect(next.layers[0].objects[1]).toMatchObject({ points: [5, -3, 15, 7] });
  });
});
```

- [ ] **Step 5: Run transform tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/transforms.test.ts
```

Expected: FAIL because `src/lib/canvas/transforms.ts` does not exist.

- [ ] **Step 6: Implement transform helper**

Create `src/lib/canvas/transforms.ts`:

```ts
import type { CanvasDocumentContent } from "../../types";
import { cloneCanvasDocumentContent } from "./document";

export function translateCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
  delta: { dx: number; dy: number },
): CanvasDocumentContent {
  const selected = new Set(objectIds);
  return {
    ...cloneCanvasDocumentContent(content),
    layers: content.layers.map((layer) => ({
      ...layer,
      objects: layer.objects.map((object) => {
        if (!selected.has(object.id)) return object;
        if (object.type === "image") {
          return { ...object, x: object.x + delta.dx, y: object.y + delta.dy };
        }
        return {
          ...object,
          points: object.points.map((point, index) =>
            index % 2 === 0 ? point + delta.dx : point + delta.dy,
          ),
        };
      }),
    })),
  };
}
```

- [ ] **Step 7: Write failing clipboard tests**

Create `src/lib/canvas/clipboard.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import { createCanvasDocumentContent, createImageObject } from "./document";
import { copyCanvasObjects, pasteCanvasObjects } from "./clipboard";

describe("canvas clipboard", () => {
  it("copies selected objects and pastes them with new ids into the active layer", () => {
    vi.spyOn(crypto, "randomUUID")
      .mockReturnValueOnce("pasted-1" as `${string}-${string}-${string}-${string}-${string}`)
      .mockReturnValueOnce("pasted-2" as `${string}-${string}-${string}-${string}-${string}`);

    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "layer-1",
          name: "Layer 1",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-1",
              image_path: "/tmp/1.png",
              x: 10,
              y: 20,
              width: 100,
              height: 100,
            }),
          ],
        },
      ],
    });

    const clipboard = copyCanvasObjects(content, ["image-1"]);
    const result = pasteCanvasObjects(content, clipboard, "layer-1", 24);

    expect(result.pastedObjectIds).toEqual(["pasted-1"]);
    expect(result.content.layers[0].objects.map((object) => object.id)).toEqual([
      "image-1",
      "pasted-1",
    ]);
    expect(result.content.layers[0].objects[1]).toMatchObject({ x: 34, y: 44 });
  });
});
```

- [ ] **Step 8: Run clipboard tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/clipboard.test.ts
```

Expected: FAIL because `src/lib/canvas/clipboard.ts` does not exist.

- [ ] **Step 9: Implement clipboard helper**

Create `src/lib/canvas/clipboard.ts`:

```ts
import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { cloneCanvasDocumentContent } from "./document";
import { translateCanvasObjects } from "./transforms";

export interface CanvasClipboard {
  objects: CanvasObject[];
}

export function copyCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasClipboard | null {
  const selected = new Set(objectIds);
  const objects = content.layers.flatMap((layer) =>
    layer.objects.filter((object) => selected.has(object.id)),
  );
  return objects.length ? { objects: JSON.parse(JSON.stringify(objects)) as CanvasObject[] } : null;
}

export function pasteCanvasObjects(
  content: CanvasDocumentContent,
  clipboard: CanvasClipboard | null,
  activeLayerId: string | null,
  offset = 24,
): { content: CanvasDocumentContent; pastedObjectIds: string[] } {
  if (!clipboard?.objects.length || !activeLayerId) {
    return { content: cloneCanvasDocumentContent(content), pastedObjectIds: [] };
  }

  const pastedObjectIds: string[] = [];
  const copies = clipboard.objects.map((object) => {
    const id = crypto.randomUUID();
    pastedObjectIds.push(id);
    return { ...JSON.parse(JSON.stringify(object)), id } as CanvasObject;
  });

  const withCopies = {
    ...cloneCanvasDocumentContent(content),
    layers: content.layers.map((layer) =>
      layer.id === activeLayerId
        ? { ...layer, objects: [...layer.objects, ...copies] }
        : layer,
    ),
  };

  return {
    content: translateCanvasObjects(withCopies, pastedObjectIds, { dx: offset, dy: offset }),
    pastedObjectIds,
  };
}
```

- [ ] **Step 10: Write failing ordering tests**

Create `src/lib/canvas/ordering.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject } from "./document";
import { reorderCanvasObjects } from "./ordering";

function image(id: string) {
  return createImageObject({ id, image_path: `/tmp/${id}.png`, width: 100, height: 100 });
}

describe("canvas ordering", () => {
  it("moves selected objects forward and backward inside their layer", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "layer-1",
          name: "Layer 1",
          visible: true,
          locked: false,
          objects: [image("a"), image("b"), image("c")],
        },
      ],
    });

    expect(
      reorderCanvasObjects(content, ["a"], "forward").layers[0].objects.map((object) => object.id),
    ).toEqual(["b", "a", "c"]);
    expect(
      reorderCanvasObjects(content, ["b"], "backward").layers[0].objects.map((object) => object.id),
    ).toEqual(["b", "a", "c"]);
    expect(
      reorderCanvasObjects(content, ["a"], "front").layers[0].objects.map((object) => object.id),
    ).toEqual(["b", "c", "a"]);
    expect(
      reorderCanvasObjects(content, ["c"], "back").layers[0].objects.map((object) => object.id),
    ).toEqual(["c", "a", "b"]);
  });
});
```

- [ ] **Step 11: Run ordering tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/ordering.test.ts
```

Expected: FAIL because `src/lib/canvas/ordering.ts` does not exist.

- [ ] **Step 12: Implement ordering helper**

Create `src/lib/canvas/ordering.ts`:

```ts
import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { cloneCanvasDocumentContent } from "./document";

export type CanvasOrderDirection = "forward" | "backward" | "front" | "back";

export function reorderCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
  direction: CanvasOrderDirection,
): CanvasDocumentContent {
  const selected = new Set(objectIds);
  return {
    ...cloneCanvasDocumentContent(content),
    layers: content.layers.map((layer) => ({
      ...layer,
      objects: reorderLayerObjects(layer.objects, selected, direction),
    })),
  };
}

function reorderLayerObjects(
  objects: CanvasObject[],
  selected: Set<string>,
  direction: CanvasOrderDirection,
): CanvasObject[] {
  if (direction === "front") {
    return [
      ...objects.filter((object) => !selected.has(object.id)),
      ...objects.filter((object) => selected.has(object.id)),
    ];
  }
  if (direction === "back") {
    return [
      ...objects.filter((object) => selected.has(object.id)),
      ...objects.filter((object) => !selected.has(object.id)),
    ];
  }

  const next = [...objects];
  if (direction === "forward") {
    for (let index = next.length - 2; index >= 0; index -= 1) {
      if (selected.has(next[index].id) && !selected.has(next[index + 1].id)) {
        [next[index], next[index + 1]] = [next[index + 1], next[index]];
      }
    }
  } else {
    for (let index = 1; index < next.length; index += 1) {
      if (selected.has(next[index].id) && !selected.has(next[index - 1].id)) {
        [next[index], next[index - 1]] = [next[index - 1], next[index]];
      }
    }
  }
  return next;
}
```

- [ ] **Step 13: Run helper tests and verify GREEN**

Run:

```bash
npx vitest run src/lib/canvas/document.test.ts src/lib/canvas/transforms.test.ts src/lib/canvas/clipboard.test.ts src/lib/canvas/ordering.test.ts
```

Expected: PASS.

---

## Task 4: Camera Fit Helper

**Files:**
- Modify: `src/lib/canvas/frame.ts`
- Modify: `src/lib/canvas/frame.test.ts`

- [ ] **Step 1: Write failing fit camera tests**

Append to `src/lib/canvas/frame.test.ts`:

```ts
import { fitViewportToCanvasRect } from "./frame";
import type { CanvasRect } from "./bounds";

it("fits a canvas rect into a stage with padding", () => {
  expect(
    fitViewportToCanvasRect(
      { x: 0, y: 0, width: 1024, height: 512 },
      { width: 600, height: 400 },
      40,
    ),
  ).toEqual({
    x: 40,
    y: 70,
    scale: 0.5078125,
  });
});

it("keeps fit camera scale within zoom limits", () => {
  expect(
    fitViewportToCanvasRect(
      { x: 0, y: 0, width: 10, height: 10 },
      { width: 1000, height: 1000 },
      40,
    ).scale,
  ).toBe(4);
});

it("fits a canvas-space rect with a nonzero origin", () => {
  const rect: CanvasRect = { x: 100, y: 50, width: 200, height: 100 };
  expect(fitViewportToCanvasRect(rect, { width: 600, height: 400 }, 40)).toEqual({
    x: -220,
    y: -60,
    scale: 2.6,
  });
});
```

- [ ] **Step 2: Run frame tests and verify RED**

Run:

```bash
npx vitest run src/lib/canvas/frame.test.ts
```

Expected: FAIL because `fitViewportToCanvasRect` is not exported.

- [ ] **Step 3: Implement fit camera helper**

Add to `src/lib/canvas/frame.ts`:

```ts
export function fitViewportToCanvasRect(
  rect: CanvasRect,
  stageSize: { width: number; height: number },
  padding = 64,
): CanvasViewport {
  const availableWidth = Math.max(1, stageSize.width - padding * 2);
  const availableHeight = Math.max(1, stageSize.height - padding * 2);
  const scale = clampZoom(Math.min(availableWidth / rect.width, availableHeight / rect.height));

  return {
    scale,
    x: (stageSize.width - rect.width * scale) / 2 - rect.x * scale,
    y: (stageSize.height - rect.height * scale) / 2 - rect.y * scale,
  };
}
```

- [ ] **Step 4: Run frame tests and verify GREEN**

Run:

```bash
npx vitest run src/lib/canvas/frame.test.ts
```

Expected: PASS.

---

## Task 5: Page State, Shortcuts, And Toolbar Wiring

**Files:**
- Modify: `src/pages/CanvasPage.tsx`
- Modify: `src/pages/CanvasPage.test.tsx`
- Modify: `src/components/canvas/CanvasToolbar.tsx`
- Modify: `src/components/canvas/CanvasStage.tsx` (props contract only; interaction remains Task 6)
- Modify: `src/locales/en.json`
- Modify: `src/locales/de.json`
- Modify: `src/locales/es.json`
- Modify: `src/locales/fr.json`
- Modify: `src/locales/ja.json`
- Modify: `src/locales/ko.json`
- Modify: `src/locales/zh-CN.json`
- Modify: `src/locales/zh-TW.json`

- [ ] **Step 1: Expand the `CanvasStage` mock in `CanvasPage.test.tsx`**

Change the existing mock so tests can invoke selection-aware handlers:

```tsx
vi.mock("../components/canvas/CanvasStage", () => ({
  default: ({
    activeTool,
    selectedObjectIds,
    onSelectionChange,
    onMoveSelection,
    onStageSizeChange,
    onExport,
  }: {
    activeTool: string;
    selectedObjectIds: string[];
    onSelectionChange: (ids: string[]) => void;
    onMoveSelection: (delta: { dx: number; dy: number }) => void;
    onStageSizeChange: (size: { width: number; height: number }) => void;
    onExport: () => Promise<string>;
  }) => (
    <div>
      <div>Canvas stage</div>
      <div>Active tool: {activeTool}</div>
      <div>Selected objects: {selectedObjectIds.join(",") || "none"}</div>
      <button type="button" onClick={() => onSelectionChange(["image-1"])}>
        select image
      </button>
      <button type="button" onClick={() => onMoveSelection({ dx: 12, dy: 8 })}>
        move selection
      </button>
      <button type="button" onClick={() => onStageSizeChange({ width: 800, height: 600 })}>
        resize stage
      </button>
      <button type="button" onClick={() => void onExport()}>
        export stage
      </button>
    </div>
  ),
}));
```

- [ ] **Step 2: Add failing page tests**

Append tests to `src/pages/CanvasPage.test.tsx`:

```tsx
it("deletes the selected canvas object with the keyboard", async () => {
  getCanvasDocument.mockResolvedValueOnce({
    ...(await getCanvasDocument()),
    content: {
      version: 1,
      viewport: { x: 0, y: 0, scale: 1 },
      frame: { x: 0, y: 0, width: 1024, height: 1024, aspect: "1:1" },
      layers: [
        {
          id: "layer-1",
          name: "Sketch",
          visible: true,
          locked: false,
          objects: [
            {
              type: "image",
              id: "image-1",
              image_path: "/tmp/image.png",
              x: 0,
              y: 0,
              width: 100,
              height: 100,
              original_width: 100,
              original_height: 100,
              rotation: 0,
              opacity: 1,
            },
          ],
        },
      ],
    },
  });

  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  fireEvent.keyDown(window, { key: "Delete" });

  await waitFor(() => {
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [expect.objectContaining({ objects: [] })],
      }),
      expect.any(String),
    );
  });
});

it("ignores canvas shortcuts while typing in the generation prompt", async () => {
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "Select" }));
  const promptEditor = await screen.findByPlaceholderText(
    "Describe how to develop this framed sketch...",
  );
  promptEditor.focus();
  fireEvent.keyDown(promptEditor, { key: "b" });

  expect(screen.getByText("Active tool: select")).toBeInTheDocument();
});

it("copies and pastes selected objects with toolbar commands", async () => {
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  fireEvent.click(screen.getByRole("button", { name: "Copy" }));
  fireEvent.click(screen.getByRole("button", { name: "Paste" }));

  await waitFor(() => {
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [
          expect.objectContaining({
            objects: expect.arrayContaining([
              expect.objectContaining({ id: "image-1" }),
              expect.objectContaining({ type: "image" }),
            ]),
          }),
        ],
      }),
      expect.any(String),
    );
  });
});

it("moves selected objects from the stage callback", async () => {
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  fireEvent.click(screen.getByRole("button", { name: "move selection" }));

  await waitFor(() => {
    expect(saveCanvasDocument).toHaveBeenCalled();
  });
});

it("fits the selected object using the reported stage size", async () => {
  getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  fireEvent.click(screen.getByRole("button", { name: "resize stage" }));
  fireEvent.click(screen.getByRole("button", { name: "Fit Selection" }));

  expect(await screen.findByText("1 selected")).toBeInTheDocument();
  expect(screen.getByText(/%$/)).toBeInTheDocument();
  await waitFor(() => {
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        viewport: expect.objectContaining({ scale: 4 }),
      }),
      expect.any(String),
    );
  });
});

it("brings a selected object to the front", async () => {
  getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithTwoImages());
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  fireEvent.click(screen.getByRole("button", { name: "Bring to Front" }));

  await waitFor(() => {
    const savedContent = saveCanvasDocument.mock.calls.at(-1)?.[1];
    expect(savedContent.layers[0].objects.map((object: { id: string }) => object.id)).toEqual([
      "image-2",
      "image-1",
    ]);
  });
});

it("reconciles selection when its layer becomes locked", async () => {
  getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
  render(<CanvasPage />, { wrapper: TestWrapper });

  fireEvent.click(await screen.findByRole("button", { name: "select image" }));
  expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Lock Layer" }));

  expect(await screen.findByText("Selected objects: none")).toBeInTheDocument();
});
```

Also add deferred-promise regressions for document lifecycle safety:

- Start loading A, select B, resolve B first, then A; assert the late A result
  never replaces B content.
- Copy on loaded A, switch to unresolved B, issue paste, advance the autosave
  debounce, and assert no A-derived content is written to B before B loads.
- Resolve an older A save after B becomes dirty and assert the A completion does
  not clear B's dirty/saving state.
- Start save 1 for A, edit again, and prove save 2 does not call the backend
  until save 1 settles; save 2 must carry the newest snapshot.
- Remove dirty A through a canvas-documents query refresh before its debounce;
  assert the automatic transition flushes A to A before selecting B/null.
- Reject autosave and switch-flush saves; assert dirty content remains, no
  unhandled rejection occurs, Retry Save succeeds, and failed switch stays on A.
- Reject a B load, show a load error, retry the same selected ID, and reach the
  ready B editor after a successful retry.

Before appending the tests above, add this local factory near the existing mock setup:

```ts
function canvasDocumentWithImage() {
  return {
    id: "canvas-1",
    project_id: "project-1",
    name: "Mood board",
    document_path: "/tmp/canvas-1.json",
    preview_path: null,
    width: 1024,
    height: 1024,
    created_at: "2026-05-12T00:00:00Z",
    updated_at: "2026-05-12T00:00:00Z",
    deleted_at: null,
    content: {
      version: 1,
      viewport: { x: 0, y: 0, scale: 1 },
      frame: { x: 0, y: 0, width: 1024, height: 1024, aspect: "1:1" },
      layers: [
        {
          id: "layer-1",
          name: "Sketch",
          visible: true,
          locked: false,
          objects: [
            {
              type: "image" as const,
              id: "image-1",
              image_path: "/tmp/image.png",
              x: 0,
              y: 0,
              width: 100,
              height: 100,
              original_width: 100,
              original_height: 100,
              rotation: 0,
              opacity: 1,
            },
          ],
        },
      ],
    },
  };
}

function canvasDocumentWithTwoImages() {
  const document = canvasDocumentWithImage();
  const firstImage = document.content.layers[0].objects[0];
  return {
    ...document,
    content: {
      ...document.content,
      layers: [{
        ...document.content.layers[0],
        objects: [
          firstImage,
          { ...firstImage, id: "image-2", x: 200 },
        ],
      }],
    },
  };
}
```

Use it in tests with:

```ts
getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
```

- [ ] **Step 3: Run page tests and verify RED**

Run:

```bash
npx vitest run src/pages/CanvasPage.test.tsx
```

Expected: FAIL because toolbar actions, selection state, and keyboard handlers are not implemented.

- [ ] **Step 4: Add toolbar props and buttons**

Modify `CanvasToolbarProps` in `src/components/canvas/CanvasToolbar.tsx`:

```ts
selectedObjectCount: number;
zoomPercent: number;
canPaste: boolean;
onDeleteSelection: () => void;
onCopySelection: () => void;
onPasteSelection: () => void;
onReorderSelection: (direction: CanvasOrderDirection) => void;
onFitFrame: () => void;
onFitSelection: () => void;
```

Import additional icons:

```ts
import {
  ArrowDown,
  ArrowUp,
  BringToFront,
  Brush,
  Clipboard,
  ClipboardCopy,
  Eraser,
  Hand,
  ImagePlus,
  Maximize2,
  Minus,
  MousePointer2,
  Plus,
  Redo2,
  SendToBack,
  Trash2,
  Undo2,
} from "lucide-react";
import type { CanvasOrderDirection } from "../../lib/canvas/ordering";
```

Add icon buttons after import image:

```tsx
<button type="button" aria-label={t("canvas.copySelection")} title={t("canvas.copySelection")} onClick={onCopySelection} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <ClipboardCopy size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.pasteSelection")} title={t("canvas.pasteSelection")} onClick={onPasteSelection} disabled={!canPaste} className={TOOL_BUTTON_CLASS}>
  <Clipboard size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.deleteSelection")} title={t("canvas.deleteSelection")} onClick={onDeleteSelection} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <Trash2 size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.bringForward")} title={t("canvas.bringForward")} onClick={() => onReorderSelection("forward")} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <ArrowUp size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.sendBackward")} title={t("canvas.sendBackward")} onClick={() => onReorderSelection("backward")} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <ArrowDown size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.bringToFront")} title={t("canvas.bringToFront")} onClick={() => onReorderSelection("front")} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <BringToFront size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.sendToBack")} title={t("canvas.sendToBack")} onClick={() => onReorderSelection("back")} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <SendToBack size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.fitFrame")} title={t("canvas.fitFrame")} onClick={onFitFrame} className={TOOL_BUTTON_CLASS}>
  <Maximize2 size={16} strokeWidth={1.8} />
</button>
<button type="button" aria-label={t("canvas.fitSelection")} title={t("canvas.fitSelection")} onClick={onFitSelection} disabled={selectedObjectCount === 0} className={TOOL_BUTTON_CLASS}>
  <MousePointer2 size={16} strokeWidth={1.8} />
</button>
<span>{t("canvas.selectionCount", { count: selectedObjectCount })}</span>
<span>{t("canvas.zoomStatus", { zoom: zoomPercent })}</span>
```

- [ ] **Step 5: Add English locale keys first**

Add these keys to `src/locales/en.json` under the canvas section:

```json
"copySelection": "Copy",
"pasteSelection": "Paste",
"deleteSelection": "Delete",
"bringForward": "Bring Forward",
"sendBackward": "Send Backward",
"bringToFront": "Bring to Front",
"sendToBack": "Send to Back",
"fitFrame": "Fit Frame",
"fitSelection": "Fit Selection",
"selectionCount": "{{count}} selected",
"zoomStatus": "{{zoom}}%"
```

Add save/load recovery labels to every locale as English fallbacks in this
task: `canvas.saveStatus.error = "Save failed"`,
`canvas.retrySave = "Retry save"`,
`canvas.loadError = "Couldn't load this canvas."`, and
`canvas.retryLoad = "Retry load"`.

Add these exact fallback keys to every other locale file first, then improve translations after tests pass:

```json
"copySelection": "Copy",
"pasteSelection": "Paste",
"deleteSelection": "Delete",
"bringForward": "Bring Forward",
"sendBackward": "Send Backward",
"bringToFront": "Bring to Front",
"sendToBack": "Send to Back",
"fitFrame": "Fit Frame",
"fitSelection": "Fit Selection",
"selectionCount": "{{count}} selected",
"zoomStatus": "{{zoom}}%"
```

- [ ] **Step 6: Implement CanvasPage editor state and handlers**

In `CanvasPage.tsx`, import helpers:

```ts
import type { CanvasClipboard } from "../lib/canvas/clipboard";
import { copyCanvasObjects, pasteCanvasObjects } from "../lib/canvas/clipboard";
import { removeCanvasObjects } from "../lib/canvas/document";
import { fitViewportToCanvasRect } from "../lib/canvas/frame";
import { getCombinedCanvasBounds } from "../lib/canvas/bounds";
import { reorderCanvasObjects } from "../lib/canvas/ordering";
import type { CanvasOrderDirection } from "../lib/canvas/ordering";
import { reconcileSelectedObjectIds } from "../lib/canvas/selection";
import { translateCanvasObjects } from "../lib/canvas/transforms";
```

Add state:

```ts
const [selectedObjectIds, setSelectedObjectIds] = useState<string[]>([]);
const [clipboard, setClipboard] = useState<CanvasClipboard | null>(null);
const [stageSize, setStageSize] = useState({ width: 960, height: 640 });
const [loadedDocumentId, setLoadedDocumentId] = useState<string | null>(null);
```

Use a monotonically increasing load-request token and selected-ID ref. Set
`loadedDocumentId` to null and clear selection immediately on a switch; apply a
load result/finally block only when its token and selected ID are still current.
The stage/editor is ready only when `loadedDocumentId === selectedDocumentId`.

Reconcile stale selection whenever document content changes (document switch,
undo/redo, layer visibility/lock, delete, or external load):

```ts
useEffect(() => {
  setSelectedObjectIds((current) => {
    const next = reconcileSelectedObjectIds(content, current);
    return next.length === current.length && next.every((id, index) => id === current[index])
      ? current
      : next;
  });
}, [content]);
```

Add handlers:

```ts
function updateSelection(nextIds: string[]) {
  setSelectedObjectIds(reconcileSelectedObjectIds(content, nextIds));
}

function handleDeleteSelection() {
  if (!selectedObjectIds.length) return;
  updateContent(removeCanvasObjects(content, selectedObjectIds));
  setSelectedObjectIds([]);
}

function handleCopySelection() {
  setClipboard(copyCanvasObjects(content, selectedObjectIds));
}

function handlePasteSelection() {
  const result = pasteCanvasObjects(content, clipboard, activeLayer?.id ?? null);
  if (!result.pastedObjectIds.length) return;
  updateContent(result.content);
  setSelectedObjectIds(result.pastedObjectIds);
}

function handleMoveSelection(delta: { dx: number; dy: number }) {
  if (!selectedObjectIds.length) return;
  updateContent(translateCanvasObjects(content, selectedObjectIds, delta));
}

function handleReorderSelection(direction: CanvasOrderDirection) {
  if (!selectedObjectIds.length) return;
  updateContent(reorderCanvasObjects(content, selectedObjectIds, direction));
}

function handleFitFrame() {
  handleViewportChange(fitViewportToCanvasRect(content.frame, stageSize));
}

function handleFitSelection() {
  const bounds = getCombinedCanvasBounds(content, selectedObjectIds) ?? content.frame;
  handleViewportChange(fitViewportToCanvasRect(bounds, stageSize));
}
```

Gate editor mutations, keyboard commands, and autosave while the selected
document is not the loaded document. Change persistence to
`persistDocument(documentId, snapshot)` so the write target is captured, not
read from a later render. Use a save-operation token plus current document and
snapshot refs so an older save cannot clear dirty/saving state for a newer
document or snapshot. When a user switches away with a pending dirty snapshot,
flush it to the old loaded document ID without delaying the switch.

The readiness gate applies to every keyboard-owned editor mutation, including
Escape selection clearing and `v`/`b`/`e`/`h` tool changes. Add a deferred-load
test that presses a tool shortcut while B is unresolved and proves the tool is
unchanged after B loads.

Serialize/coalesce backend saves so same-document snapshots cannot overtake;
the newest queued snapshot must eventually persist. Route both sidebar and
query-driven document transitions through one async routine that waits for the
old dirty snapshot to save to its old ID before switching. A failed flush keeps
the old document ready and dirty. Catch every fire-and-forget save rejection,
retain the latest snapshot, and expose a sanitized Retry Save action. Track
token-guarded load errors and expose Retry Load for the same selected ID.

Add keyboard effect:

```ts
useEffect(() => {
  function isTypingTarget(target: EventTarget | null) {
    return target instanceof HTMLInputElement
      || target instanceof HTMLTextAreaElement
      || (target instanceof HTMLElement && target.isContentEditable);
  }

  function handleKeyDown(event: KeyboardEvent) {
    if (isTypingTarget(event.target)) return;
    const key = event.key.toLowerCase();
    const command = event.metaKey || event.ctrlKey;

    if (command && key === "z") {
      event.preventDefault();
      if (event.shiftKey) handleRedo();
      else handleUndo();
      return;
    }
    if (command && key === "y") {
      event.preventDefault();
      handleRedo();
      return;
    }
    if (command && key === "c") {
      event.preventDefault();
      handleCopySelection();
      return;
    }
    if (command && key === "v") {
      event.preventDefault();
      handlePasteSelection();
      return;
    }
    if (event.key === "Delete" || event.key === "Backspace") {
      event.preventDefault();
      handleDeleteSelection();
      return;
    }
    if (event.key === "Escape") {
      setSelectedObjectIds([]);
      return;
    }
    if (key === "v") setActiveTool("select");
    if (key === "b") setActiveTool("brush");
    if (key === "e") setActiveTool("eraser");
    if (key === "h") setActiveTool("pan");
  }

  window.addEventListener("keydown", handleKeyDown);
  return () => window.removeEventListener("keydown", handleKeyDown);
}, [content, selectedObjectIds, clipboard, activeLayer?.id]);
```

Before wiring the page, extend `CanvasStageProps` with
`selectedObjectIds`, `onSelectionChange`, `onMoveSelection`, and
`onStageSizeChange` so the production component accepts the new page contract.
Do not consume those props or change stage interaction in Task 5; Task 6 owns
that behavior. Then wire `selectedObjectIds`, `onSelectionChange`, `onMoveSelection`, and
`onStageSizeChange` into `CanvasStage`. Wire every command plus
`selectedObjectCount={selectedObjectIds.length}`,
`zoomPercent={Math.round(content.viewport.scale * 100)}`, and
`canPaste={Boolean(clipboard?.entries.length)}` into `CanvasToolbar`.

- [ ] **Step 7: Run page tests and fix compile failures**

Run:

```bash
npx vitest run src/pages/CanvasPage.test.tsx
```

Expected: PASS after import/type issues are fixed.

Mock `exportCanvasFrame` and `readImageSize` at the module boundary in the page
tests. For debounce cases, enable fake timers only after initial loading,
advance 500 ms inside async `act`, flush promises, and restore real timers in
teardown. Task 5 tests must not add jsdom `getContext` or React `act` warnings.

- [ ] **Step 8: Run page typecheck and commit Task 5**

Run:

```bash
npx tsc --noEmit --pretty false
git diff --check
git add src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx src/components/canvas/CanvasToolbar.tsx src/components/canvas/CanvasStage.tsx src/locales
git commit -m "feat: wire canvas editor commands"
```

Expected: typecheck and diff check pass; commit contains only Task 5 files.

---

## Task 6: CanvasStage Selection, Marquee, And Group Movement

**Files:**
- Modify: `src/components/canvas/CanvasStage.tsx`
- Create: `src/components/canvas/CanvasStage.test.tsx`

- [ ] **Step 1: Build a failing Konva façade test harness**

Mock `react-konva` locally in `CanvasStage.test.tsx`:

- `Stage` uses `forwardRef`, captures its pointer handlers, and exposes a
  controlled `getPointerPosition()` result.
- `Transformer` exposes `nodes()` and `getLayer().batchDraw()` spies.
- `Image` exposes a stable imperative node with position/scale/rotation methods.
- `Rect`, `Line`, and `Group` render filtered DOM elements with geometry in
  `data-*` attributes. Production nodes use stable Konva `id`/`name` props for
  object, selection outline, marquee, and generation frame identification.
- A local controllable `ResizeObserver` exposes `triggerResize(width, height)`.
- A controlled wrapper writes `onSelectionChange` back into
  `selectedObjectIds`. Mock `window.Image` so image `onload` is deterministic.

Add RED tests for:

1. Resize `799.6 x 280.2` reports/renders `{ width: 800, height: 320 }`.
2. Click selects the topmost visible/unlocked object; empty click clears.
3. Shift-click adds/removes exact IDs and never arms a drag for that toggle.
4. Down/move/up in one `act` marquee-selects intersecting visible/unlocked
   image and stroke IDs, proving pointer-up does not read stale React state.
5. Marquee chrome uses projected screen geometry while moving and disappears
   at completion.
6. Group drag previews every selected object locally, calls
   `onMoveSelection` zero times during movement and exactly once on pointer-up
   with total delta, and never calls `onTransformImage`.
7. Space from textarea/contenteditable does nothing. Space outside typing
   controls prevents default, pans from the original viewport anchor, and
   keyup/window blur disables temporary pan.
8. One external image attaches exactly its node to `Transformer` and shows
   Reset Aspect. One stroke and multi-selection show bounds chrome without a
   transformer. Hidden/locked/stale IDs show neither.
9. A transformer-anchor pointer-down does not start selection movement.

- [ ] **Step 2: Run stage tests and verify RED**

```bash
npx vitest run src/components/canvas/CanvasStage.test.tsx
```

Expected: FAIL because external selection, marquee, group preview, resize
reporting, and temporary pan are not implemented.

- [ ] **Step 3: Consume the external selection contract**

Destructure the four Task 5 props. Remove local selection state. Derive
effective IDs with `reconcileSelectedObjectIds(content, selectedObjectIds)` so
hidden, locked, missing, and duplicate IDs cannot render chrome or attach a
transformer. A selected image is transformable only when it is the sole
effective selection on a visible/unlocked layer.

Add refs/state:

```ts
const selectionDragRef = useRef<{ canvasPoint: { x: number; y: number } } | null>(null);
const marqueeAnchorRef = useRef<{ x: number; y: number } | null>(null);
const marqueeRectRef = useRef<CanvasRect | null>(null);
const [marqueeRect, setMarqueeRect] = useState<CanvasRect | null>(null);
const [selectionPreviewDelta, setSelectionPreviewDelta] = useState({ dx: 0, dy: 0 });
const spacePanActiveRef = useRef(false);
```

Import and reuse `normalizeCanvasRect` from `bounds.ts`; do not add a duplicate
normalizer.

- [ ] **Step 4: Report size and implement safe temporary pan**

Inside `ResizeObserver`, compute the clamped/rounded `nextSize`, then call both
`setStageSize(nextSize)` and `onStageSizeChange(nextSize)`.

Space handlers must ignore input, textarea, and contenteditable targets,
prevent default for accepted Space events, update the ref synchronously, reset
on keyup and window blur, and clean up all listeners. Space/right-click/pan-tool
pointer-down creates the existing pan anchor without clearing external
selection.

- [ ] **Step 5: Implement click, Shift toggle, and synchronous marquee**

Before generic hit testing, ignore events whose target parent class is
`Transformer`. In select mode use `hitTestCanvasObjectId` and the effective
selection:

- Normal click selects one object, while clicking any member of an existing
  multi-selection preserves the group.
- Shift-click uses `toggleSelectedObjectId`; it never arms group movement.
- Empty click clears and starts marquee.
- Pointer move writes the normalized rectangle to both `marqueeRectRef` and
  state. Pointer-up reads the ref, calls `selectCanvasObjectsInRect`, then
  clears anchor/ref/state. This remains correct when down/move/up are batched.

- [ ] **Step 6: Preview group movement and commit once**

On non-Shift pointer-down for a selected/hit object, store the starting canvas
point. Pointer move computes the total delta from that fixed origin and updates
only `selectionPreviewDelta`. Apply the delta while rendering every effective
selected image/stroke and the combined bounds. Disable/remove native Konva
image dragging and `onDragEnd` so movement cannot apply twice. Pointer-up calls
`onMoveSelection` once when total delta is non-zero, then clears the preview.

- [ ] **Step 7: Render external selection chrome and transformer**

Use stable names `canvas-selection-outline` and `canvas-marquee`. Render the
combined outline for multi-selection and for a single non-image object; a sole
image uses the existing Transformer. Include `loadedImages` in transformer
attachment dependencies so late image loading attaches the node. One hidden,
locked, or stale ID attaches no node. Preserve image transform/reset-aspect
behavior and the existing blue-violet, dashed, non-listening visual language.

- [ ] **Step 8: Run focused integration checks GREEN**

```bash
npx vitest run src/components/canvas/CanvasStage.test.tsx src/pages/CanvasPage.test.tsx
npx tsc --noEmit --pretty false
git diff --check
```

Expected: Stage tests and all 26 CanvasPage tests pass with no canvas/act
warnings; typecheck/diff check pass.

- [ ] **Step 9: Commit Task 6**

```bash
git add src/components/canvas/CanvasStage.tsx src/components/canvas/CanvasStage.test.tsx
git commit -m "feat: add canvas stage multi-selection"
```

Expected: commit contains only the Stage implementation and focused test file.

---

## Task 7: Locale And i18n Verification

**Files:**
- Modify: `src/locales/*.json`
- Modify: `src/i18n.test.ts`

- [ ] **Step 1: Run i18n tests and verify current failure or pass**

Run:

```bash
npx vitest run src/i18n.test.ts
```

Expected: PASS if all new keys exist in every locale; otherwise FAIL listing missing keys.

- [ ] **Step 2: Add missing locale keys**

For every locale, ensure the `canvas` object includes:

```json
"copySelection": "Copy",
"pasteSelection": "Paste",
"deleteSelection": "Delete",
"bringForward": "Bring Forward",
"sendBackward": "Send Backward",
"bringToFront": "Bring to Front",
"sendToBack": "Send to Back",
"fitFrame": "Fit Frame",
"fitSelection": "Fit Selection",
"selectionCount": "{{count}} selected",
"zoomStatus": "{{zoom}}%"
```

Use localized equivalents where practical. The key requirement is that all eight locale files have identical key coverage.

- [ ] **Step 3: Run i18n tests and verify GREEN**

Run:

```bash
npx vitest run src/i18n.test.ts
```

Expected: PASS.

---

## Task 8: Full Verification And Commit

**Files:**
- All files touched by Tasks 1-7

- [ ] **Step 1: Run targeted canvas tests**

Run:

```bash
npx vitest run src/lib/canvas/bounds.test.ts src/lib/canvas/selection.test.ts src/lib/canvas/transforms.test.ts src/lib/canvas/clipboard.test.ts src/lib/canvas/ordering.test.ts src/lib/canvas/document.test.ts src/lib/canvas/frame.test.ts src/pages/CanvasPage.test.tsx src/i18n.test.ts
```

Expected: PASS.

- [ ] **Step 2: Run full frontend test suite**

Run:

```bash
npm test
```

Expected: PASS.

- [ ] **Step 3: Run production frontend build**

Run:

```bash
npm run build
```

Expected: PASS with TypeScript and Vite build completing successfully.

- [ ] **Step 4: Review diff**

Run:

```bash
git diff --check
git diff -- src/lib/canvas src/components/canvas src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx src/locales
```

Expected: no whitespace errors; diff shows only the infinite canvas editor changes.

- [ ] **Step 5: Commit implementation**

Run:

```bash
git add src/lib/canvas src/components/canvas src/pages/CanvasPage.tsx src/pages/CanvasPage.test.tsx src/locales src/i18n.test.ts
git commit -m "feat: improve infinite canvas editor controls"
```

Expected: commit succeeds.

---

## Self-Review

- Spec coverage: The plan covers helper modules, selection, marquee, deletion, copy/paste, ordering, group movement, shortcuts, fit camera actions, locale labels, and verification.
- Scope check: The plan does not introduce tldraw SDK, multiplayer, pages, rich text, arrows, or saved selection state.
- Type consistency: The planned helper names are stable across tasks: `getCanvasObjectBounds`, `getCombinedCanvasBounds`, `selectCanvasObjectsInRect`, `translateCanvasObjects`, `copyCanvasObjects`, `pasteCanvasObjects`, `reorderCanvasObjects`, and `fitViewportToCanvasRect`.
- Test strategy: Every helper task starts with a failing Vitest test before implementation. Page and stage behavior is covered through page tests plus targeted manual interaction assumptions for Konva pointer behavior.
