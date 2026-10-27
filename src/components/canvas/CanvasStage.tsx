import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import CanvasActionMenu, { type CanvasActionProps } from "./CanvasActionMenu";
import CanvasResizeOverlay from "./CanvasResizeOverlay";
import {
  scaleCanvasObjects,
  snapCanvasSelection,
  type CanvasSnapGuide,
} from "../../lib/canvas/editor";
import type { MutableRefObject } from "react";
import {
  Group,
  Ellipse,
  Shape,
  Image as KonvaImage,
  Layer,
  Line,
  Rect,
  Stage,
  Transformer,
} from "react-konva";
import type Konva from "konva";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import type { CanvasRect } from "../../lib/canvas/bounds";
import {
  canvasRectToScreenRect,
  getCanvasArrowHeadPoints,
  getCombinedCanvasBounds,
  normalizeCanvasRect,
} from "../../lib/canvas/bounds";
import {
  frameToScreenRect,
  panViewportFromPointerDelta,
  screenPointToCanvasPoint,
  viewportFromWheel,
} from "../../lib/canvas/frame";
import { getCanvasLayersBackToFront } from "../../lib/canvas/document";
import {
  hitTestCanvasObjectId,
  reconcileSelectedObjectIds,
  selectCanvasObjectsInRect,
  toggleSelectedObjectId,
} from "../../lib/canvas/selection";
import type {
  CanvasFrame,
  CanvasTextObject,
  CanvasDocumentContent,
  CanvasObject,
  CanvasStrokeObject,
  CanvasTool,
  CanvasViewport,
} from "../../types";

interface CanvasStageProps extends CanvasActionProps {
  onAddObject?: (object: CanvasObject) => void;
  onUpdateText?: (
    id: string,
    text: string,
    width: number,
    height: number,
  ) => void;
  onTransformSelection?: (bounds: CanvasRect) => void;
  onFrameChange?: (frame: CanvasFrame) => void;
  frameEditing?: boolean;
  fillColor?: string;
  objectOpacity?: number;
  fontSize?: number;
  content: CanvasDocumentContent;
  activeLayerId: string | null;
  activeTool: CanvasTool;
  selectedObjectIds: string[];
  strokeColor: string;
  strokeSize: number;
  onViewportChange: (viewport: CanvasViewport) => void;
  onAddStroke: (stroke: CanvasStrokeObject) => void;
  onTransformImage: (
    objectId: string,
    transform: {
      x: number;
      y: number;
      width: number;
      height: number;
      rotation?: number;
    },
  ) => void;
  onResetImageAspect: (objectId: string) => void;
  onSelectionChange: (ids: string[]) => void;
  onMoveSelection: (delta: { dx: number; dy: number }) => void;
  onStageSizeChange: (size: { width: number; height: number }) => void;
  onExport: () => Promise<string>;
}

interface LoadedImageMap {
  [path: string]: HTMLImageElement;
}

const CANVAS_WORLD_SIZE = 6000;
const ZERO_SELECTION_DELTA = { dx: 0, dy: 0 };

