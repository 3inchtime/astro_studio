import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { useState } from "react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CanvasDocumentContent, CanvasTool } from "../../types";
import CanvasStage from "./CanvasStage";

const konva = vi.hoisted(() => {
  type Point = { x: number; y: number };
  type KonvaProps = Record<string, unknown>;

  class FakeImageNode {
    readonly id: string;
    props: KonvaProps = {};
    private xValue = 0;
    private yValue = 0;
    private widthValue = 0;
    private heightValue = 0;
    private rotationValue = 0;
    private scaleXValue = 1;
    private scaleYValue = 1;

    constructor(id: string) {
      this.id = id;
    }

    update(props: KonvaProps) {
      this.props = props;
      this.xValue = Number(props.x ?? 0);
      this.yValue = Number(props.y ?? 0);
      this.widthValue = Number(props.width ?? 0);
      this.heightValue = Number(props.height ?? 0);
      this.rotationValue = Number(props.rotation ?? 0);
      this.scaleXValue = 1;
      this.scaleYValue = 1;
    }

    position(next?: Point) {
      if (next) {
        this.xValue = next.x;
        this.yValue = next.y;
      }
      return { x: this.xValue, y: this.yValue };
    }

    width(next?: number) {
      if (next !== undefined) this.widthValue = next;
      return this.widthValue;
    }

    height(next?: number) {
      if (next !== undefined) this.heightValue = next;
      return this.heightValue;
    }

    rotation(next?: number) {
      if (next !== undefined) this.rotationValue = next;
      return this.rotationValue;
    }

    scaleX(next?: number) {
      if (next !== undefined) this.scaleXValue = next;
      return this.scaleXValue;
    }

    scaleY(next?: number) {
      if (next !== undefined) this.scaleYValue = next;
      return this.scaleYValue;
    }
  }

  class FakeTransformer {
    attachedNodes: unknown[] = [];
    readonly batchDraw = vi.fn();
    readonly nodes = vi.fn((next?: unknown[]) => {
      if (next) this.attachedNodes = next;
      return this.attachedNodes;
    });
    readonly getLayer = vi.fn(() => ({ batchDraw: this.batchDraw }));

    getClassName() {
      return "Transformer";
    }
  }

  const transformer = new FakeTransformer();

  const harness = {
    pointer: null as Point | null,
    stageProps: null as KonvaProps | null,
    resizeCallback: null as ResizeObserverCallback | null,
    imageProps: new Map<string, KonvaProps>(),
    shapeProps: new Map<string, KonvaProps>(),
    imageNodes: new Map<string, FakeImageNode>(),
    transformer,
    transformerParent: transformer,
    toAssetUrl: vi.fn((path: string) => `asset:${path}`),
  };

  const stageNode = {
    getPointerPosition: () => harness.pointer,
    getParent: () => null,
    getStage: () => stageNode,
  };

  return {
    get pointer() {
      return harness.pointer;
    },
    set pointer(value: Point | null) {
      harness.pointer = value;
    },
    get stageProps() {
      return harness.stageProps;
    },
    set stageProps(value: KonvaProps | null) {
      harness.stageProps = value;
    },
    get resizeCallback() {
      return harness.resizeCallback;
    },
    set resizeCallback(value: ResizeObserverCallback | null) {
      harness.resizeCallback = value;
    },
    imageProps: harness.imageProps,
    shapeProps: harness.shapeProps,
    imageNodes: harness.imageNodes,
    transformer: harness.transformer,
    transformerParent: harness.transformerParent,
    toAssetUrl: harness.toAssetUrl,
    stageNode,
    createImageNode(id: string) {
      return new FakeImageNode(id);
    },
    reset() {
      harness.pointer = null;
      harness.stageProps = null;
      harness.resizeCallback = null;
      harness.imageProps.clear();
      harness.shapeProps.clear();
      harness.imageNodes.clear();
      harness.transformer.attachedNodes = [];
      harness.transformer.nodes.mockClear();
      harness.transformer.getLayer.mockClear();
      harness.transformer.batchDraw.mockClear();
      harness.toAssetUrl.mockClear();
    },
  };
});

