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
    Rect: primitive("rect"),
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

function baseProps(overrides: Partial<CanvasStageProps> = {}): CanvasStageProps {
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
  const [selectedObjectIds, setSelectedObjectIds] = useState(initialSelectedObjectIds);

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
  return [...document.querySelectorAll<HTMLElement>('[data-konva-kind="line"]')].find(
    (node) => node.dataset.points === JSON.stringify(points),
  );
}

function dispatchSpace(target: EventTarget, type: "keydown" | "keyup" = "keydown") {
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
    this.onload?.call(this as unknown as GlobalEventHandlers, new Event("load"));
  }

  get src() {
    return this.source;
  }
}

describe("CanvasStage", () => {
  beforeEach(() => {
    konva.reset();
    Object.values(callbacks).forEach((callback) => callback.mockClear());
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
    expect(screen.getByTestId("konva-stage")).toHaveAttribute("data-width", "800");
    expect(screen.getByTestId("konva-stage")).toHaveAttribute("data-height", "320");
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

    view.rerender(<CanvasStage {...props} selectedObjectIds={["image-second"]} />);

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
    expect(screen.getByRole("button", { name: "Reset aspect" })).toBeInTheDocument();

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
    expect(objectNode("image-top")).toHaveAttribute("data-x", "50");

    view.rerender(<ControlledStage {...props} activeTool={"brush" as CanvasTool} />);

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
});