export default function CanvasStage({
  content,
  activeLayerId,
  activeTool,
  selectedObjectIds,
  strokeColor,
  strokeSize,
  onCopySelection,
  onPasteSelection,
  onDuplicateSelection,
  onDeleteSelection,
  onReorderSelection,
  onFrameFromSelection,
  onSelectAll,
  canPaste,
  onAddObject,
  onUpdateText,
  onTransformSelection,
  onFrameChange,
  frameEditing = false,
  fillColor = "transparent",
  objectOpacity = 1,
  fontSize = 24,
  onViewportChange,
  onAddStroke,
  onTransformImage,
  onResetImageAspect,
  onSelectionChange,
  onMoveSelection,
  onStageSizeChange,
}: CanvasStageProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const stageRef = useRef<Konva.Stage | null>(null);
  const transformerRef = useRef<Konva.Transformer | null>(null);
  const imageNodeRefs = useRef<Record<string, Konva.Image | null>>({});
  const draftObjectRef = useRef<CanvasObject | null>(null);
  const shapeAnchorRef = useRef<{ x: number; y: number } | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [resizePreview, setResizePreview] = useState<CanvasRect | null>(null);
  const [framePreview, setFramePreview] = useState<CanvasRect | null>(null);
  const [snapGuides, setSnapGuides] = useState<CanvasSnapGuide[]>([]);
  const composingRef = useRef(false);
  const [textEditor, setTextEditor] = useState<
    (CanvasTextObject & { existing?: boolean }) | null
  >(null);
  const textEditorRef = useRef(textEditor);
  textEditorRef.current = textEditor;
  const draftStrokeRef = useRef<CanvasStrokeObject | null>(null);
  const draftContentRef = useRef<CanvasDocumentContent | null>(null);
  const isPointerDrawingRef = useRef(false);
  const panAnchorRef = useRef<{
    pointer: { x: number; y: number };
    viewport: CanvasViewport;
    temporary: boolean;
  } | null>(null);
  const selectionDragRef = useRef<{
    canvasPoint: { x: number; y: number };
    content: CanvasDocumentContent;
    selectionKey: string;
  } | null>(null);
  const marqueeAnchorRef = useRef<{
    x: number;
    y: number;
    content: CanvasDocumentContent;
  } | null>(null);
  const marqueeRectRef = useRef<CanvasRect | null>(null);
  const selectionPreviewDeltaRef = useRef(ZERO_SELECTION_DELTA);
  const spacePanActiveRef = useRef(false);
  const onSelectionChangeRef = useRef(onSelectionChange);
  const onStageSizeChangeRef = useRef(onStageSizeChange);
  const [stageSize, setStageSize] = useState({ width: 960, height: 640 });
  const [loadedImages, setLoadedImages] = useState<LoadedImageMap>({});
  const [marqueeRect, setMarqueeRect] = useState<CanvasRect | null>(null);
  const [selectionPreviewDelta, setSelectionPreviewDelta] =
    useState(ZERO_SELECTION_DELTA);
  const [, rerenderTick] = useState(0);

  onSelectionChangeRef.current = onSelectionChange;
  onStageSizeChangeRef.current = onStageSizeChange;

  const resetSelectionPreview = useCallback(() => {
    selectionPreviewDeltaRef.current = ZERO_SELECTION_DELTA;
    setSelectionPreviewDelta(ZERO_SELECTION_DELTA);
  }, []);

  const cancelActiveGestures = useCallback(() => {
    setContextMenu(null);
    setResizePreview(null);
    setFramePreview(null);
    setSnapGuides([]);
    draftObjectRef.current = null;
    shapeAnchorRef.current = null;
    textEditorRef.current = null;
    composingRef.current = false;
    setTextEditor(null);
    panAnchorRef.current = null;
    selectionDragRef.current = null;
    marqueeAnchorRef.current = null;
    marqueeRectRef.current = null;
    selectionPreviewDeltaRef.current = ZERO_SELECTION_DELTA;
    isPointerDrawingRef.current = false;
    draftStrokeRef.current = null;
    draftContentRef.current = null;
    setMarqueeRect(null);
    setSelectionPreviewDelta(ZERO_SELECTION_DELTA);
    rerenderTick((value) => value + 1);
  }, []);

  useEffect(() => {
    if (!containerRef.current) {
      return;
    }

    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) {
        return;
      }
      const nextSize = {
        width: Math.max(320, Math.round(entry.contentRect.width)),
        height: Math.max(320, Math.round(entry.contentRect.height)),
      };
      setStageSize(nextSize);
      onStageSizeChangeRef.current(nextSize);
    });

    resizeObserver.observe(containerRef.current);
    return () => resizeObserver.disconnect();
  }, []);

  const imagePaths = useMemo(() => {
    const paths = new Set<string>();
    content.layers.forEach((layer) => {
      layer.objects.forEach((object) => {
        if (object.type === "image") {
          paths.add(object.image_path);
        }
      });
    });
    return [...paths];
  }, [content.layers]);

  useEffect(() => {
    imagePaths.forEach((path) => {
      if (loadedImages[path]) {
        return;
      }

      const image = new window.Image();
      image.crossOrigin = "anonymous";
      image.onload = () => {
        setLoadedImages((current) => ({ ...current, [path]: image }));
      };
      image.src = toAssetUrl(path);
    });
  }, [imagePaths, loadedImages]);

  const frameRect = frameToScreenRect(
    { ...content.frame, ...framePreview },
    content.viewport,
  );
  const previewContent = resizePreview
    ? scaleCanvasObjects(content, selectedObjectIds, resizePreview)
    : content;
  const renderedLayers = useMemo(
    () => getCanvasLayersBackToFront(previewContent.layers),
    [previewContent.layers],
  );
  const effectiveSelectedObjectIds = useMemo(
    () => reconcileSelectedObjectIds(content, selectedObjectIds),
    [content, selectedObjectIds],
  );
  const effectiveSelectedObjectIdSet = useMemo(
    () => new Set(effectiveSelectedObjectIds),
    [effectiveSelectedObjectIds],
  );
  const effectiveSelectionKey = getSelectionKey(effectiveSelectedObjectIds);
  const selectedImageObject = useMemo(() => {
    if (effectiveSelectedObjectIds.length !== 1) {
      return null;
    }

    const selectedObjectId = effectiveSelectedObjectIds[0];
    for (const layer of content.layers) {
      if (!layer.visible || layer.locked) {
        continue;
      }
      const object = layer.objects.find(
        (entry) => entry.type === "image" && entry.id === selectedObjectId,
      );
      if (object?.type === "image") {
        return object;
      }
    }
    return null;
  }, [content.layers, effectiveSelectedObjectIds]);
  const selectedImageRect = selectedImageObject
    ? frameToScreenRect(
        {
          x: selectedImageObject.x + selectionPreviewDelta.dx,
          y: selectedImageObject.y + selectionPreviewDelta.dy,
          width: selectedImageObject.width,
          height: selectedImageObject.height,
          aspect: "free",
        },
        content.viewport,
      )
    : null;
  const combinedSelectionBounds = getCombinedCanvasBounds(
    content,
    effectiveSelectedObjectIds,
  );
  const combinedSelectionRect = combinedSelectionBounds
    ? canvasRectToScreenRect(
        {
          ...combinedSelectionBounds,
          x: combinedSelectionBounds.x + selectionPreviewDelta.dx,
          y: combinedSelectionBounds.y + selectionPreviewDelta.dy,
        },
        content.viewport,
      )
    : null;
  const marqueeScreenRect = marqueeRect
    ? canvasRectToScreenRect(marqueeRect, content.viewport)
    : null;
  const backgroundGrid = [];
  for (
    let position = -CANVAS_WORLD_SIZE;
    position <= CANVAS_WORLD_SIZE;
    position += 128
  ) {
    backgroundGrid.push(position);
  }

  function getCanvasPoint() {
    const pointer = stageRef.current?.getPointerPosition();
    if (!pointer) {
      return null;
    }
    return screenPointToCanvasPoint(pointer, content.viewport);
  }

  function handlePointerDown(
    event: Konva.KonvaEventObject<MouseEvent | TouchEvent>,
  ) {
    if (event.target.getParent()?.getClassName() === "Transformer") {
      return;
    }

    if ("button" in event.evt && event.evt.button === 2) return;
    const isSecondaryPan = "button" in event.evt && event.evt.button === 1;
    if (activeTool === "pan" || spacePanActiveRef.current || isSecondaryPan) {
      if (isSecondaryPan) {
        event.evt.preventDefault();
      }
      const pointer = stageRef.current?.getPointerPosition();
      if (!pointer) {
        return;
      }
      cancelActiveGestures();
      panAnchorRef.current = {
        pointer,
        viewport: content.viewport,
        temporary:
          spacePanActiveRef.current && activeTool !== "pan" && !isSecondaryPan,
      };
      return;
    }

    if (frameEditing) return;
    if (activeTool === "select") {
      const point = getCanvasPoint();
      if (!point) return;

      const hitObjectId = hitTestCanvasObjectId(content, point);
      if (hitObjectId) {
        if (event.evt.shiftKey) {
          selectionDragRef.current = null;
          resetSelectionPreview();
          onSelectionChange(
            toggleSelectedObjectId(effectiveSelectedObjectIds, hitObjectId),
          );
          return;
        }

        const nextSelection =
          effectiveSelectedObjectIds.length > 1 &&
          effectiveSelectedObjectIdSet.has(hitObjectId)
            ? effectiveSelectedObjectIds
            : [hitObjectId];
        selectionDragRef.current = {
          canvasPoint: point,
          content,
          selectionKey: getSelectionKey(nextSelection),
        };
        resetSelectionPreview();
        onSelectionChange(nextSelection);
        return;
      }

      onSelectionChange([]);
      selectionDragRef.current = null;
      resetSelectionPreview();
      marqueeAnchorRef.current = { ...point, content };
      const nextMarqueeRect = { x: point.x, y: point.y, width: 0, height: 0 };
      marqueeRectRef.current = nextMarqueeRect;
      setMarqueeRect(nextMarqueeRect);
      return;
    }

    if (textEditorRef.current) return;
    const drawingLayer =
      content.layers.find((l) => l.id === activeLayerId) ?? content.layers[0];
    if (!drawingLayer || drawingLayer.locked || !drawingLayer.visible) return;
    if (activeTool === "text") {
      event.evt.preventDefault();
      const point = getCanvasPoint();
      draftContentRef.current = content;
      if (point)
        setTextEditor({
          type: "text",
          id: crypto.randomUUID(),
          ...point,
          width: 240,
          height: fontSize * 1.2,
          rotation: 0,
          text: "",
          fontSize,
          color: strokeColor,
          opacity: objectOpacity,
        });
      return;
    }
    if (["rectangle", "ellipse", "line", "arrow"].includes(activeTool)) {
      const point = getCanvasPoint();
      if (!point) return;
      shapeAnchorRef.current = point;
      draftContentRef.current = content;
      const style = {
        id: crypto.randomUUID(),
        color: strokeColor,
        size: strokeSize,
        opacity: objectOpacity,
      };
      draftObjectRef.current =
        activeTool === "line" || activeTool === "arrow"
          ? {
              ...style,
              type: activeTool,
              points: [point.x, point.y, point.x, point.y],
            }
          : {
              ...style,
              type: activeTool as "rectangle" | "ellipse",
              ...point,
              width: 0,
              height: 0,
              rotation: 0,
              fill: fillColor,
            };
      rerenderTick((v) => v + 1);
      return;
    }
    if (activeTool !== "brush" && activeTool !== "eraser") {
      return;
    }

    const layer =
      content.layers.find((entry) => entry.id === activeLayerId) ??
      content.layers[0];
    if (!layer || layer.locked || !layer.visible) {
      return;
    }

    const point = getCanvasPoint();
    if (!point) {
      return;
    }

    isPointerDrawingRef.current = true;
    draftContentRef.current = content;
    draftStrokeRef.current = {
      type: "stroke",
      id: crypto.randomUUID(),
      tool: activeTool,
      points: [point.x, point.y],
      color: activeTool === "eraser" ? "#000000" : strokeColor,
      size: strokeSize,
      opacity: objectOpacity,
    };
    rerenderTick((value) => value + 1);
  }

  function handlePointerMove(
    event?: Konva.KonvaEventObject<MouseEvent | TouchEvent>,
  ) {
    if (draftObjectRef.current && shapeAnchorRef.current) {
      const point = getCanvasPoint();
      if (!point) return;
      const a = shapeAnchorRef.current;
      let dx = point.x - a.x,
        dy = point.y - a.y;
      const o = draftObjectRef.current;
      if (event?.evt.shiftKey) {
        if (o.type === "line" || o.type === "arrow" || o.type === "stroke") {
          const angle =
            (Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI) / 4;
          const distance = Math.hypot(dx, dy);
          dx = Math.cos(angle) * distance;
          dy = Math.sin(angle) * distance;
        } else {
          const size = Math.max(Math.abs(dx), Math.abs(dy));
          dx = Math.sign(dx || 1) * size;
          dy = Math.sign(dy || 1) * size;
        }
      }
      draftObjectRef.current =
        o.type === "line" || o.type === "arrow" || o.type === "stroke"
          ? { ...o, points: [a.x, a.y, a.x + dx, a.y + dy] }
          : {
              ...o,
              ...normalizeCanvasRect({ x: a.x, y: a.y, width: dx, height: dy }),
            };
      rerenderTick((v) => v + 1);
      return;
    }
    if (panAnchorRef.current) {
      const pointer = stageRef.current?.getPointerPosition();
      if (!pointer) {
        return;
      }
      onViewportChange(
        panViewportFromPointerDelta(
          panAnchorRef.current.viewport,
          panAnchorRef.current.pointer,
          pointer,
        ),
      );
      return;
    }

    if (selectionDragRef.current) {
      const point = getCanvasPoint();
      if (!point) return;
      const rawDelta = {
        dx: point.x - selectionDragRef.current.canvasPoint.x,
        dy: point.y - selectionDragRef.current.canvasPoint.y,
      };
      const snapped = event?.evt.altKey
        ? { delta: rawDelta, guides: [] }
        : snapCanvasSelection(content, effectiveSelectedObjectIds, rawDelta);
      const nextSelectionPreviewDelta = snapped.delta;
      setSnapGuides(snapped.guides);
      selectionPreviewDeltaRef.current = nextSelectionPreviewDelta;
      setSelectionPreviewDelta(nextSelectionPreviewDelta);
      return;
    }

    if (marqueeAnchorRef.current) {
      const point = getCanvasPoint();
      if (!point) return;
      const nextMarqueeRect = normalizeCanvasRect({
        x: marqueeAnchorRef.current.x,
        y: marqueeAnchorRef.current.y,
        width: point.x - marqueeAnchorRef.current.x,
        height: point.y - marqueeAnchorRef.current.y,
      });
      marqueeRectRef.current = nextMarqueeRect;
      setMarqueeRect(nextMarqueeRect);
      return;
    }

    if (!isPointerDrawingRef.current || !draftStrokeRef.current) {
      return;
    }

    const point = getCanvasPoint();
    if (!point) {
      return;
    }

    draftStrokeRef.current = {
      ...draftStrokeRef.current,
      points: [...draftStrokeRef.current.points, point.x, point.y],
    };
    rerenderTick((value) => value + 1);
  }

  function handlePointerUp() {
    if (draftObjectRef.current) {
      const object = draftObjectRef.current;
      if (
        object.type === "line" ||
        object.type === "arrow" ||
        object.type === "stroke"
          ? Math.hypot(
              object.points[2] - object.points[0],
              object.points[3] - object.points[1],
            ) > 1
          : "width" in object && object.width > 1 && object.height > 1
      )
        onAddObject?.(object);
      draftObjectRef.current = null;
      shapeAnchorRef.current = null;
      draftContentRef.current = null;
      rerenderTick((v) => v + 1);
      return;
    }
    panAnchorRef.current = null;

    if (selectionDragRef.current) {
      const totalDelta = selectionPreviewDeltaRef.current;
      setSnapGuides([]);
      selectionDragRef.current = null;
      resetSelectionPreview();
      if (totalDelta.dx !== 0 || totalDelta.dy !== 0) {
        onMoveSelection(totalDelta);
      }
      return;
    }

    if (marqueeAnchorRef.current) {
      const completedMarqueeRect = marqueeRectRef.current;
      if (completedMarqueeRect) {
        onSelectionChange(
          selectCanvasObjectsInRect(content, completedMarqueeRect),
        );
      }
      marqueeAnchorRef.current = null;
      marqueeRectRef.current = null;
      setMarqueeRect(null);
      return;
    }

    if (!isPointerDrawingRef.current || !draftStrokeRef.current) {
      return;
    }

    isPointerDrawingRef.current = false;
    draftContentRef.current = null;
    if (draftStrokeRef.current.points.length >= 4) {
      onAddStroke(draftStrokeRef.current);
    }
    draftStrokeRef.current = null;
    rerenderTick((value) => value + 1);
  }

  function handleWheel(event: Konva.KonvaEventObject<WheelEvent>) {
    event.evt.preventDefault();

    const pointer = stageRef.current?.getPointerPosition();
    if (!pointer) {
      return;
    }

    onViewportChange(viewportFromWheel(content.viewport, pointer, event.evt));
  }

  useEffect(() => {
    const transformer = transformerRef.current;
    if (!transformer || !selectedImageObject || onTransformSelection) {
      transformer?.nodes([]);
      return;
    }

    const node = imageNodeRefs.current[selectedImageObject.id];
    transformer.nodes(node ? [node] : []);
    transformer.getLayer()?.batchDraw();
  }, [content.layers, loadedImages, selectedImageObject, onTransformSelection]);

  useEffect(() => {
    const activeSelectionDrag = selectionDragRef.current;
    const selectionDragIsStale =
      activeSelectionDrag !== null &&
      (activeSelectionDrag.content !== content ||
        activeSelectionDrag.selectionKey !== effectiveSelectionKey);
    const marqueeIsStale =
      marqueeAnchorRef.current !== null &&
      marqueeAnchorRef.current.content !== content;
    const draftIsStale =
      draftContentRef.current !== null && draftContentRef.current !== content;

    if (selectionDragIsStale || marqueeIsStale || draftIsStale) {
      cancelActiveGestures();
    }
  }, [cancelActiveGestures, content, effectiveSelectionKey]);

  useEffect(() => {
    cancelActiveGestures();
    if (activeTool !== "select") {
      onSelectionChangeRef.current([]);
    }
  }, [activeTool, cancelActiveGestures]);

  useEffect(() => {
    function isTypingTarget(target: EventTarget | null) {
      if (!(target instanceof HTMLElement)) return false;
      const tagName = target.tagName.toLowerCase();
      return (
        tagName === "input" ||
        tagName === "textarea" ||
        target.isContentEditable ||
        target.closest('[contenteditable="true"]') !== null
      );
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        cancelActiveGestures();
        return;
      }
      if (event.code !== "Space" || isTypingTarget(event.target)) return;
      event.preventDefault();
      spacePanActiveRef.current = true;
    }

    function handleKeyUp(event: KeyboardEvent) {
      if (event.code === "Space") {
        spacePanActiveRef.current = false;
        if (panAnchorRef.current?.temporary) {
          panAnchorRef.current = null;
        }
      }
    }

    function handleWindowBlur() {
      spacePanActiveRef.current = false;
      cancelActiveGestures();
    }

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("blur", handleWindowBlur);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("blur", handleWindowBlur);
    };
  }, [cancelActiveGestures]);

  function commitText() {
    const editor = textEditorRef.current;
    if (!editor || composingRef.current) return;
    textEditorRef.current = null;
    draftContentRef.current = null;
    setTextEditor(null);
    const context = document.createElement("canvas").getContext("2d");
    if (context) context.font = `${editor.fontSize}px sans-serif`;
    const width = Math.max(
      1,
      Math.ceil(
        Math.max(
          ...editor.text
            .split("\n")
            .map((line) =>
              context
                ? context.measureText(line).width
                : [...line].length * editor.fontSize,
            ),
        ),
      ),
    );
    const height = Math.max(
      editor.fontSize * 1.2,
      editor.text.split("\n").length * editor.fontSize * 1.2,
    );
    if (editor.existing) onUpdateText?.(editor.id, editor.text, width, height);
    else if (editor.text.trim()) onAddObject?.({ ...editor, width, height });
  }

  return (
    <div
      ref={containerRef}
      className="relative h-full min-h-0 flex-1 overflow-hidden"
      onContextMenu={(event) => {
        event.preventDefault();
        if (
          (event.target as HTMLElement).closest(
            'textarea,input,[contenteditable="true"]',
          )
        )
          return;
        const rect = containerRef.current?.getBoundingClientRect();
        const point = {
          x: event.clientX - (rect?.left ?? 0),
          y: event.clientY - (rect?.top ?? 0),
        };
        const hit = hitTestCanvasObjectId(
          content,
          screenPointToCanvasPoint(point, content.viewport),
        );
        if (hit && !selectedObjectIds.includes(hit)) onSelectionChange([hit]);
        setContextMenu({
          x: Math.max(8, Math.min(stageSize.width - 208, point.x)),
          y: Math.max(8, Math.min(stageSize.height - 330, point.y)),
        });
      }}
    >
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.96),_rgba(244,241,234,0.9)_36%,_rgba(238,234,226,0.98)_100%)]" />
      <div className="absolute left-5 top-5 z-10 rounded-[12px] border border-border-subtle bg-surface/88 px-3 py-2 text-[12px] text-muted shadow-card">
        {t("canvas.emptyStateHint")}
      </div>

      <Stage
        ref={stageRef}
        width={stageSize.width}
        height={stageSize.height}
        onMouseDown={handlePointerDown}
        onMousemove={handlePointerMove}
        onMouseup={handlePointerUp}
        onMouseleave={handlePointerUp}
        onTouchStart={handlePointerDown}
        onTouchMove={handlePointerMove}
        onTouchEnd={handlePointerUp}
        onTouchCancel={cancelActiveGestures}
        onWheel={handleWheel}
        onDblClick={() => {
          const point = getCanvasPoint();
          const id = point && hitTestCanvasObjectId(content, point);
          const object = content.layers
            .flatMap((l) => l.objects)
            .find((o) => o.id === id);
          if (object?.type === "text") {
            cancelActiveGestures();
            draftContentRef.current = content;
            setTextEditor({ ...object, existing: true });
          }
        }}
        className="relative z-0"
      >
        <Layer listening={false}>
          {backgroundGrid.map((position) => (
            <Line
              key={`v-${position}`}
              points={[
                position * content.viewport.scale + content.viewport.x,
                -CANVAS_WORLD_SIZE * content.viewport.scale +
                  content.viewport.y,
                position * content.viewport.scale + content.viewport.x,
                CANVAS_WORLD_SIZE * content.viewport.scale + content.viewport.y,
              ]}
              stroke="rgba(95,88,74,0.06)"
              strokeWidth={1}
            />
          ))}
          {backgroundGrid.map((position) => (
            <Line
              key={`h-${position}`}
              points={[
                -CANVAS_WORLD_SIZE * content.viewport.scale +
                  content.viewport.x,
                position * content.viewport.scale + content.viewport.y,
                CANVAS_WORLD_SIZE * content.viewport.scale + content.viewport.x,
                position * content.viewport.scale + content.viewport.y,
              ]}
              stroke="rgba(95,88,74,0.06)"
              strokeWidth={1}
            />
          ))}
        </Layer>

        {renderedLayers.map((layer) => (
          <Layer key={layer.id} visible={layer.visible}>
            {layer.objects
              .filter((object) => object.id !== textEditor?.id)
              .map((object) =>
                renderCanvasObject(
                  object,
                  content.viewport,
                  loadedImages,
                  effectiveSelectedObjectIdSet.has(object.id),
                  selectionPreviewDelta,
                  imageNodeRefs,
                  onTransformImage,
                ),
              )}
            {layer.id === (activeLayerId ?? content.layers[0]?.id) ? (
              <Group>
                {draftObjectRef.current
                  ? renderCanvasObject(
                      draftObjectRef.current,
                      content.viewport,
                      loadedImages,
                      false,
                      ZERO_SELECTION_DELTA,
                      imageNodeRefs,
                      onTransformImage,
                    )
                  : null}
                {draftStrokeRef.current ? (
                  <Line
                    points={projectStrokePoints(
                      draftStrokeRef.current.points,
                      content.viewport,
                    )}
                    stroke={draftStrokeRef.current.color}
                    strokeWidth={
                      draftStrokeRef.current.size * content.viewport.scale
                    }
                    opacity={draftStrokeRef.current.opacity}
                    lineCap="round"
                    lineJoin="round"
                    tension={0.3}
                    globalCompositeOperation={
                      draftStrokeRef.current.tool === "eraser"
                        ? "destination-out"
                        : "source-over"
                    }
                  />
                ) : null}
              </Group>
            ) : null}
          </Layer>
        ))}
        <Layer>
          {snapGuides.map((guide) => (
            <Line
              key={guide.axis}
              listening={false}
              stroke="#e879b2"
              strokeWidth={1}
              dash={[4, 4]}
              points={
                guide.axis === "x"
                  ? [
                      guide.position * content.viewport.scale +
                        content.viewport.x,
                      0,
                      guide.position * content.viewport.scale +
                        content.viewport.x,
                      stageSize.height,
                    ]
                  : [
                      0,
                      guide.position * content.viewport.scale +
                        content.viewport.y,
                      stageSize.width,
                      guide.position * content.viewport.scale +
                        content.viewport.y,
                    ]
              }
            />
          ))}
          <Rect
            name="canvas-generation-frame"
            x={frameRect.x}
            y={frameRect.y}
            width={frameRect.width}
            height={frameRect.height}
            cornerRadius={18}
            stroke="rgba(79,106,255,0.92)"
            strokeWidth={2}
            dash={[10, 6]}
            shadowColor="rgba(79,106,255,0.2)"
            shadowBlur={18}
            listening={false}
          />

          {combinedSelectionRect &&
          (effectiveSelectedObjectIds.length > 1 || !selectedImageObject) ? (
            <Rect
              name="canvas-selection-outline"
              x={combinedSelectionRect.x}
              y={combinedSelectionRect.y}
              width={combinedSelectionRect.width}
              height={combinedSelectionRect.height}
              stroke="#4f6aff"
              strokeWidth={1.5}
              dash={[8, 5]}
              listening={false}
            />
          ) : null}

          {marqueeScreenRect ? (
            <Rect
              name="canvas-marquee"
              x={marqueeScreenRect.x}
              y={marqueeScreenRect.y}
              width={marqueeScreenRect.width}
              height={marqueeScreenRect.height}
              fill="rgba(79,106,255,0.08)"
              stroke="rgba(79,106,255,0.78)"
              strokeWidth={1}
              dash={[6, 4]}
              listening={false}
            />
          ) : null}

          <Transformer
            ref={transformerRef}
            rotateEnabled={false}
            enabledAnchors={[
              "top-left",
              "top-center",
              "top-right",
              "middle-left",
              "middle-right",
              "bottom-left",
              "bottom-center",
              "bottom-right",
            ]}
            boundBoxFunc={(oldBox, nextBox) => {
              if (nextBox.width < 12 || nextBox.height < 12) {
                return oldBox;
              }
              return nextBox;
            }}
            anchorFill="#ffffff"
            anchorStroke="#4f6aff"
            anchorSize={9}
            borderStroke="#4f6aff"
            borderDash={[8, 5]}
            borderStrokeWidth={1.5}
          />
        </Layer>
      </Stage>

      {frameEditing && onFrameChange ? (
        <CanvasResizeOverlay
          kind="frame"
          bounds={framePreview ?? content.frame}
          viewport={content.viewport}
          identity={content}
          onPreview={setFramePreview}
          onCommit={(bounds) =>
            onFrameChange({
              ...content.frame,
              ...bounds,
              aspect:
                bounds.width === content.frame.width &&
                bounds.height === content.frame.height
                  ? content.frame.aspect
                  : "free",
            })
          }
        />
      ) : null}
      {!frameEditing && combinedSelectionBounds && onTransformSelection ? (
        <CanvasResizeOverlay
          key={effectiveSelectionKey}
          kind="selection"
          keepRatio={content.layers.some((layer) =>
            layer.objects.some(
              (object) =>
                effectiveSelectedObjectIdSet.has(object.id) &&
                (object.type === "stroke" ||
                  object.type === "line" ||
                  object.type === "arrow" ||
                  object.type === "text" ||
                  ("rotation" in object && object.rotation % 360 !== 0)),
            ),
          )}
          bounds={
            resizePreview ?? {
              ...combinedSelectionBounds,
              x: combinedSelectionBounds.x + selectionPreviewDelta.dx,
              y: combinedSelectionBounds.y + selectionPreviewDelta.dy,
            }
          }
          viewport={content.viewport}
          identity={content}
          onPreview={setResizePreview}
          onCommit={onTransformSelection}
        />
      ) : null}
      {contextMenu ? (
        <CanvasActionMenu
          style={{ left: contextMenu.x, top: contextMenu.y }}
          hasSelection={effectiveSelectedObjectIds.length > 0}
          selectionCount={effectiveSelectedObjectIds.length}
          onClose={() => setContextMenu(null)}
          onCopySelection={onCopySelection}
          onPasteSelection={onPasteSelection}
          onDuplicateSelection={onDuplicateSelection}
          onDeleteSelection={onDeleteSelection}
          onReorderSelection={onReorderSelection}
          onFrameFromSelection={onFrameFromSelection}
          onSelectAll={onSelectAll}
          canPaste={canPaste}
        />
      ) : null}
      {textEditor ? (
        <textarea
          autoFocus
          wrap="off"
          aria-label={t("canvas.editText")}
          value={textEditor.text}
          onChange={(event) =>
            setTextEditor({ ...textEditor, text: event.target.value })
          }
          onCompositionStart={() => {
            composingRef.current = true;
          }}
          onCompositionEnd={() => {
            composingRef.current = false;
          }}
          onKeyDown={(event) => {
            event.stopPropagation();
            if (event.key === "Escape") {
              textEditorRef.current = null;
              setTextEditor(null);
            } else if (
              event.key === "Enter" &&
              (event.metaKey || event.ctrlKey) &&
              !event.nativeEvent.isComposing &&
              !composingRef.current
            ) {
              event.preventDefault();
              commitText();
            }
          }}
          onBlur={() => {
            if (document.hasFocus()) commitText();
            else {
              textEditorRef.current = null;
              setTextEditor(null);
            }
          }}
          className="absolute z-30 resize-none rounded border border-primary bg-surface/95 p-0 outline-none"
          style={{
            left: textEditor.x * content.viewport.scale + content.viewport.x,
            top: textEditor.y * content.viewport.scale + content.viewport.y,
            width: Math.max(240, textEditor.width) * content.viewport.scale,
            minHeight: Math.max(80, textEditor.height) * content.viewport.scale,
            fontFamily: "sans-serif",
            fontSize: textEditor.fontSize * content.viewport.scale,
            color: textEditor.color,
            lineHeight: 1.2,
            transform: `rotate(${textEditor.rotation}deg)`,
            transformOrigin: "top left",
          }}
        />
      ) : null}
      {selectedImageObject && selectedImageRect ? (
        <button
          type="button"
          onClick={() => onResetImageAspect(selectedImageObject.id)}
          className="focus-ring absolute z-20 rounded-[10px] border border-border-subtle bg-surface/95 px-3 py-2 text-[12px] font-medium text-foreground shadow-card transition-colors hover:bg-subtle"
          style={{
            left: Math.min(
              stageSize.width - 132,
              Math.max(16, selectedImageRect.x + selectedImageRect.width + 12),
            ),
            top: Math.min(
              stageSize.height - 48,
              Math.max(16, selectedImageRect.y),
            ),
          }}
        >
          {t("canvas.resetImageAspect")}
        </button>
      ) : null}
    </div>
  );
}