vi.mock("react-konva", async () => {
  const React = await vi.importActual<typeof import("react")>("react");

  function domValue(value: unknown) {
    if (Array.isArray(value)) return JSON.stringify(value);
    if (typeof value === "boolean") return String(value);
    if (typeof value === "number" || typeof value === "string") return value;
    return undefined;
  }

  function primitive(kind: "group" | "layer" | "line" | "rect") {
    return function KonvaPrimitive(props: Record<string, unknown>) {
      return React.createElement(
        "div",
        {
          id: typeof props.id === "string" ? props.id : undefined,
          "data-konva-kind": kind,
          "data-name": domValue(props.name),
          "data-x": domValue(props.x),
          "data-y": domValue(props.y),
          "data-width": domValue(props.width),
          "data-height": domValue(props.height),
          "data-points": domValue(props.points),
          "data-visible": domValue(props.visible),
          "data-listening": domValue(props.listening),
        },
        props.children as React.ReactNode,
      );
    };
  }

  const Stage = React.forwardRef(function MockStage(
    props: Record<string, unknown>,
    ref: React.ForwardedRef<unknown>,
  ) {
    konva.stageProps = props;
    React.useImperativeHandle(ref, () => konva.stageNode);
    return React.createElement(
      "div",
      {
        "data-testid": "konva-stage",
        "data-width": domValue(props.width),
        "data-height": domValue(props.height),
      },
      props.children as React.ReactNode,
    );
  });

  const Image = React.forwardRef(function MockImage(
    props: Record<string, unknown>,
    ref: React.ForwardedRef<unknown>,
  ) {
    const id = String(props.id ?? "missing-image-id");
    const node = konva.imageNodes.get(id) ?? konva.createImageNode(id);

    konva.imageNodes.set(id, node);
    konva.imageProps.set(id, props);
    node.update(props);
    React.useImperativeHandle(ref, () => node);

    return React.createElement("div", {
      id,
      "data-konva-kind": "image",
      "data-name": domValue(props.name),
      "data-x": domValue(props.x),
      "data-y": domValue(props.y),
      "data-width": domValue(props.width),
      "data-height": domValue(props.height),
      "data-draggable": domValue(props.draggable),
    });
  });

  const Transformer = React.forwardRef(function MockTransformer(
    props: Record<string, unknown>,
    ref: React.ForwardedRef<unknown>,
  ) {
    React.useImperativeHandle(ref, () => konva.transformer);
    return React.createElement("div", {
      "data-konva-kind": "transformer",
      "data-name": domValue(props.name),
    });
  });

  return {
    Stage,
    Image,
    Transformer,
    Group: primitive("group"),
    Layer: primitive("layer"),
    Line: primitive("line"),
    Shape: (props: Record<string, unknown>) => {
      konva.shapeProps.set(String(props.id), props);
      return React.createElement(primitive("rect"), props);
    },
    Rect: primitive("rect"),
    Ellipse: primitive("rect"),
    Arrow: primitive("line"),
    Text: primitive("rect"),
  };
});

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "canvas.emptyStateHint": "Use the toolbar to draw and select.",
        "canvas.resetImageAspect": "Reset aspect",
      })[key] ?? key,
  }),
}));

vi.mock("../../lib/api", () => ({
  toAssetUrl: konva.toAssetUrl,
}));

type CanvasStageProps = ComponentProps<typeof CanvasStage>;
type StageHandlerName =
  | "onMouseDown"
  | "onMousemove"
  | "onMouseup"
  | "onMouseleave"
  | "onTouchStart"
  | "onTouchMove"
  | "onTouchEnd"
  | "onTouchCancel"
  | "onWheel";

const callbacks = {
  onViewportChange: vi.fn(),
  onAddStroke: vi.fn(),
  onTransformImage: vi.fn(),
  onResetImageAspect: vi.fn(),
  onSelectionChange: vi.fn(),
  onMoveSelection: vi.fn(),
  onStageSizeChange: vi.fn(),
  onExport: vi.fn(async () => "/tmp/export.png"),
};

function createContent(
  overrides: Partial<CanvasDocumentContent> = {},
): CanvasDocumentContent {
  return {
    version: 1,
    viewport: { x: 0, y: 0, scale: 1 },
    frame: { x: 0, y: 0, width: 512, height: 512, aspect: "1:1" },
    layers: [
      {
        id: "top-layer",
        name: "Top",
        visible: true,
        locked: false,
        objects: [
          {
            type: "image",
            id: "image-top",
            image_path: "/tmp/top.png",
            x: 20,
            y: 20,
            width: 80,
            height: 60,
            original_width: 160,
            original_height: 120,
            rotation: 0,
            opacity: 1,
          },
          {
            type: "image",
            id: "image-second",
            image_path: "/tmp/second.png",
            x: 140,
            y: 20,
            width: 60,
            height: 60,
            original_width: 120,
            original_height: 120,
            rotation: 0,
            opacity: 1,
          },
          {
            type: "stroke",
            id: "stroke-1",
            tool: "brush",
            points: [150, 130, 190, 160],
            color: "#5f584a",
            size: 10,
            opacity: 1,
          },
        ],
      },
      {
        id: "bottom-layer",
        name: "Bottom",
        visible: true,
        locked: false,
        objects: [
          {
            type: "image",
            id: "image-bottom",
            image_path: "/tmp/bottom.png",
            x: 20,
            y: 20,
            width: 80,
            height: 60,
            original_width: 80,
            original_height: 60,
            rotation: 0,
            opacity: 1,
          },
        ],
      },
      {
        id: "locked-layer",
        name: "Locked",
        visible: true,
        locked: true,
        objects: [
          {
            type: "image",
            id: "locked-image",
            image_path: "/tmp/locked.png",
            x: 0,
            y: 0,
            width: 300,
            height: 300,
            original_width: 300,
            original_height: 300,
            rotation: 0,
            opacity: 1,
          },
        ],
      },
      {
        id: "hidden-layer",
        name: "Hidden",
        visible: false,
        locked: false,
        objects: [
          {
            type: "image",
            id: "hidden-image",
            image_path: "/tmp/hidden.png",
            x: 130,
            y: 0,
            width: 100,
            height: 190,
            original_width: 100,
            original_height: 190,
            rotation: 0,
            opacity: 1,
          },
        ],
      },
    ],
    ...overrides,
  };
}

