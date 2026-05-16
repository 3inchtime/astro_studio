import { useEffect, useMemo, useRef, useState } from "react";
import type { MutableRefObject } from "react";
import {
  Group,
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
import {
  frameToScreenRect,
  isSecondaryButtonPan,
  panViewportFromPointerDelta,
  screenPointToCanvasPoint,
  zoomViewportAtScreenPoint,
} from "../../lib/canvas/frame";
import type {
  CanvasDocumentContent,
  CanvasLayer,
  CanvasObject,
  CanvasStrokeObject,
  CanvasTool,
  CanvasViewport,
} from "../../types";

interface CanvasStageProps {
  content: CanvasDocumentContent;
  activeLayerId: string | null;
  activeTool: CanvasTool;
  strokeColor: string;
  strokeSize: number;
  onViewportChange: (viewport: CanvasViewport) => void;
  onAddStroke: (stroke: CanvasStrokeObject) => void;
  onTransformImage: (
    objectId: string,
    transform: { x: number; y: number; width: number; height: number; rotation?: number },
  ) => void;
  onResetImageAspect: (objectId: string) => void;
  onExport: () => Promise<string>;
}

interface LoadedImageMap {
  [path: string]: HTMLImageElement;
}

const CANVAS_WORLD_SIZE = 6000;

export default function CanvasStage({
  content,
  activeLayerId,
  activeTool,
  strokeColor,
  strokeSize,
  onViewportChange,
  onAddStroke,
  onTransformImage,
  onResetImageAspect,
}: CanvasStageProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const stageRef = useRef<Konva.Stage | null>(null);
  const transformerRef = useRef<Konva.Transformer | null>(null);
  const imageNodeRefs = useRef<Record<string, Konva.Image | null>>({});
  const draftStrokeRef = useRef<CanvasStrokeObject | null>(null);
  const isPointerDrawingRef = useRef(false);
  const panAnchorRef = useRef<{
    pointer: { x: number; y: number };
    viewport: CanvasViewport;
  } | null>(null);
  const [stageSize, setStageSize] = useState({ width: 960, height: 640 });
  const [loadedImages, setLoadedImages] = useState<LoadedImageMap>({});
  const [selectedObjectId, setSelectedObjectId] = useState<string | null>(null);
  const [, rerenderTick] = useState(0);

  useEffect(() => {
    if (!containerRef.current) {
      return;
    }

    const resizeObserver = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) {
        return;
      }
      setStageSize({
        width: Math.max(320, Math.round(entry.contentRect.width)),
        height: Math.max(320, Math.round(entry.contentRect.height)),
      });
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
      image.onload = () => {
        setLoadedImages((current) => ({ ...current, [path]: image }));
      };
      image.src = toAssetUrl(path);
    });
  }, [imagePaths, loadedImages]);

  const frameRect = frameToScreenRect(content.frame, content.viewport);
  const selectedImageObject = useMemo(() => {
    for (const layer of content.layers) {
      const object = layer.objects.find(
        (entry) => entry.type === "image" && entry.id === selectedObjectId,
      );
      if (object?.type === "image") {
        return object;
      }
    }
    return null;
  }, [content.layers, selectedObjectId]);
  const selectedImageRect = selectedImageObject
    ? frameToScreenRect(
        {
          x: selectedImageObject.x,
          y: selectedImageObject.y,
          width: selectedImageObject.width,
          height: selectedImageObject.height,
          aspect: "free",
        },
        content.viewport,
      )
    : null;
  const backgroundGrid = [];
  for (let position = -CANVAS_WORLD_SIZE; position <= CANVAS_WORLD_SIZE; position += 128) {
    backgroundGrid.push(position);
  }

  function getCanvasPoint() {
    const pointer = stageRef.current?.getPointerPosition();
    if (!pointer) {
      return null;
    }
    return screenPointToCanvasPoint(pointer, content.viewport);
  }

  function handlePointerDown(event: Konva.KonvaEventObject<MouseEvent | TouchEvent>) {
    if ("button" in event.evt && isSecondaryButtonPan(event.evt.button)) {
      event.evt.preventDefault();
      const pointer = stageRef.current?.getPointerPosition();
      if (!pointer) {
        return;
      }
      setSelectedObjectId(null);
      isPointerDrawingRef.current = false;
      draftStrokeRef.current = null;
      panAnchorRef.current = {
        pointer,
        viewport: content.viewport,
      };
      return;
    }

    if (activeTool !== "select") {
      setSelectedObjectId(null);
    }

    if (activeTool === "select" && event.target === event.target.getStage()) {
      setSelectedObjectId(null);
    }

    if (activeTool === "pan") {
      const pointer = stageRef.current?.getPointerPosition();
      if (!pointer) {
        return;
      }
      panAnchorRef.current = {
        pointer,
        viewport: content.viewport,
      };
      return;
    }

    if (activeTool !== "brush" && activeTool !== "eraser") {
      return;
    }

    const layer = content.layers.find((entry) => entry.id === activeLayerId) ?? content.layers[0];
    if (!layer || layer.locked || !layer.visible) {
      return;
    }

    const point = getCanvasPoint();
    if (!point) {
      return;
    }

    isPointerDrawingRef.current = true;
    draftStrokeRef.current = {
      type: "stroke",
      id: crypto.randomUUID(),
      tool: activeTool,
      points: [point.x, point.y],
      color: activeTool === "eraser" ? "#000000" : strokeColor,
      size: strokeSize,
      opacity: 1,
    };
    rerenderTick((value) => value + 1);
  }

  function handlePointerMove() {
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
    panAnchorRef.current = null;

    if (!isPointerDrawingRef.current || !draftStrokeRef.current) {
      return;
    }

    isPointerDrawingRef.current = false;
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

    const zoomFactor = event.evt.deltaY > 0 ? 0.92 : 1.08;
    onViewportChange(
      zoomViewportAtScreenPoint(content.viewport, pointer, content.viewport.scale * zoomFactor),
    );
  }

  useEffect(() => {
    const transformer = transformerRef.current;
    if (!transformer || !selectedObjectId) {
      transformer?.nodes([]);
      return;
    }

    const node = imageNodeRefs.current[selectedObjectId];
    transformer.nodes(node ? [node] : []);
    transformer.getLayer()?.batchDraw();
  }, [content.layers, selectedObjectId]);

  useEffect(() => {
    if (activeTool !== "select") {
      setSelectedObjectId(null);
    }
  }, [activeTool]);

  return (
    <div
      ref={containerRef}
      className="relative h-full min-h-0 flex-1 overflow-hidden"
      onContextMenu={(event) => event.preventDefault()}
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
        onWheel={handleWheel}
        className="relative z-0"
      >
        <Layer listening={false}>
          {backgroundGrid.map((position) => (
            <Line
              key={`v-${position}`}
              points={[
                position * content.viewport.scale + content.viewport.x,
                -CANVAS_WORLD_SIZE * content.viewport.scale + content.viewport.y,
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
                -CANVAS_WORLD_SIZE * content.viewport.scale + content.viewport.x,
                position * content.viewport.scale + content.viewport.y,
                CANVAS_WORLD_SIZE * content.viewport.scale + content.viewport.x,
                position * content.viewport.scale + content.viewport.y,
              ]}
              stroke="rgba(95,88,74,0.06)"
              strokeWidth={1}
            />
          ))}
        </Layer>

        <Layer>
          {content.layers.map((layer) => (
            <Group key={layer.id} visible={layer.visible}>
              {layer.objects.map((object) =>
                renderCanvasObject(
                  object,
                  content.viewport,
                  loadedImages,
                  activeTool,
                  layer,
                  selectedObjectId,
                  imageNodeRefs,
                  setSelectedObjectId,
                  onTransformImage,
                ),
              )}
            </Group>
          ))}

          {draftStrokeRef.current ? (
            <Line
              points={projectStrokePoints(draftStrokeRef.current.points, content.viewport)}
              stroke={draftStrokeRef.current.color}
              strokeWidth={draftStrokeRef.current.size * content.viewport.scale}
              opacity={draftStrokeRef.current.opacity}
              lineCap="round"
              lineJoin="round"
              tension={0.3}
              globalCompositeOperation={
                draftStrokeRef.current.tool === "eraser" ? "destination-out" : "source-over"
              }
            />
          ) : null}

          <Rect
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
          />

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
  activeTool: CanvasTool,
  layer: CanvasLayer,
  selectedObjectId: string | null,
  imageNodeRefs: MutableRefObject<Record<string, Konva.Image | null>>,
  setSelectedObjectId: (objectId: string | null) => void,
  onTransformImage: (
    objectId: string,
    transform: { x: number; y: number; width: number; height: number; rotation?: number },
  ) => void,
) {
  if (object.type === "stroke") {
    return (
      <Line
        key={object.id}
        points={projectStrokePoints(object.points, viewport)}
        stroke={object.color}
        strokeWidth={object.size * viewport.scale}
        opacity={object.opacity}
        lineCap="round"
        lineJoin="round"
        tension={0.3}
        globalCompositeOperation={object.tool === "eraser" ? "destination-out" : "source-over"}
      />
    );
  }

  const image = loadedImages[object.image_path];
  if (!image) {
    return null;
  }

  return (
    <KonvaImage
      key={object.id}
      ref={(node) => {
        imageNodeRefs.current[object.id] = node;
      }}
      image={image}
      x={object.x * viewport.scale + viewport.x}
      y={object.y * viewport.scale + viewport.y}
      width={object.width * viewport.scale}
      height={object.height * viewport.scale}
      rotation={object.rotation}
      opacity={object.opacity}
      draggable={activeTool === "select" && !layer.locked}
      stroke={selectedObjectId === object.id ? "#4f6aff" : undefined}
      strokeWidth={selectedObjectId === object.id ? 1.5 : 0}
      onClick={(event) => {
        event.cancelBubble = true;
        if (activeTool === "select" && !layer.locked) {
          setSelectedObjectId(object.id);
        }
      }}
      onTap={(event) => {
        event.cancelBubble = true;
        if (activeTool === "select" && !layer.locked) {
          setSelectedObjectId(object.id);
        }
      }}
      onDragEnd={(event) => {
        const position = screenPointToCanvasPoint(event.target.position(), viewport);
        onTransformImage(object.id, {
          x: position.x,
          y: position.y,
          width: object.width,
          height: object.height,
          rotation: object.rotation,
        });
      }}
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

function projectStrokePoints(points: number[], viewport: CanvasViewport) {
  const projected: number[] = [];
  for (let index = 0; index < points.length; index += 2) {
    projected.push(points[index] * viewport.scale + viewport.x);
    projected.push(points[index + 1] * viewport.scale + viewport.y);
  }
  return projected;
}