function renderCanvasObject(
  object: CanvasObject,
  viewport: CanvasViewport,
  loadedImages: LoadedImageMap,
  isSelected: boolean,
  selectionPreviewDelta: { dx: number; dy: number },
  imageNodeRefs: MutableRefObject<Record<string, Konva.Image | null>>,
  onTransformImage: (
    objectId: string,
    transform: {
      x: number;
      y: number;
      width: number;
      height: number;
      rotation?: number;
    },
  ) => void,
) {
  if (object.type === "stroke") {
    return (
      <Line
        key={object.id}
        id={object.id}
        name="canvas-object"
        points={projectStrokePoints(
          object.points,
          viewport,
          isSelected ? selectionPreviewDelta : undefined,
        )}
        stroke={object.color}
        strokeWidth={object.size * viewport.scale}
        opacity={object.opacity}
        lineCap="round"
        lineJoin="round"
        tension={0.3}
        globalCompositeOperation={
          object.tool === "eraser" ? "destination-out" : "source-over"
        }
      />
    );
  }

  const delta = isSelected ? selectionPreviewDelta : ZERO_SELECTION_DELTA;
  if (object.type === "line" || object.type === "arrow") {
    const props = {
      stroke: object.color,
      strokeWidth: object.size * viewport.scale,
      opacity: object.opacity,
      lineCap: "round" as const,
      lineJoin: "round" as const,
    };
    return (
      <Group key={object.id} id={object.id} name="canvas-object">
        <Line
          {...props}
          points={projectStrokePoints(object.points, viewport, delta)}
        />
        {object.type === "arrow" ? (
          <Line
            {...props}
            points={projectStrokePoints(
              getCanvasArrowHeadPoints(object),
              viewport,
              delta,
            )}
          />
        ) : null}
      </Group>
    );
  }
  if (
    object.type === "rectangle" ||
    object.type === "ellipse" ||
    object.type === "text"
  ) {
    const common = {
      id: object.id,
      name: "canvas-object",
      x: (object.x + delta.dx) * viewport.scale + viewport.x,
      y: (object.y + delta.dy) * viewport.scale + viewport.y,
      width: object.width * viewport.scale,
      height: object.height * viewport.scale,
      rotation: object.rotation,
      opacity: object.opacity,
    };
    if (object.type === "text")
      return (
        <Shape
          key={object.id}
          {...common}
          sceneFunc={(context) => {
            context.setAttr(
              "font",
              `${object.fontSize * viewport.scale}px sans-serif`,
            );
            context.setAttr("fillStyle", object.color);
            context.setAttr("textBaseline", "top");
            object.text
              .split("\n")
              .forEach((line, index) =>
                context.fillText(
                  line,
                  0,
                  index * object.fontSize * viewport.scale * 1.2,
                ),
              );
          }}
          hitFunc={(context, shape) => {
            context.beginPath();
            context.rect(0, 0, common.width, common.height);
            context.closePath();
            context.fillStrokeShape(shape);
          }}
        />
      );
    if (object.type === "ellipse")
      return (
        <Group key={object.id} {...common}>
          <Ellipse
            x={common.width / 2}
            y={common.height / 2}
            radiusX={common.width / 2}
            radiusY={common.height / 2}
            stroke={object.color}
            strokeWidth={object.size * viewport.scale}
            fill={object.fill}
          />
        </Group>
      );
    return (
      <Rect
        key={object.id}
        {...common}
        stroke={object.color}
        strokeWidth={object.size * viewport.scale}
        fill={object.fill}
      />
    );
  }
  if (object.type !== "image") return null;
  const image = loadedImages[object.image_path];
  if (!image) {
    return null;
  }

  return (
    <KonvaImage
      key={object.id}
      id={object.id}
      name="canvas-object"
      ref={(node) => {
        imageNodeRefs.current[object.id] = node;
      }}
      image={image}
      x={
        (object.x + (isSelected ? selectionPreviewDelta.dx : 0)) *
          viewport.scale +
        viewport.x
      }
      y={
        (object.y + (isSelected ? selectionPreviewDelta.dy : 0)) *
          viewport.scale +
        viewport.y
      }
      width={object.width * viewport.scale}
      height={object.height * viewport.scale}
      rotation={object.rotation}
      opacity={object.opacity}
      draggable={false}
      onTransformEnd={(event) => {
        const node = event.target;
        const scaleX = node.scaleX();
        const scaleY = node.scaleY();
        const position = screenPointToCanvasPoint(node.position(), viewport);

        node.scaleX(1);
        node.scaleY(1);
        onTransformImage(object.id, {
          x: position.x,
          y: position.y,
          width: Math.max(8, (node.width() * scaleX) / viewport.scale),
          height: Math.max(8, (node.height() * scaleY) / viewport.scale),
          rotation: node.rotation(),
        });
      }}
    />
  );
}

function projectStrokePoints(
  points: number[],
  viewport: CanvasViewport,
  delta: { dx: number; dy: number } = { dx: 0, dy: 0 },
) {
  const projected: number[] = [];
  for (let index = 0; index < points.length; index += 2) {
    projected.push((points[index] + delta.dx) * viewport.scale + viewport.x);
    projected.push(
      (points[index + 1] + delta.dy) * viewport.scale + viewport.y,
    );
  }
  return projected;
}

function getSelectionKey(objectIds: string[]) {
  return JSON.stringify(objectIds);
}