function baseProps(
  overrides: Partial<CanvasStageProps> = {},
): CanvasStageProps {
  return {
    content: createContent(),
    activeLayerId: "top-layer",
    activeTool: "select",
    selectedObjectIds: [],
    strokeColor: "#201f1b",
    strokeSize: 8,
    ...callbacks,
    ...overrides,
  };
}

function ControlledStage({
  selectedObjectIds: initialSelectedObjectIds,
  onSelectionChange,
  ...props
}: CanvasStageProps) {
  const [selectedObjectIds, setSelectedObjectIds] = useState(
    initialSelectedObjectIds,
  );

  return (
    <CanvasStage
      {...props}
      selectedObjectIds={selectedObjectIds}
      onSelectionChange={(nextIds) => {
        onSelectionChange(nextIds);
        setSelectedObjectIds(nextIds);
      }}
    />
  );
}

function ViewportControlledStage() {
  const [content, setContent] = useState(() =>
    createContent({ viewport: { x: 10, y: 20, scale: 1 } }),
  );

  return (
    <CanvasStage
      {...baseProps({ content })}
      onViewportChange={(nextViewport) => {
        callbacks.onViewportChange(nextViewport);
        setContent((current) => ({ ...current, viewport: nextViewport }));
      }}
    />
  );
}

function renderStage(overrides: Partial<CanvasStageProps> = {}) {
  return render(<ControlledStage {...baseProps(overrides)} />);
}

function getStageHandler(name: StageHandlerName) {
  const handler = konva.stageProps?.[name];
  if (typeof handler !== "function") {
    throw new Error(`Missing Stage handler ${name}`);
  }
  return handler as (event?: unknown) => void;
}

function setPointer(x: number, y: number) {
  konva.pointer = { x, y };
}

function createPointerEvent({
  button = 0,
  shiftKey = false,
  target = konva.stageNode,
}: {
  button?: number;
  shiftKey?: boolean;
  target?: unknown;
} = {}) {
  return {
    evt: {
      button,
      shiftKey,
      preventDefault: vi.fn(),
    },
    target,
    cancelBubble: false,
  };
}

function pointerDown(
  x: number,
  y: number,
  options: Parameters<typeof createPointerEvent>[0] = {},
) {
  setPointer(x, y);
  getStageHandler("onMouseDown")(createPointerEvent(options));
}

function pointerMove(x: number, y: number) {
  setPointer(x, y);
  getStageHandler("onMousemove")();
}

function pointerUp() {
  getStageHandler("onMouseup")();
}

function pointerCancel() {
  getStageHandler("onTouchCancel")();
}

function clearPointer() {
  konva.pointer = null;
}

function konvaNodeByName(name: string) {
  return document.querySelector<HTMLElement>(`[data-name="${name}"]`);
}

function objectNode(id: string) {
  const node = document.getElementById(id);
  if (!node) throw new Error(`Missing rendered canvas object ${id}`);
  return node;
}

function lineNodeWithPoints(points: number[]) {
  return [
    ...document.querySelectorAll<HTMLElement>('[data-konva-kind="line"]'),
  ].find((node) => node.dataset.points === JSON.stringify(points));
}

function dispatchSpace(
  target: EventTarget,
  type: "keydown" | "keyup" = "keydown",
) {
  const event = new KeyboardEvent(type, {
    key: " ",
    code: "Space",
    bubbles: true,
    cancelable: true,
  });
  target.dispatchEvent(event);
  return event;
}

function triggerResize(width: number, height: number) {
  const callback = konva.resizeCallback;
  if (!callback) throw new Error("ResizeObserver was not created");
  callback(
    [{ contentRect: { width, height } } as ResizeObserverEntry],
    {} as ResizeObserver,
  );
}

class ControlledResizeObserver implements ResizeObserver {
  constructor(callback: ResizeObserverCallback) {
    konva.resizeCallback = callback;
  }

  readonly observe = vi.fn();
  readonly unobserve = vi.fn();
  readonly disconnect = vi.fn();
}

class ControlledWindowImage {
  onload: ((this: GlobalEventHandlers, event: Event) => unknown) | null = null;
  private source = "";

  set src(value: string) {
    this.source = value;
    this.onload?.call(
      this as unknown as GlobalEventHandlers,
      new Event("load"),
    );
  }

  get src() {
    return this.source;
  }
}

describe("CanvasStage", () => {
  beforeEach(() => {
    konva.reset();
    Object.values(callbacks).forEach((callback) => callback.mockClear());
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
      font: "",
      measureText: (text: string) => ({
        width: [...text].reduce(
          (sum, char) => sum + (char.charCodeAt(0) > 255 ? 24 : 12),
          0,
        ),
      }),
    } as unknown as CanvasRenderingContext2D);
    vi.stubGlobal("ResizeObserver", ControlledResizeObserver);
    vi.stubGlobal("Image", ControlledWindowImage);
  });

  afterEach(() => {
    cleanup();
    document.body.replaceChildren();
    vi.unstubAllGlobals();
  });

  it("rounds, clamps, reports, and renders the same observed stage size", () => {
    renderStage();

    act(() => triggerResize(799.6, 280.2));

    expect(callbacks.onStageSizeChange).toHaveBeenLastCalledWith({
      width: 800,
      height: 320,
    });
    expect(screen.getByTestId("konva-stage")).toHaveAttribute(
      "data-width",
      "800",
    );
    expect(screen.getByTestId("konva-stage")).toHaveAttribute(
      "data-height",
      "320",
    );
  });

  it("selects the topmost visible unlocked hit and clears on an empty click", () => {
    renderStage({ selectedObjectIds: ["image-second"] });

    act(() => {
      pointerDown(30, 30);
      pointerUp();
    });
    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith(["image-top"]);

    act(() => {
      pointerDown(450, 450);
      pointerUp();
    });
    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith([]);
  });

  it("shift-click adds and removes exact ids without arming movement", () => {
    renderStage({ selectedObjectIds: ["image-top"] });

    act(() => {
      pointerDown(150, 30, { shiftKey: true });
      pointerMove(180, 60);
      pointerUp();
    });
    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith([
      "image-top",
      "image-second",
    ]);
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();

    act(() => {
      pointerDown(150, 30, { shiftKey: true });
      pointerMove(190, 70);
      pointerUp();
    });
    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith(["image-top"]);
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("uses the synchronous marquee rect when down, move, and up are batched", () => {
    renderStage();

    act(() => {
      pointerDown(130, 10);
      pointerMove(210, 180);
      pointerUp();
    });

    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith([
      "image-second",
      "stroke-1",
    ]);
  });

  it("projects marquee chrome to screen coordinates and removes it on completion", () => {
    renderStage({
      content: createContent({ viewport: { x: 100, y: 50, scale: 2 } }),
    });

    act(() => pointerDown(120, 70));
    act(() => pointerMove(180, 130));

    const marquee = konvaNodeByName("canvas-marquee");
    expect(marquee).toHaveAttribute("data-x", "120");
    expect(marquee).toHaveAttribute("data-y", "70");
    expect(marquee).toHaveAttribute("data-width", "60");
    expect(marquee).toHaveAttribute("data-height", "60");
    expect(marquee).toHaveAttribute("data-listening", "false");

    act(() => pointerUp());
    expect(konvaNodeByName("canvas-marquee")).toBeNull();
  });

  it("previews every selected object locally and commits one total group delta", () => {
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));

    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");
    expect(objectNode("image-top")).toHaveAttribute("data-y", "35");
    expect(objectNode("stroke-1")).toHaveAttribute(
      "data-points",
      JSON.stringify([170, 145, 210, 175]),
    );
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
    expect(callbacks.onTransformImage).not.toHaveBeenCalled();

    act(() => pointerUp());

    expect(callbacks.onMoveSelection).toHaveBeenCalledTimes(1);
    expect(callbacks.onMoveSelection).toHaveBeenCalledWith({ dx: 20, dy: 15 });
    expect(callbacks.onTransformImage).not.toHaveBeenCalled();
  });

  it("commits the synchronous preview delta when a batched release loses its pointer", () => {
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });

    act(() => {
      pointerDown(30, 30);
      pointerMove(50, 45);
      clearPointer();
      pointerUp();
    });

    expect(callbacks.onMoveSelection).toHaveBeenCalledTimes(1);
    expect(callbacks.onMoveSelection).toHaveBeenCalledWith({ dx: 20, dy: 15 });
  });

  it("cancels a group drag on window blur without committing its preview", () => {
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");

    act(() => window.dispatchEvent(new Event("blur")));

    expect(objectNode("image-top")).toHaveAttribute("data-x", "20");
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("cancels a group drag on touch cancel without committing its preview", () => {
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");

    act(() => pointerCancel());

    expect(objectNode("image-top")).toHaveAttribute("data-x", "20");
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("cancels a drag when the external selection changes before release", () => {
    const content = createContent();
    const props = baseProps({
      content,
      selectedObjectIds: ["image-top", "stroke-1"],
    });
    const view = render(<CanvasStage {...props} />);

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");

    view.rerender(
      <CanvasStage {...props} selectedObjectIds={["image-second"]} />,
    );

    expect(objectNode("image-second")).toHaveAttribute("data-x", "140");
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("cancels a drag when content identity changes before release", () => {
    const content = createContent();
    const props = baseProps({
      content,
      selectedObjectIds: ["image-top", "stroke-1"],
    });
    const view = render(<CanvasStage {...props} />);

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");

    view.rerender(<CanvasStage {...props} content={{ ...content }} />);

    expect(objectNode("image-top")).toHaveAttribute("data-x", "20");
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("does not self-cancel when pointer-down selects the object being dragged", () => {
    renderStage({ selectedObjectIds: ["stroke-1"] });

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));

    expect(objectNode("image-top")).toHaveAttribute("data-x", "40");
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).toHaveBeenCalledWith({ dx: 20, dy: 15 });
  });

  it("cancels marquee state when content changes before release", () => {
    const content = createContent();
    const props = baseProps({ content });
    const view = render(<CanvasStage {...props} />);

    act(() => pointerDown(300, 300));
    act(() => pointerMove(400, 400));
    expect(konvaNodeByName("canvas-marquee")).toBeInTheDocument();
    callbacks.onSelectionChange.mockClear();

    view.rerender(<CanvasStage {...props} content={{ ...content }} />);

    expect(konvaNodeByName("canvas-marquee")).toBeNull();
    act(() => pointerUp());
    expect(callbacks.onSelectionChange).not.toHaveBeenCalled();
  });

  it("cancels a draft stroke when content changes before release", () => {
    const content = createContent();
    const props = baseProps({ content, activeTool: "brush" });
    const view = render(<CanvasStage {...props} />);

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(lineNodeWithPoints([30, 30, 50, 45])).toBeInTheDocument();

    view.rerender(<CanvasStage {...props} content={{ ...content }} />);

    expect(lineNodeWithPoints([30, 30, 50, 45])).toBeUndefined();
    act(() => pointerUp());
    expect(callbacks.onAddStroke).not.toHaveBeenCalled();
  });

  it("cancels a draft stroke on blur without adding it on release", () => {
    renderStage({ activeTool: "brush" });

    act(() => pointerDown(30, 30));
    act(() => pointerMove(50, 45));
    expect(lineNodeWithPoints([30, 30, 50, 45])).toBeInTheDocument();

    act(() => window.dispatchEvent(new Event("blur")));

    expect(lineNodeWithPoints([30, 30, 50, 45])).toBeUndefined();
    act(() => pointerUp());
    expect(callbacks.onAddStroke).not.toHaveBeenCalled();
  });

  it("gates temporary Space pan for typing targets and resets it on keyup and blur", () => {
    renderStage({
      content: createContent({ viewport: { x: 10, y: 20, scale: 1 } }),
      selectedObjectIds: ["image-top"],
    });

    const textarea = document.createElement("textarea");
    const editable = document.createElement("div");
    editable.contentEditable = "true";
    Object.defineProperty(editable, "isContentEditable", { value: true });
    document.body.append(textarea, editable);

    const textareaSpace = dispatchSpace(textarea);
    const editableSpace = dispatchSpace(editable);
    expect(textareaSpace.defaultPrevented).toBe(false);
    expect(editableSpace.defaultPrevented).toBe(false);

    const acceptedSpace = dispatchSpace(window);
    expect(acceptedSpace.defaultPrevented).toBe(true);

    act(() => {
      pointerDown(100, 100);
      pointerMove(110, 120);
      pointerMove(120, 130);
    });
    expect(callbacks.onViewportChange).toHaveBeenNthCalledWith(1, {
      x: 20,
      y: 40,
      scale: 1,
    });
    expect(callbacks.onViewportChange).toHaveBeenNthCalledWith(2, {
      x: 30,
      y: 50,
      scale: 1,
    });
    expect(callbacks.onSelectionChange).not.toHaveBeenCalled();

    act(() => dispatchSpace(window, "keyup"));
    act(() => pointerMove(130, 140));
    expect(callbacks.onViewportChange).toHaveBeenCalledTimes(2);

    act(() => {
      pointerUp();
      pointerDown(450, 450);
      pointerMove(460, 460);
      pointerUp();
    });
    expect(callbacks.onViewportChange).toHaveBeenCalledTimes(2);

    act(() => {
      dispatchSpace(window);
      window.dispatchEvent(new Event("blur"));
      pointerDown(450, 450);
      pointerMove(470, 470);
      pointerUp();
    });
    expect(callbacks.onViewportChange).toHaveBeenCalledTimes(2);
  });

  it("keeps an anchored pan active across its own viewport content rerenders", () => {
    render(<ViewportControlledStage />);

    act(() => dispatchSpace(window));
    act(() => {
      pointerDown(100, 100);
      pointerMove(110, 120);
    });
    expect(callbacks.onViewportChange).toHaveBeenLastCalledWith({
      x: 20,
      y: 40,
      scale: 1,
    });

    act(() => pointerMove(120, 130));
    expect(callbacks.onViewportChange).toHaveBeenLastCalledWith({
      x: 30,
      y: 50,
      scale: 1,
    });
    expect(callbacks.onViewportChange).toHaveBeenCalledTimes(2);

    act(() => {
      pointerUp();
      dispatchSpace(window, "keyup");
    });
  });

  it("attaches one external image to Transformer and preserves image controls", async () => {
    renderStage({ selectedObjectIds: ["image-top"] });

    await waitFor(() => {
      expect(konva.transformer.attachedNodes).toEqual([
        konva.imageNodes.get("image-top"),
      ]);
    });
    expect(
      screen.getByRole("button", { name: "Reset aspect" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Reset aspect" }));
    expect(callbacks.onResetImageAspect).toHaveBeenCalledWith("image-top");
  });

  it("uses bounds chrome without a transformer for a stroke or multi-selection", async () => {
    const { unmount } = renderStage({ selectedObjectIds: ["stroke-1"] });

    await waitFor(() => expect(konva.transformer.attachedNodes).toEqual([]));
    expect(konvaNodeByName("canvas-selection-outline")).toHaveAttribute(
      "data-listening",
      "false",
    );
    expect(screen.queryByRole("button", { name: "Reset aspect" })).toBeNull();

    unmount();
    konva.transformer.nodes.mockClear();
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });

    await waitFor(() => expect(konva.transformer.attachedNodes).toEqual([]));
    expect(konvaNodeByName("canvas-selection-outline")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reset aspect" })).toBeNull();
  });

  it.each(["hidden-image", "locked-image", "missing-image"])(
    "renders no chrome or transformer for invalid external selection %s",
    async (selectedId) => {
      renderStage({ selectedObjectIds: [selectedId] });

      await waitFor(() => expect(konva.transformer.attachedNodes).toEqual([]));
      expect(konvaNodeByName("canvas-selection-outline")).toBeNull();
      expect(screen.queryByRole("button", { name: "Reset aspect" })).toBeNull();
    },
  );

  it("ignores pointer-down from a Transformer anchor", () => {
    renderStage({ selectedObjectIds: ["image-top", "stroke-1"] });
    const anchorTarget = {
      getParent: () => konva.transformerParent,
      getStage: () => konva.stageNode,
    };

    act(() => {
      pointerDown(30, 30, { target: anchorTarget });
      pointerMove(70, 70);
      pointerUp();
    });

    expect(callbacks.onSelectionChange).not.toHaveBeenCalled();
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("clears controlled selection and pending previews when leaving select", () => {
    const props = baseProps({ selectedObjectIds: ["image-top", "stroke-1"] });
    const view = render(<ControlledStage {...props} />);

    act(() => pointerDown(30, 30));
    act(() => pointerMove(60, 50));
    expect(objectNode("image-top")).toHaveAttribute("data-x", "52.5");

    view.rerender(
      <ControlledStage {...props} activeTool={"brush" as CanvasTool} />,
    );

    expect(callbacks.onSelectionChange).toHaveBeenLastCalledWith([]);
    expect(konvaNodeByName("canvas-selection-outline")).toBeNull();
    act(() => pointerUp());
    expect(callbacks.onMoveSelection).not.toHaveBeenCalled();
  });

  it("adds stable ids and names for objects and non-listening canvas chrome", () => {
    renderStage({ selectedObjectIds: ["stroke-1"] });

    expect(objectNode("image-top")).toHaveAttribute("data-konva-kind", "image");
    expect(objectNode("stroke-1")).toHaveAttribute("data-konva-kind", "line");
    expect(konvaNodeByName("canvas-generation-frame")).toHaveAttribute(
      "data-konva-kind",
      "rect",
    );
    expect(konvaNodeByName("canvas-generation-frame")).toHaveAttribute(
      "data-listening",
      "false",
    );
  });

  describe("canvas editor operations", () => {
    it("pans ordinary wheel input instead of zooming", () => {
      renderStage();
      konva.pointer = { x: 100, y: 100 };
      act(() =>
        getStageHandler("onWheel")({
          evt: { deltaX: 12, deltaY: 20, preventDefault: vi.fn() },
        }),
      );
      expect(callbacks.onViewportChange).toHaveBeenCalledWith(
        expect.objectContaining({ scale: 1, x: -12, y: -20 }),
      );
    });
    it("creates a constrained rectangle in one transaction", () => {
      const onAddObject = vi.fn();
      renderStage({ activeTool: "rectangle", onAddObject });
      act(() => {
        pointerDown(10, 20);
        konva.pointer = { x: 80, y: 50 };
        getStageHandler("onMousemove")({ evt: { shiftKey: true } });
      });
      expect(onAddObject).not.toHaveBeenCalled();
      act(() => pointerUp());
      expect(onAddObject).toHaveBeenCalledWith(
        expect.objectContaining({
          type: "rectangle",
          x: 10,
          y: 20,
          width: 70,
          height: 70,
        }),
      );
    });
    it("edits native multiline text and commits with command Enter", () => {
      const onAddObject = vi.fn();
      renderStage({ activeTool: "text", onAddObject });
      act(() => pointerDown(20, 30));
      const editor = screen.getByRole("textbox");
      fireEvent.change(editor, { target: { value: "Hello\nworld" } });
      fireEvent.compositionStart(editor);
      fireEvent.keyDown(editor, { key: "Enter", metaKey: true });
      expect(onAddObject).not.toHaveBeenCalled();
      fireEvent.compositionEnd(editor);
      fireEvent.keyDown(editor, { key: "Enter", metaKey: true });
      expect(onAddObject).toHaveBeenCalledWith(
        expect.objectContaining({ type: "text", text: "Hello\nworld" }),
      );
      expect(callbacks.onAddStroke).not.toHaveBeenCalled();
    });
    it("cancels a shape preview on escape", () => {
      const onAddObject = vi.fn();
      renderStage({ activeTool: "ellipse", onAddObject });
      act(() => {
        pointerDown(10, 10);
        pointerMove(70, 60);
      });
      fireEvent.keyDown(window, { key: "Escape" });
      act(() => pointerUp());
      expect(onAddObject).not.toHaveBeenCalled();
    });
  });

  it("commits snapped final drag rather than the unsnapped pointer delta", () => {
    const content = createContent();
    content.layers[0].objects = [
      {
        type: "rectangle",
        id: "box",
        x: 20,
        y: 20,
        width: 40,
        height: 40,
        rotation: 0,
        fill: "transparent",
        color: "#000",
        size: 2,
        opacity: 1,
      },
    ];
    renderStage({ content, selectedObjectIds: ["box"] });
    act(() => {
      pointerDown(30, 30);
      pointerMove(13, 13);
      pointerUp();
    });
    expect(callbacks.onMoveSelection).toHaveBeenCalledWith({
      dx: -20,
      dy: -20,
    });
  });
  it("resizes the selection once and cancels an unfinished frame resize", () => {
    const content = createContent();
    content.layers[0].objects = [
      {
        type: "rectangle",
        id: "box",
        x: 20,
        y: 20,
        width: 40,
        height: 40,
        rotation: 0,
        fill: "transparent",
        color: "#000",
        size: 2,
        opacity: 1,
      },
    ];
    const onTransformSelection = vi.fn(),
      onFrameChange = vi.fn();
    const view = renderStage({
      content,
      selectedObjectIds: ["box"],
      onTransformSelection,
    });
    fireEvent.mouseDown(
      screen.getByRole("button", {
        name: "canvas.resizeSelection bottom-right",
      }),
      { clientX: 60, clientY: 60 },
    );
    fireEvent.mouseMove(window, { clientX: 100, clientY: 100 });
    expect(onTransformSelection).not.toHaveBeenCalled();
    fireEvent.mouseUp(window);
    expect(onTransformSelection).toHaveBeenCalledWith({
      x: 20,
      y: 20,
      width: 80,
      height: 80,
    });
    view.unmount();
    renderStage({ content, frameEditing: true, onFrameChange });
    fireEvent.mouseDown(
      screen.getByRole("button", { name: "canvas.resizeFrame bottom-right" }),
      { clientX: 60, clientY: 60 },
    );
    fireEvent.mouseMove(window, { clientX: 100, clientY: 100 });
    fireEvent.keyDown(window, { key: "Escape" });
    fireEvent.mouseUp(window);
    expect(onFrameChange).not.toHaveBeenCalled();
  });

  it("offers contextual selected object actions on right click without drawing", () => {
    const onCopySelection = vi.fn();
    renderStage({ onCopySelection });
    fireEvent.contextMenu(screen.getByTestId("konva-stage"), {
      clientX: 30,
      clientY: 30,
    });
    fireEvent.click(
      screen.getByRole("menuitem", { name: "canvas.copySelection" }),
    );
    expect(onCopySelection).toHaveBeenCalledOnce();
    expect(callbacks.onAddStroke).not.toHaveBeenCalled();
  });

  it("cancels text on window blur and on content switch", () => {
    const onAddObject = vi.fn();
    const props = baseProps({ activeTool: "text", onAddObject });
    const view = render(<ControlledStage {...props} />);
    act(() => pointerDown(20, 30));
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "discard" },
    });
    fireEvent.blur(window);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(onAddObject).not.toHaveBeenCalled();
    act(() => pointerDown(20, 30));
    view.rerender(
      <ControlledStage
        {...props}
        content={{ ...props.content, layers: [...props.content.layers] }}
      />,
    );
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
  it("renders arrow shafts and open heads in the same isolated document layer", () => {
    const content = createContent();
    content.layers[0].objects = [
      {
        type: "arrow",
        id: "arrow",
        points: [0, 0, 100, 0],
        color: "#000",
        size: 2,
        opacity: 1,
      },
    ];
    renderStage({ content });
    const arrow = document.getElementById("arrow");
    expect(arrow?.parentElement?.dataset.konvaKind).toBe("layer");
    expect(arrow?.querySelectorAll('[data-konva-kind="line"]')).toHaveLength(2);
  });
  it("renders text using the same explicit multiline fillText layout as export", () => {
    const content = createContent();
    content.layers[0].objects = [
      {
        type: "text",
        id: "label",
        x: 0,
        y: 0,
        width: 100,
        height: 10,
        rotation: 0,
        text: "Long first line\n第二行",
        fontSize: 20,
        color: "#123456",
        opacity: 1,
      },
    ];
    renderStage({ content });
    const context = { setAttr: vi.fn(), fillText: vi.fn() };
    const draw = konva.shapeProps.get("label")?.sceneFunc as (
      ctx: typeof context,
    ) => void;
    expect(draw).toBeTypeOf("function");
    draw(context);
    expect(context.fillText).toHaveBeenNthCalledWith(
      1,
      "Long first line",
      0,
      0,
    );
    expect(context.fillText).toHaveBeenNthCalledWith(2, "第二行", 0, 24);
  });
  it("right mouse down and drag never mutate drawing before the context menu opens", () => {
    const onAddObject = vi.fn(),
      onCopySelection = vi.fn();
    renderStage({ activeTool: "brush", onAddObject, onCopySelection });
    act(() => {
      pointerDown(30, 30, { button: 2 });
      pointerMove(70, 60);
      pointerUp();
    });
    expect(callbacks.onViewportChange).not.toHaveBeenCalled();
    expect(callbacks.onAddStroke).not.toHaveBeenCalled();
    expect(onAddObject).not.toHaveBeenCalled();
    fireEvent.contextMenu(screen.getByTestId("konva-stage"), {
      clientX: 30,
      clientY: 30,
    });
    expect(screen.getByRole("menu")).toBeInTheDocument();
  });
  it("measures Chinese text in the rendering font rather than estimating Latin character width", () => {
    const onAddObject = vi.fn();
    renderStage({ activeTool: "text", fontSize: 24, onAddObject });
    act(() => pointerDown(20, 30));
    const editor = screen.getByRole("textbox");
    expect(editor).toHaveAttribute("wrap", "off");
    expect(editor).toHaveStyle({ fontFamily: "sans-serif" });
    fireEvent.change(editor, { target: { value: "一二三四五六七八九十" } });
    fireEvent.keyDown(editor, { key: "Enter", ctrlKey: true });
    expect(onAddObject).toHaveBeenCalledWith(
      expect.objectContaining({ type: "text", width: 240, height: 24 * 1.2 }),
    );
  });
  it("discards unfinished frame resize when frame editing is closed and reopened", () => {
    const onFrameChange = vi.fn();
    const props = baseProps({ frameEditing: true, onFrameChange });
    const view = render(<ControlledStage {...props} />);
    fireEvent.mouseDown(
      screen.getByRole("button", { name: "canvas.resizeFrame bottom-right" }),
      { clientX: 512, clientY: 512 },
    );
    fireEvent.mouseMove(window, { clientX: 600, clientY: 600 });
    expect(konvaNodeByName("canvas-generation-frame")).toHaveAttribute(
      "data-width",
      "600",
    );
    view.rerender(<ControlledStage {...props} frameEditing={false} />);
    expect(konvaNodeByName("canvas-generation-frame")).toHaveAttribute(
      "data-width",
      "512",
    );
    view.rerender(<ControlledStage {...props} frameEditing />);
    fireEvent.mouseUp(window);
    expect(konvaNodeByName("canvas-generation-frame")).toHaveAttribute(
      "data-width",
      "512",
    );
    expect(onFrameChange).not.toHaveBeenCalled();
  });
  it("prevents native pointer focus from immediately dismissing the new text editor", () => {
    renderStage({ activeTool: "text" });
    setPointer(20, 30);
    const event = createPointerEvent();
    act(() => getStageHandler("onMouseDown")(event));
    expect(event.evt.preventDefault).toHaveBeenCalledOnce();
    expect(screen.getByRole("textbox")).toHaveFocus();
  });
  it("never inserts unsupported text nodes into native Konva groups", () => {
    renderStage();
    const groups = [...document.querySelectorAll('[data-konva-kind="group"]')];
    expect(
      groups.some((group) =>
        [...group.childNodes].some(
          (child) => child.nodeType === Node.TEXT_NODE,
        ),
      ),
    ).toBe(false);
  });
  it.each(["rectangle", "brush"] as const)(
    "cancels %s drawing when switching to selection",
    (activeTool) => {
      const onAddObject = vi.fn();
      const props = baseProps({ activeTool, onAddObject });
      const view = render(<ControlledStage {...props} />);
      act(() => {
        pointerDown(100, 100);
        pointerMove(150, 150);
      });
      view.rerender(<ControlledStage {...props} activeTool="select" />);
      act(() => {
        pointerMove(200, 200);
        pointerUp();
      });
      expect(onAddObject).not.toHaveBeenCalled();
      expect(callbacks.onAddStroke).not.toHaveBeenCalled();
    },
  );
});
