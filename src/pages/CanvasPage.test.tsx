import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { CanvasDocumentContent } from "../types";
import CanvasPage from "./CanvasPage";
import {
  waitForCanvasSaves,
  flushCanvasSaves,
} from "../lib/canvas/persistence";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

const listCanvasDocuments = vi.fn();
const createCanvasDocument = vi.fn();
const getCanvasDocument = vi.fn();
const saveCanvasDocument = vi.fn();
const saveCanvasExport = vi.fn();
const editImage = vi.fn();
const getCanvasGenerationStatus = vi.fn().mockResolvedValue(null);
const generationCompleteListeners = new Set<() => void>();
const getImageModel = vi.fn();
const pickSourceImages = vi.fn();
const importCanvasAsset = vi.fn();
const exportCanvasFrame = vi.fn();
const readImageSize = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (
      key: string,
      options?: { count?: number; number?: number; zoom?: number },
    ) =>
      ({
        "canvas.title": "Infinite Canvas",
        "canvas.assetsTitle": "Canvas Assets",
        "canvas.assetsEmpty": "Create a project canvas to start sketching.",
        "canvas.emptyHint":
          "Sketch ideas, frame a region, and send that framed composition into image generation.",
        "canvas.generationTitle": "Generation",
        "canvas.generationEmpty":
          "Prompt controls and generated results will appear here.",
        "canvas.toolbarPlaceholder": "Canvas tools will live here",
        "canvas.newCanvas": "New Canvas",
        "canvas.importImage": "Import Image",
        "canvas.copySelection": "Copy",
        "canvas.pasteSelection": "Paste",
        "canvas.deleteSelection": "Delete",
        "canvas.bringForward": "Bring Forward",
        "canvas.sendBackward": "Send Backward",
        "canvas.bringToFront": "Bring to Front",
        "canvas.sendToBack": "Send to Back",
        "canvas.fitFrame": "Fit Frame",
        "canvas.fitSelection": "Fit Selection",
        "canvas.selectionCount": `${options?.count ?? 0} selected`,
        "canvas.zoomStatus": `${options?.zoom ?? 0}%`,
        "canvas.layersTitle": "Layers",
        "canvas.frameAspect": "Frame",
        "canvas.newLayer": "New Layer",
        "canvas.lockLayer": "Lock Layer",
        "canvas.unlockLayer": "Unlock Layer",
        "canvas.hideLayer": "Hide Layer",
        "canvas.showLayer": "Show Layer",
        "canvas.defaultLayerName": `Canvas Layer ${options?.number ?? 1}`,
        "canvas.tool.select": "Select",
        "canvas.tool.brush": "Brush",
        "canvas.tool.eraser": "Eraser",
        "canvas.tool.pan": "Pan",
        "canvas.tool.undo": "Undo",
        "canvas.tool.redo": "Redo",
        "canvas.tool.zoomIn": "Zoom In",
        "canvas.tool.zoomOut": "Zoom Out",
        "canvas.generate": "Generate",
        "canvas.promptPlaceholder":
          "Describe how to develop this framed sketch...",
        "canvas.noDocumentSelected":
          "Choose or create a canvas document to begin.",
        "canvas.loading": "Loading canvas...",
        "canvas.emptyStateTitle": "Start sketching",
        "canvas.emptyStateHint":
          "Use the bottom toolbar to draw, pan, and frame the region you want to send into generation.",
        "canvas.saveStatus.saved": "Saved",
        "canvas.saveStatus.saving": "Saving...",
        "canvas.saveStatus.dirty": "Unsaved",
        "canvas.saveStatus.error": "Save failed",
        "canvas.retrySave": "Retry save",
        "canvas.loadError": "Couldn't load this canvas.",
        "canvas.retryLoad": "Retry load",
        "canvas.generating": "Generating...",
        "canvas.resetImageAspect": "Reset aspect",
        "canvas.workspaceLabel": "Canvas workspace",
        "canvas.inspectorLabel": "Generation and layers",
        "canvas.assetCount": `${options?.count ?? 0} ${
          options?.count === 1 ? "canvas" : "canvases"
        }`,
        "canvas.promptLabel": "Generation prompt",
        "canvas.objectCount": `${options?.count ?? 0} ${
          options?.count === 1 ? "object" : "objects"
        }`,
        "generate.modelLabel": "Model",
      })[key] ?? key,
  }),
}));

vi.mock("../lib/api", () => ({
  listCanvasDocuments: (...args: unknown[]) => listCanvasDocuments(...args),
  createCanvasDocument: (...args: unknown[]) => createCanvasDocument(...args),
  getCanvasDocument: (...args: unknown[]) => getCanvasDocument(...args),
  saveCanvasDocument: (...args: unknown[]) => saveCanvasDocument(...args),
  saveCanvasExport: (...args: unknown[]) => saveCanvasExport(...args),
  editImage: (...args: unknown[]) => editImage(...args),
  getCanvasGenerationStatus: (...args: unknown[]) =>
    getCanvasGenerationStatus(...args),
  onGenerationComplete: (handler: () => void) => {
    generationCompleteListeners.add(handler);
    return Promise.resolve(() => generationCompleteListeners.delete(handler));
  },
  onGenerationPartial: () => Promise.resolve(() => {}),
  onGenerationFailed: () => Promise.resolve(() => {}),
  onGenerationProgress: () => Promise.resolve(() => {}),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  hasTauriRuntime: () => true,
  pickSourceImages: (...args: unknown[]) => pickSourceImages(...args),
  importCanvasAsset: (...args: unknown[]) => importCanvasAsset(...args),
  toAssetUrl: (path: string) => path,
}));

vi.mock("../lib/canvas/export", () => ({
  exportCanvasFrame: (...args: unknown[]) => exportCanvasFrame(...args),
  readImageSize: (...args: unknown[]) => readImageSize(...args),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    activeProjectId: "project-1",
    activeConversationId: null,
    setActiveConversationId: vi.fn(),
    refreshConversations: vi.fn(),
  }),
}));

vi.mock("../components/canvas/CanvasStage", () => ({
  default: ({
    content,
    activeTool,
    selectedObjectIds,
    onSelectionChange,
    onMoveSelection,
    onStageSizeChange,
    onExport,
  }: {
    content: CanvasDocumentContent;
    activeTool: string;
    selectedObjectIds: string[];
    onSelectionChange: (ids: string[]) => void;
    onMoveSelection: (delta: { dx: number; dy: number }) => void;
    onStageSizeChange: (size: { width: number; height: number }) => void;
    onExport: () => Promise<string>;
  }) => (
    <div>
      <div>Canvas stage</div>
      <div>
        Canvas objects:{" "}
        {content.layers
          .flatMap((layer) => layer.objects.map((object) => object.id))
          .join(",") || "none"}
      </div>
      <div>Active tool: {activeTool}</div>
      <div>Selected objects: {selectedObjectIds.join(",") || "none"}</div>
      <button type="button" onClick={() => onSelectionChange(["image-1"])}>
        select image
      </button>
      <button type="button" onClick={() => onMoveSelection({ dx: 12, dy: 8 })}>
        move selection
      </button>
      <button
        type="button"
        onClick={() => onStageSizeChange({ width: 800, height: 600 })}
      >
        resize stage
      </button>
      <button type="button" onClick={() => void onExport()}>
        export stage
      </button>
    </div>
  ),
}));

function canvasDocumentWithImage() {
  return {
    id: "canvas-1",
    revision: 0,
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
              image_path: "/tmp/image-1.png",
              x: 100,
              y: 50,
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
  const layer = document.content.layers[0];

  return {
    ...document,
    content: {
      ...document.content,
      layers: [
        {
          ...layer,
          objects: [
            ...layer.objects,
            {
              type: "image" as const,
              id: "image-2",
              image_path: "/tmp/image-2.png",
              x: 300,
              y: 200,
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

function canvasDocumentWithIdentity(
  documentId: string,
  name: string,
  objectId: string,
) {
  const document = canvasDocumentWithImage();
  const layer = document.content.layers[0];
  const image = layer.objects[0];

  return {
    ...document,
    id: documentId,
    name,
    document_path: `/tmp/${documentId}.json`,
    content: {
      ...document.content,
      layers: [
        {
          ...layer,
          objects: [
            {
              ...image,
              id: objectId,
              image_path: `/tmp/${objectId}.png`,
            },
          ],
        },
      ],
    },
  };
}

function canvasDocumentWithoutObjects(documentId: string, name: string) {
  const document = canvasDocumentWithIdentity(documentId, name, "unused-image");
  const layer = document.content.layers[0];

  return {
    ...document,
    content: {
      ...document.content,
      layers: [{ ...layer, objects: [] }],
    },
  };
}

function createDeferred<T>() {
  let resolve: (value: T) => void = () => {};
  let reject: (reason?: unknown) => void = () => {};
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return { promise, resolve, reject };
}

async function advanceAutosave() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });
}

async function flushAsyncWork() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("CanvasPage", () => {
  beforeEach(() => {
    queryClient.clear();
    listCanvasDocuments.mockReset();
    createCanvasDocument.mockReset();
    getCanvasDocument.mockReset();
    saveCanvasDocument.mockReset();
    saveCanvasExport.mockReset();
    editImage.mockReset();
    getCanvasGenerationStatus.mockReset().mockResolvedValue(null);
    generationCompleteListeners.clear();
    getImageModel.mockReset();
    pickSourceImages.mockReset();
    importCanvasAsset.mockReset().mockImplementation(async (path: string) => ({ id: "asset-1", file_path: `/managed${path}` }));
    exportCanvasFrame.mockReset();
    readImageSize.mockReset();

    listCanvasDocuments.mockResolvedValue([
      {
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
      },
    ]);
    getCanvasDocument.mockResolvedValue({
      id: "canvas-1",
      revision: 0,
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
            objects: [],
          },
        ],
      },
    });
    createCanvasDocument.mockResolvedValue({
      id: "canvas-2",
      project_id: "project-1",
      name: "Fresh canvas",
      document_path: "/tmp/canvas-2.json",
      preview_path: null,
      width: 1024,
      height: 1024,
      created_at: "2026-05-12T00:00:00Z",
      updated_at: "2026-05-12T00:00:00Z",
      deleted_at: null,
    });
    saveCanvasDocument.mockResolvedValue({
      id: "canvas-1",
      revision: 0,
      project_id: "project-1",
      name: "Mood board",
      document_path: "/tmp/canvas-1.json",
      preview_path: "/tmp/canvas-preview.png",
      width: 1024,
      height: 1024,
      created_at: "2026-05-12T00:00:00Z",
      updated_at: "2026-05-12T00:00:01Z",
      deleted_at: null,
    });
    saveCanvasExport.mockResolvedValue("/tmp/canvas-export.png");
    editImage.mockResolvedValue({
      generation_id: "generation-1",
      conversation_id: "conversation-1",
      images: [],
    });
    getImageModel.mockResolvedValue("gpt-image-2");
    pickSourceImages.mockResolvedValue([]);
    exportCanvasFrame.mockResolvedValue("data:image/png;base64,canvas-preview");
    readImageSize.mockResolvedValue({ width: 512, height: 512 });
  });

  afterEach(async () => {
    cleanup();
    vi.useRealTimers();
    // A failed write deliberately survives unmount for load/retry recovery.
    // Finish that write before the next test starts a fresh mocked app session.
    exportCanvasFrame.mockResolvedValue("preview");
    saveCanvasDocument.mockResolvedValue({ id: "canvas-1", revision: 1 });
    await waitForCanvasSaves("canvas-1", true, () => true);
    await waitForCanvasSaves("canvas-2", true, () => true);
    vi.restoreAllMocks();
  });

  it("loads canvas documents for the active project", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Mood board")).toBeInTheDocument();
    expect(listCanvasDocuments).toHaveBeenCalledWith("project-1");
    await waitFor(() => {
      expect(getCanvasDocument).toHaveBeenCalledWith("canvas-1");
    });
  });
  it.each(["toolbar", "shortcut"])("opens new text defaults instead of styling the old selection via %s", async (source) => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
    render(<CanvasPage />, { wrapper: TestWrapper });
    fireEvent.click(await screen.findByRole("button", { name: "select image" }));
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();
    if (source === "toolbar") fireEvent.click(screen.getByRole("button", { name: "canvas.tool.text" }));
    else fireEvent.keyDown(window, { key: "t" });
    expect(screen.getByText("Selected objects: none")).toBeInTheDocument();
    expect(screen.getByLabelText("canvas.fontFamily")).toBeInTheDocument();
  });

  it("creates a new canvas document", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    fireEvent.click(await screen.findByRole("button", { name: "New Canvas" }));

    await waitFor(() => {
      expect(createCanvasDocument).toHaveBeenCalledWith("project-1", null);
    });
  });

  it("still lets the user create the first canvas when the list is empty", async () => {
    listCanvasDocuments.mockResolvedValueOnce([]);

    render(<CanvasPage />, { wrapper: TestWrapper });

    const emptyStateCard = await screen.findByTestId("canvas-empty-state-card");
    expect(emptyStateCard).toHaveClass("w-[min(360px,calc(100%-48px))]");

    fireEvent.click(await screen.findByRole("button", { name: "New Canvas" }));

    await waitFor(() => {
      expect(createCanvasDocument).toHaveBeenCalledWith("project-1", null);
    });
  });

  it("submits generation with an exported frame path", async () => {
    let resolveEditImage: (value: unknown) => void = () => {};
    editImage.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveEditImage = resolve;
      }),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    await screen.findByText("Mood board");
    await screen.findByText("Canvas objects: none");

    fireEvent.click(screen.getByRole("button", { name: "Mood board" }));
    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how to develop this framed sketch...",
      ),
      {
        target: {
          value: "Turn this sketch into a polished cinematic environment",
        },
      },
    );
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    expect(
      await screen.findByRole("status", { name: "Generating..." }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Generating..." }),
    ).toBeDisabled();

    await waitFor(() => {
      expect(saveCanvasExport).toHaveBeenCalledWith(
        "canvas-1",
        expect.any(String),
      );
    });

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-image-2",
          prompt: "Turn this sketch into a polished cinematic environment",
          sourceImagePaths: ["/tmp/canvas-export.png"],
          projectId: "project-1",
        }),
      );
    });

    resolveEditImage({
      generation_id: "generation-1",
      conversation_id: "conversation-1",
      images: [],
    });

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Generate" })).toBeEnabled();
    });
  });

  it("persists a pending request before submission and inserts returned images as one undo batch", async () => {
    editImage.mockResolvedValueOnce({
      generation_id: "g",
      conversation_id: "c",
      images: [
        {
          id: "a",
          generation_id: "g",
          file_path: "/a.png",
          thumbnail_path: "/a.png",
          width: 200,
          height: 100,
          file_size: 1,
        },
        {
          id: "b",
          generation_id: "g",
          file_path: "/b.png",
          thumbnail_path: "/b.png",
          width: 100,
          height: 100,
          file_size: 1,
        },
      ],
    });
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: none");
    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how to develop this framed sketch...",
      ),
      { target: { value: "draw" } },
    );
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(saveCanvasDocument).toHaveBeenCalledWith(
        "canvas-1",
        expect.objectContaining({
          generation_requests: [
            expect.objectContaining({
              status: "completed",
              image_ids: ["a", "b"],
            }),
          ],
        }),
        expect.any(String),
        expect.any(Number),
      ),
    );
    expect(saveCanvasDocument.mock.invocationCallOrder[0]).toBeLessThan(
      editImage.mock.invocationCallOrder[0],
    );
    fireEvent.keyDown(window, { key: "z", metaKey: true });
    await screen.findByText("Canvas objects: none");
    fireEvent.keyDown(window, { key: "x" });
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(screen.getByText("Canvas objects: none")).toBeInTheDocument();
  });

  it("shows failed exports and retries with a new request identity", async () => {
    saveCanvasExport.mockRejectedValueOnce(new Error("PNG write failed"));
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: none");
    fireEvent.change(screen.getByLabelText("Generation prompt"), {
      target: { value: "draw" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    expect(await screen.findByText(/PNG write failed/)).toBeInTheDocument();
    expect(editImage).not.toHaveBeenCalled();
    const firstId =
      saveCanvasDocument.mock.calls[0][1].generation_requests[0].request_id;
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(
      screen.getByRole("button", { name: "canvas.generationRetry" }),
    );
    await waitFor(() => expect(editImage).toHaveBeenCalled());
    expect(editImage.mock.calls[0][0].clientRequestId).not.toBe(firstId);
  });

  it("retries a saved request using its original export instead of the changed canvas", async () => {
    const document = canvasDocumentWithImage();
    document.content.generation_requests = [
      {
        request_id: "old-request",
        project_id: "project-1",
        prompt: "original edit",
        frame: { ...document.content.frame },
        status: "failed",
        image_ids: [],
        created_at: "",
        error: "failed",
        source_image_path: "/tmp/original-export.png",
        parameters: {
          prompt: "original edit",
          model: "gpt-image-2.6-flare",
          advancedOptions: { markupPath: "/tmp/original-markup.png" },
        },
      },
    ];
    getCanvasDocument.mockResolvedValueOnce(document);
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(
      screen.getByRole("button", { name: "canvas.generationRetry" }),
    );
    await waitFor(() =>
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          sourceImagePaths: ["/tmp/original-export.png"],
          advancedOptions: { markupPath: "/tmp/original-markup.png" },
        }),
      ),
    );
    expect(saveCanvasExport).not.toHaveBeenCalled();
  });

  it("merges late results into current edits and keeps completion metadata through undo", async () => {
    const deferred = createDeferred<unknown>();
    editImage.mockReturnValueOnce(deferred.promise);
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    fireEvent.change(screen.getByLabelText("Generation prompt"), {
      target: { value: "draw" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() => expect(editImage).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await act(async () =>
      deferred.resolve({
        generation_id: "g",
        conversation_id: "c",
        images: [{ id: "a", file_path: "/a.png", width: 100, height: 100 }],
      }),
    );
    await waitFor(() =>
      expect(
        saveCanvasDocument.mock.calls.at(-1)?.[1].layers[1].objects[0].x,
      ).toBe(112),
    );
    fireEvent.keyDown(window, { key: "z", metaKey: true });
    await screen.findByText("Canvas objects: image-1");
    vi.useFakeTimers();
    await advanceAutosave();
    expect(
      saveCanvasDocument.mock.calls.at(-1)?.[1].generation_requests[0].status,
    ).toBe("completed");
  });

  it("does not place a late result into another document", async () => {
    const first = canvasDocumentWithIdentity(
        "canvas-1",
        "Mood board",
        "image-a",
      ),
      second = canvasDocumentWithIdentity(
        "canvas-2",
        "Second canvas",
        "image-b",
      );
    listCanvasDocuments.mockResolvedValue([first, second]);
    getCanvasDocument.mockImplementation((id: string) =>
      Promise.resolve(id === "canvas-1" ? first : second),
    );
    const deferred = createDeferred<unknown>();
    editImage.mockReturnValueOnce(deferred.promise);
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-a");
    fireEvent.change(screen.getByLabelText("Generation prompt"), {
      target: { value: "draw" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() => expect(editImage).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await screen.findByText("Canvas objects: image-b");
    await act(async () =>
      deferred.resolve({
        generation_id: "g",
        conversation_id: "c",
        images: [{ id: "a", file_path: "/a.png", width: 100, height: 100 }],
      }),
    );
    expect(screen.getByText("Canvas objects: image-b")).toBeInTheDocument();
    expect(
      saveCanvasDocument.mock.calls.filter((call) => call[0] === "canvas-2"),
    ).toHaveLength(0);
  });

  it("gives the canvas prompt editor a larger writing area", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    const promptEditor = await screen.findByPlaceholderText(
      "Describe how to develop this framed sketch...",
    );

    expect(promptEditor).toHaveClass("min-h-[144px]");
  });

  it("starts with properties and preserves the generation draft while switching inspector tabs", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas stage");
    const properties = screen.getByRole("tab", {
      name: "canvas.propertiesTitle",
    });
    expect(properties).toHaveAttribute("aria-selected", "true");
    const generation = screen.getByRole("tab", { name: "Generation" });
    fireEvent.click(generation);
    const editor = screen.getByPlaceholderText(
      "Describe how to develop this framed sketch...",
    );
    fireEvent.change(editor, { target: { value: "A quiet mountain lake" } });
    fireEvent.click(screen.getByRole("tab", { name: "Layers" }));
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(screen.getByRole("tab", { name: "Layers" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    fireEvent.click(generation);
    expect(
      screen.getByPlaceholderText(
        "Describe how to develop this framed sketch...",
      ),
    ).toBe(editor);
    expect(editor).toHaveValue("A quiet mountain lake");
  });

  it("docks the inspector at 1200px while documents remain a drawer until 1440px", async () => {
    const original = window.matchMedia;
    const mediaSpy = vi
      .spyOn(window, "matchMedia")
      .mockImplementation((query) => ({
        ...original(query),
        matches: query === "(min-width: 1200px)",
      }));
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas stage");
    expect(document.getElementById("canvas-inspector")).toHaveAttribute(
      "data-layout",
      "docked",
    );
    expect(document.getElementById("canvas-documents")).toHaveAttribute(
      "data-layout",
      "drawer",
    );
    fireEvent.click(
      screen.getByRole("button", { name: "studio.canvas.documents" }),
    );
    expect(
      screen.getByRole("dialog", { name: "studio.canvas.documents" }),
    ).toHaveAttribute("aria-modal", "true");
    mediaSpy.mockRestore();
  });

  it("uses the professional editor layout with floating canvas tools", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByLabelText("Canvas workspace")).toHaveClass(
      "min-w-0",
      "min-[1440px]:grid-cols-[220px_minmax(0,1fr)_300px]",
    );
    await screen.findByText("Canvas stage");
    expect(screen.getByText("1 canvas")).toBeInTheDocument();
    expect(screen.getByText("0 objects")).toBeInTheDocument();
    expect(screen.getByLabelText("Generation and layers")).toBeInTheDocument();
    expect(screen.getByTestId("canvas-floating-toolbar")).toBeInTheDocument();
  });

  it("opens one compact panel at a time and restores focus on Escape", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas stage");
    const documents = screen.getByRole("button", {
      name: "studio.canvas.documents",
    });
    const inspector = screen.getByRole("button", {
      name: "studio.canvas.inspector",
    });
    expect(documents).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(documents);
    expect(documents).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(inspector);
    expect(documents).toHaveAttribute("aria-expanded", "false");
    expect(inspector).toHaveAttribute("aria-expanded", "true");
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(inspector).toHaveAttribute("aria-expanded", "false");
    expect(inspector).toHaveFocus();
    expect(screen.getByText("Canvas stage")).toBeInTheDocument();
  });

  it("focuses the compact inspector and suspends canvas shortcuts until it closes", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("1 object");
    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    const inspector = screen.getByRole("button", {
      name: "studio.canvas.inspector",
    });
    inspector.focus();
    fireEvent.click(inspector);
    const panel = document.getElementById("canvas-inspector")!;
    expect(panel).toContainElement(document.activeElement as HTMLElement);
    fireEvent.keyDown(window, { key: "Delete" });
    fireEvent.keyDown(window, { key: "b" });
    expect(screen.getByText("1 object")).toBeInTheDocument();
    expect(screen.getByText("Active tool: select")).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(inspector).toHaveFocus();
    expect(inspector).toHaveAttribute("aria-expanded", "false");
    fireEvent.keyDown(window, { key: "b" });
    expect(screen.getByText("Active tool: brush")).toBeInTheDocument();
  });

  it("releases the compact panel when resizing to the docked layout", async () => {
    const originalMatchMedia = window.matchMedia;
    let docked = false;
    let onResize = () => {};
    const mediaSpy = vi
      .spyOn(window, "matchMedia")
      .mockImplementation((query) =>
        query === "(min-width: 1200px)"
          ? {
              ...originalMatchMedia(query),
              get matches() {
                return docked;
              },
              addEventListener: (_event, listener) => {
                onResize = listener as () => void;
              },
            }
          : originalMatchMedia(query),
      );
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas stage");
    fireEvent.click(
      screen.getByRole("button", { name: "studio.canvas.inspector" }),
    );
    expect(
      screen.getByRole("dialog", { name: "Generation and layers" }),
    ).toHaveAttribute("aria-modal", "true");
    act(() => {
      docked = true;
      onResize();
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "b" });
    expect(screen.getByText("Active tool: brush")).toBeInTheDocument();
    mediaSpy.mockRestore();
  });

  it("localizes the default name when adding a new layer", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    await screen.findByText("0 objects");
    fireEvent.click(screen.getByRole("tab", { name: "Layers" }));
    fireEvent.click(screen.getByRole("button", { name: "New Layer" }));

    expect(screen.getByText("Canvas Layer 2")).toBeInTheDocument();
  });

  it("imports an image into the canvas document", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/reference.png"]);

    render(<CanvasPage />, { wrapper: TestWrapper });

    fireEvent.click(
      await screen.findByRole("button", { name: "Import Image" }),
    );

    await waitFor(() => {
      expect(pickSourceImages).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(saveCanvasDocument).toHaveBeenCalledWith(
        "canvas-1",
        expect.objectContaining({
          layers: [
            expect.objectContaining({
              objects: [
                expect.objectContaining({
                  type: "image",
                  image_path: "/managed/tmp/reference.png",
                  asset_id: "asset-1",
                  original_width: 512,
                  original_height: 512,
                }),
              ],
            }),
          ],
        }),
        expect.any(String),
        expect.any(Number),
      );
    });

    expect(screen.getByText("Active tool: select")).toBeInTheDocument();
  });

  it("deletes the selected image with the Delete shortcut and autosaves", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Delete", code: "Delete" });
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [expect.objectContaining({ objects: [] })],
      }),
      expect.any(String),
        expect.any(Number),
    );
  });

  it("supports tool shortcuts but ignores them while typing in the prompt", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    await screen.findByText("0 objects");
    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    expect(screen.getByText("Active tool: select")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "b", code: "KeyB" });
    expect(screen.getByText("Active tool: brush")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    const promptEditor = screen.getByLabelText("Generation prompt");
    promptEditor.focus();
    fireEvent.keyDown(promptEditor, { key: "b", code: "KeyB" });

    expect(screen.getByText("Active tool: select")).toBeInTheDocument();
  });

  it("copies and pastes the selected image and autosaves both objects", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));

    fireEvent.keyDown(window, { key: "c", metaKey: true });
    fireEvent.keyDown(window, { key: "v", metaKey: true });
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalled();
    const savedContent = saveCanvasDocument.mock.calls.at(-1)?.[1];
    expect(savedContent.layers[0].objects).toHaveLength(2);
    expect(savedContent.layers[0].objects[0].id).toBe("image-1");
    expect(savedContent.layers[0].objects[1]).toMatchObject({
      type: "image",
      image_path: "/tmp/image-1.png",
    });
    expect(savedContent.layers[0].objects[1].id).not.toBe("image-1");
  });

  it("moves the selected image by the stage delta and autosaves", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalled();
    const savedContent = saveCanvasDocument.mock.calls.at(-1)?.[1];
    expect(savedContent.layers[0].objects[0]).toMatchObject({ x: 112, y: 58 });
  });

  it("fits the selected image to the reported stage size", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(screen.getByText("1 selected")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "resize stage" }));
    fireEvent.click(screen.getByRole("button", { name: "canvas.zoomOptions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Fit Selection" }));

    expect(screen.getByText("400%")).toBeInTheDocument();
    await advanceAutosave();
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        viewport: { x: -200, y: -100, scale: 4 },
      }),
      expect.any(String),
        expect.any(Number),
    );
  });

  it("brings the selected image to the front and autosaves the object order", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithTwoImages());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("2 objects")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "canvas.actions" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Bring to Front" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalled();
    const savedContent = saveCanvasDocument.mock.calls.at(-1)?.[1];
    expect(
      savedContent.layers[0].objects.map((object: { id: string }) => object.id),
    ).toEqual(["image-2", "image-1"]);
  });

  it("clears selection immediately when switching canvas documents", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = {
      ...canvasDocumentWithImage(),
      id: "canvas-2",
      name: "Second canvas",
      document_path: "/tmp/canvas-2.json",
    };
    let resolveSecondDocument: (
      document: typeof secondDocument,
    ) => void = () => {};

    listCanvasDocuments.mockResolvedValueOnce([firstDocument, secondDocument]);
    getCanvasDocument.mockResolvedValueOnce(firstDocument);
    getCanvasDocument.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSecondDocument = resolve;
        }),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(
      await screen.findByText("Selected objects: image-1"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));

    expect(await screen.findByText("Loading canvas...")).toBeInTheDocument();
    expect(screen.queryByText("Canvas stage")).not.toBeInTheDocument();

    await act(async () => {
      resolveSecondDocument(secondDocument);
    });

    expect(
      await screen.findByText("Selected objects: none"),
    ).toBeInTheDocument();
  });

  it("ignores stale document loads that resolve after the selected document", async () => {
    const firstDocument = canvasDocumentWithIdentity(
      "canvas-1",
      "Mood board",
      "image-a",
    );
    const secondDocument = canvasDocumentWithIdentity(
      "canvas-2",
      "Second canvas",
      "image-1",
    );
    const firstLoad = createDeferred<typeof firstDocument>();
    const secondLoad = createDeferred<typeof secondDocument>();

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id ? firstLoad.promise : secondLoad.promise,
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    await waitFor(() => {
      expect(getCanvasDocument).toHaveBeenCalledWith("canvas-1");
    });
    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await waitFor(() => {
      expect(getCanvasDocument).toHaveBeenCalledWith("canvas-2");
    });

    await act(async () => {
      secondLoad.resolve(secondDocument);
      await secondLoad.promise;
    });
    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();

    await act(async () => {
      firstLoad.resolve(firstDocument);
      await firstLoad.promise;
    });

    expect(screen.getByText("Canvas objects: image-1")).toBeInTheDocument();
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();
    expect(
      screen.queryByText("Canvas objects: image-a"),
    ).not.toBeInTheDocument();
  });

  it("blocks edits and autosave until the selected document finishes loading", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );
    const secondLoad = createDeferred<typeof secondDocument>();

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id
        ? Promise.resolve(firstDocument)
        : secondLoad.promise,
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.keyDown(window, { key: "c", metaKey: true });
    vi.useFakeTimers();
    saveCanvasDocument.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();
    expect(getCanvasDocument).toHaveBeenCalledWith("canvas-2");
    fireEvent.keyDown(window, { key: "v", code: "KeyV", ctrlKey: true });
    await advanceAutosave();

    expect(saveCanvasDocument).not.toHaveBeenCalled();

    await act(async () => {
      secondLoad.resolve(secondDocument);
      await secondLoad.promise;
    });
    expect(screen.getByText("Canvas objects: none")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "v", code: "KeyV", ctrlKey: true });
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-2",
      expect.objectContaining({
        layers: [
          expect.objectContaining({
            objects: [
              expect.objectContaining({ image_path: "/tmp/image-1.png" }),
            ],
          }),
        ],
      }),
      expect.any(String),
        expect.any(Number),
    );
  });

  it("ignores tool shortcuts until the selected document finishes loading", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );
    const secondLoad = createDeferred<typeof secondDocument>();

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id
        ? Promise.resolve(firstDocument)
        : secondLoad.promise,
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Select" }));
    expect(screen.getByText("Active tool: select")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    expect(await screen.findByText("Loading canvas...")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "b", code: "KeyB" });

    await act(async () => {
      secondLoad.resolve(secondDocument);
      await secondLoad.promise;
    });

    expect(await screen.findByText("Active tool: select")).toBeInTheDocument();
  });

  it("queues the latest dirty snapshot on unmount before maintenance can finish", async () => {
    const preview = createDeferred<string>();
    getCanvasDocument.mockResolvedValue(canvasDocumentWithImage());
    exportCanvasFrame.mockReturnValueOnce(preview.promise);
    const { unmount } = render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    expect(screen.getByText("Unsaved")).toBeInTheDocument();
    unmount();
    let flushed = false;
    const flush = flushCanvasSaves().then(() => {
      flushed = true;
    });
    await flushAsyncWork();
    expect(flushed).toBe(false);
    expect(saveCanvasDocument).not.toHaveBeenCalled();
    preview.resolve("latest-preview");
    await act(async () => {
      await flush;
    });
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [
          expect.objectContaining({
            objects: [expect.objectContaining({ x: 112, y: 58 })],
          }),
        ],
      }),
      "latest-preview",
        expect.any(Number),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);
  });

  it("does not queue a redundant save when a clean canvas unmounts", async () => {
    const { unmount } = render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: none");
    unmount();
    await flushCanvasSaves();
    expect(saveCanvasDocument).not.toHaveBeenCalled();
  });

  it("serializes same-document saves so the newest snapshot is written last", async () => {
    const document = canvasDocumentWithImage();
    const firstSave = createDeferred<typeof document>();
    const secondSave = createDeferred<typeof document>();

    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument
      .mockImplementationOnce(() => firstSave.promise)
      .mockImplementationOnce(() => secondSave.promise);

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);

    expect(saveCanvasDocument.mock.calls[0][3]).toBe(0);
    firstSave.resolve({ ...document, revision: 1 });
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(saveCanvasDocument.mock.calls[1][3]).toBe(1);
    expect(saveCanvasDocument.mock.calls[1][0]).toBe("canvas-1");
    expect(
      saveCanvasDocument.mock.calls[1][1].layers[0].objects[0],
    ).toMatchObject({
      x: 124,
      y: 66,
    });

    secondSave.resolve(document);
    await flushAsyncWork();

    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("flushes a dirty document before query reconciliation selects another", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(
        documentId === firstDocument.id ? firstDocument : secondDocument,
      ),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    saveCanvasDocument.mockClear();

    listCanvasDocuments.mockResolvedValue([secondDocument]);
    await act(async () => {
      await queryClient.refetchQueries({
        queryKey: ["canvas-documents", "project-1"],
      });
      await vi.advanceTimersByTimeAsync(0);
    });
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [
          expect.objectContaining({
            objects: [expect.objectContaining({ x: 112, y: 58 })],
          }),
        ],
      }),
      expect.any(String),
        expect.any(Number),
    );

    const secondLoadIndex = getCanvasDocument.mock.calls.findIndex(
      ([documentId]) => documentId === "canvas-2",
    );
    expect(secondLoadIndex).toBeGreaterThanOrEqual(0);
    expect(saveCanvasDocument.mock.invocationCallOrder[0]).toBeLessThan(
      getCanvasDocument.mock.invocationCallOrder[secondLoadIndex],
    );
  });

  it("keeps a failed autosave dirty and retries the same snapshot", async () => {
    const document = canvasDocumentWithImage();
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument.mockRejectedValueOnce(
      new Error("secret backend credential must not be rendered"),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
    expect(
      screen.getByRole("button", { name: "Retry save" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/secret backend credential/i),
    ).not.toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();

    const failedSnapshot = saveCanvasDocument.mock.calls[0][1];
    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(saveCanvasDocument.mock.calls[1][0]).toBe("canvas-1");
    expect(saveCanvasDocument.mock.calls[1][1]).toEqual(failedSnapshot);
    expect(
      saveCanvasDocument.mock.calls[1][1].layers[0].objects[0],
    ).toMatchObject({
      x: 112,
      y: 58,
    });
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("keeps the old document editable when a switch flush fails", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(
        documentId === firstDocument.id ? firstDocument : secondDocument,
      ),
    );
    saveCanvasDocument.mockRejectedValueOnce(
      new Error("private switch flush failure"),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
    expect(
      screen.getByRole("button", { name: "Retry save" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Canvas objects: image-1")).toBeInTheDocument();
    expect(getCanvasDocument).not.toHaveBeenCalledWith("canvas-2");
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        layers: [
          expect.objectContaining({
            objects: [expect.objectContaining({ x: 112, y: 58 })],
          }),
        ],
      }),
      expect.any(String),
        expect.any(Number),
    );
    expect(
      saveCanvasDocument.mock.calls.every(
        ([documentId]) => documentId === "canvas-1",
      ),
    ).toBe(true);
    expect(
      screen.queryByText(/private switch flush failure/i),
    ).not.toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    await flushAsyncWork();
    expect(screen.getByText("Saved")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();

    expect(getCanvasDocument).toHaveBeenCalledWith("canvas-2");
    expect(screen.getByText("Canvas objects: none")).toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("does not continue a queued document transition after a flush fails", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );
    const thirdDocument = canvasDocumentWithoutObjects(
      "canvas-3",
      "Third canvas",
    );
    const failedFlush = createDeferred<typeof firstDocument>();

    listCanvasDocuments.mockResolvedValue([
      firstDocument,
      secondDocument,
      thirdDocument,
    ]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(
        documentId === firstDocument.id
          ? firstDocument
          : documentId === secondDocument.id
            ? secondDocument
            : thirdDocument,
      ),
    );
    saveCanvasDocument.mockImplementationOnce(() => failedFlush.promise);

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    fireEvent.click(screen.getByRole("button", { name: "Third canvas" }));
    await act(async () => {
      failedFlush.reject(new Error("switch flush failed"));
      await failedFlush.promise.catch(() => {});
    });
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
    expect(screen.getByText("Canvas objects: image-1")).toBeInTheDocument();
    expect(getCanvasDocument).not.toHaveBeenCalledWith("canvas-2");
    expect(getCanvasDocument).not.toHaveBeenCalledWith("canvas-3");
  });

  it("shows a recoverable error when the selected document load fails", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects(
      "canvas-2",
      "Second canvas",
    );
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id
        ? Promise.resolve(firstDocument)
        : Promise.reject(new Error("secret load response")),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Couldn't load this canvas.",
    );
    expect(
      screen.getByRole("button", { name: "Retry load" }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/secret load response/i)).not.toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();

    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(
        documentId === firstDocument.id ? firstDocument : secondDocument,
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry load" }));
    await flushAsyncWork();

    expect(getCanvasDocument).toHaveBeenCalledTimes(3);
    expect(screen.getByText("Canvas objects: none")).toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("keeps the newest snapshot saving when an older save resolves", async () => {
    const document = canvasDocumentWithImage();
    const firstSave = createDeferred<typeof document>();
    const secondSave = createDeferred<typeof document>();

    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument
      .mockImplementationOnce(() => firstSave.promise)
      .mockImplementationOnce(() => secondSave.promise);

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(
      await screen.findByText("Canvas objects: image-1"),
    ).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Saving...")).toBeInTheDocument();

    expect(saveCanvasDocument.mock.calls[0][3]).toBe(0);
    firstSave.resolve({ ...document, revision: 1 });
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(saveCanvasDocument.mock.calls[1][3]).toBe(1);
    expect(screen.getByText("Saving...")).toBeInTheDocument();

    secondSave.resolve(document);
    await flushAsyncWork();
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("waits for an earlier mount's disk save before reopening and editing the document", async () => {
    const document = canvasDocumentWithImage();
    let disk: CanvasDocumentContent = structuredClone(document.content);
    const oldPreview = createDeferred<string>();
    getCanvasDocument.mockImplementation(async () => ({
      ...document,
      content: structuredClone(disk),
    }));
    exportCanvasFrame.mockImplementationOnce(() => oldPreview.promise);
    saveCanvasDocument.mockImplementation(
      async (_id: string, snapshot: CanvasDocumentContent) => {
        disk = structuredClone(snapshot);
        return document;
      },
    );
    const old = render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    old.unmount();
    vi.useRealTimers();
    render(<CanvasPage />, { wrapper: TestWrapper });
    await flushAsyncWork();
    expect(screen.getByText("Loading canvas...")).toBeInTheDocument();
    expect(getCanvasDocument).toHaveBeenCalledTimes(1);
    await act(async () => oldPreview.resolve("old-preview"));
    await screen.findByText("Canvas objects: image-1");
    expect(getCanvasDocument).toHaveBeenCalledTimes(2);
    expect(disk.layers[0].objects[0]).toMatchObject({ x: 112 });
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    expect(disk.layers[0].objects[0]).toMatchObject({ x: 124, y: 66 });
  });

  it("blocks reopening after an earlier mount's save fails and retries that exact save before reading", async () => {
    const document = canvasDocumentWithImage();
    let disk: CanvasDocumentContent = structuredClone(document.content);
    const oldSave = createDeferred<typeof document>();
    getCanvasDocument.mockImplementation(async () => ({
      ...document,
      content: structuredClone(disk),
    }));
    saveCanvasDocument.mockImplementationOnce(() => oldSave.promise).mockImplementation(
      async (_id: string, snapshot: CanvasDocumentContent) => {
        disk = structuredClone(snapshot);
        return document;
      },
    );
    const old = render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    old.unmount();
    vi.useRealTimers();
    render(<CanvasPage />, { wrapper: TestWrapper });
    await flushAsyncWork();
    await act(async () => oldSave.reject(new Error("disk unavailable")));
    expect(
      await screen.findByText("Couldn't load this canvas."),
    ).toBeInTheDocument();
    expect(getCanvasDocument).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Retry load" }));
    await screen.findByText("Canvas objects: image-1");
    expect(disk.layers[0].objects[0]).toMatchObject({ x: 112, y: 58 });
    expect(getCanvasDocument).toHaveBeenCalledTimes(2);
  });

  it("keeps an ambiguously rejected submission pending and recovers its successful backend result", async () => {
    editImage.mockRejectedValueOnce(new Error("IPC reply lost"));
    getCanvasGenerationStatus.mockResolvedValue({
      generation_id: "durable-job",
      status: "running",
      error: null,
      images: [],
    });
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: none");
    fireEvent.change(screen.getByLabelText("Generation prompt"), {
      target: { value: "draw" },
    });
    fireEvent.click(screen.getByRole("tab", { name: "Generation" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));
    await waitFor(() =>
      expect(getCanvasGenerationStatus).toHaveBeenCalledWith(
        "canvas-1",
        expect.any(String),
      ),
    );
    expect(await screen.findByText(/IPC reply lost/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Generating..." }),
    ).toBeDisabled();
    expect(
      saveCanvasDocument.mock.calls.at(-1)?.[1].generation_requests[0],
    ).toMatchObject({ status: "pending", generation_id: "durable-job" });
    getCanvasGenerationStatus.mockResolvedValue({
      generation_id: "durable-job",
      status: "completed",
      error: null,
      images: [
        {
          id: "recovered",
          file_path: "/recovered.png",
          width: 100,
          height: 100,
        },
      ],
    });
    await waitFor(
      () =>
        expect(
          screen.getByText(/Canvas objects: canvas-result:/),
        ).toBeInTheDocument(),
      { timeout: 3000 },
    );
    await flushAsyncWork();
    expect(editImage).toHaveBeenCalledTimes(1);
    expect(
      saveCanvasDocument.mock.calls.at(-1)?.[1].generation_requests[0].status,
    ).toBe("completed");
  });

  it("reconciles a failed canvas request after manual save recovery and does not insert again on reopen", async () => {
    const document = canvasDocumentWithImage();
    let disk: CanvasDocumentContent = structuredClone(document.content);
    disk.generation_requests = [{
      request_id: "recover-request", generation_id: "recover-generation", project_id: "project-1",
      frame: disk.frame, prompt: "Recover this drawing", status: "failed", image_ids: [],
      error: "Image save failed", created_at: "2026-05-12T00:00:00Z",
    }];
    getCanvasDocument.mockImplementation(async () => ({ ...document, content: structuredClone(disk) }));
    saveCanvasDocument.mockImplementation(async (_id: string, content: CanvasDocumentContent) => {
      disk = structuredClone(content);
      return document;
    });
    getCanvasGenerationStatus.mockResolvedValue({ generation_id: "recover-generation", status: "failed", error: "Image save failed", images: [] });
    const mounted = render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    await waitFor(() => expect(getCanvasGenerationStatus).toHaveBeenCalledWith("canvas-1", "recover-request"));
    getCanvasGenerationStatus.mockResolvedValue({
      generation_id: "recover-generation", status: "succeeded", error: null,
      images: [{ id: "recovered", file_path: "/recovered.png", width: 100, height: 100 }],
    });
    await act(async () => generationCompleteListeners.forEach(listener => listener()));
    await waitFor(() => expect(screen.getByText(/Canvas objects: canvas-result:recover-request:recovered/)).toBeInTheDocument());
    await flushAsyncWork();
    expect(disk.generation_requests?.[0]).toMatchObject({ status: "completed", image_ids: ["recovered"] });
    expect(disk.layers.flatMap(layer => layer.objects).filter(object => object.id === "canvas-result:recover-request:recovered")).toHaveLength(1);
    mounted.unmount();
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText(/Canvas objects: canvas-result:recover-request:recovered/);
    await flushAsyncWork();
    expect(disk.layers.flatMap(layer => layer.objects).filter(object => object.id === "canvas-result:recover-request:recovered")).toHaveLength(1);
    expect(editImage).not.toHaveBeenCalled();
  });

  it("clears external selection when its layer becomes locked", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(
      await screen.findByText("Selected objects: image-1"),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "Layers" }));
    fireEvent.click(screen.getByRole("button", { name: "Lock Layer" }));

    expect(
      await screen.findByText("Selected objects: none"),
    ).toBeInTheDocument();
  });

it("flushes a dirty mounted canvas while its autosave timer is pending", async () => {
  getCanvasDocument.mockResolvedValue(canvasDocumentWithImage());
  render(<CanvasPage />, { wrapper: TestWrapper });
  await screen.findByText("Canvas objects: image-1");
  vi.useFakeTimers();
  fireEvent.click(screen.getByRole("button", {name: "select image"}));
  fireEvent.click(screen.getByRole("button", {name: "move selection"}));
  expect(saveCanvasDocument).not.toHaveBeenCalled();
  await act(async () => { await flushCanvasSaves(); });
  expect(saveCanvasDocument.mock.calls[0][1].layers[0].objects[0]).toMatchObject({x:112,y:58});
  vi.useRealTimers(); cleanup();
});

  it("relinks a missing image into managed storage without changing its transform", async () => {
    const document = canvasDocumentWithImage();
    getCanvasDocument.mockResolvedValue({ ...document, missing_assets: [{ object_id: "image-1", image_path: "/tmp/image-1.png" }] });
    pickSourceImages.mockResolvedValue(["/new/replacement.png"]);
    render(<CanvasPage />, { wrapper: TestWrapper });
    fireEvent.click(await screen.findByRole("button", { name: /canvas.relinkAsset/ }));
    await waitFor(() => expect(saveCanvasDocument).toHaveBeenCalled());
    expect(importCanvasAsset).toHaveBeenCalledWith("/new/replacement.png");
    expect(saveCanvasDocument.mock.calls[0][1].layers[0].objects[0]).toMatchObject({
      id: "image-1", asset_id: "asset-1", image_path: "/managed/new/replacement.png",
      x: 100, y: 50, width: 100, height: 100, rotation: 0, opacity: 1,
    });
    await waitFor(() => expect(screen.queryByRole("button", { name: /canvas.relinkAsset/ })).not.toBeInTheDocument());
  });

  it("preserves a conflicted draft as a new canvas without overwriting the remote revision", async () => {
    const document = canvasDocumentWithImage();
    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument.mockRejectedValueOnce({ kind: "CanvasRevisionConflict", detail: { expected_revision: 0, actual_revision: 4 } });
    render(<CanvasPage />, { wrapper: TestWrapper });
    await screen.findByText("Canvas objects: image-1");
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await act(async () => { await flushCanvasSaves().catch(() => {}); });
    expect(await screen.findByText("canvas.saveConflict")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "canvas.saveCopy" }));
    await waitFor(() => expect(saveCanvasDocument.mock.calls.some((call) => call[0] === "canvas-2")).toBe(true));
    const copy = saveCanvasDocument.mock.calls.find((call) => call[0] === "canvas-2");
    expect(copy?.[1].layers[0].objects[0]).toMatchObject({ x:112, y:58 });
    expect(copy?.[3]).toBe(0);
    expect(saveCanvasDocument.mock.calls.filter((call) => call[0] === "canvas-1")).toHaveLength(1);
    await act(async () => { await flushCanvasSaves(); });
  });

  it("persists the latest canvas body when thumbnail rendering fails", async () => {
    getCanvasDocument.mockResolvedValue(canvasDocumentWithImage());
    exportCanvasFrame.mockRejectedValueOnce(new Error("Preview renderer unavailable"));
    render(<CanvasPage/>, {wrapper:TestWrapper});
    await screen.findByText("Canvas objects: image-1");
    fireEvent.click(screen.getByRole("button", {name:"select image"}));
    fireEvent.click(screen.getByRole("button", {name:"move selection"}));
    await act(async()=>{await flushCanvasSaves();});
    expect(saveCanvasDocument).toHaveBeenCalledWith("canvas-1",expect.objectContaining({layers:[expect.objectContaining({objects:[expect.objectContaining({x:112,y:58})]})]}),null,0);
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("restores an unmounted conflicted draft so it can be saved as a copy", async () => {
    const document = canvasDocumentWithImage();
    const pending = createDeferred<typeof document>();
    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument.mockReturnValueOnce(pending.promise);
    const old = render(<CanvasPage/>, {wrapper:TestWrapper});
    await screen.findByText("Canvas objects: image-1");
    fireEvent.click(screen.getByRole("button", {name:"select image"}));
    fireEvent.click(screen.getByRole("button", {name:"move selection"}));
    old.unmount();
    await waitFor(()=>expect(saveCanvasDocument).toHaveBeenCalledTimes(1));
    await act(async()=>pending.reject({kind:"CanvasRevisionConflict",detail:{expected_revision:0,actual_revision:4}} as unknown as Error));
    render(<CanvasPage/>, {wrapper:TestWrapper});
    fireEvent.click(await screen.findByRole("button", {name:"canvas.saveCopy"}));
    await waitFor(()=>expect(saveCanvasDocument.mock.calls.some(call=>call[0]==="canvas-2")).toBe(true));
    expect(saveCanvasDocument.mock.calls.find(call=>call[0]==="canvas-2")?.[1].layers[0].objects[0]).toMatchObject({x:112,y:58});
    expect(saveCanvasDocument.mock.calls.filter(call=>call[0]==="canvas-1")).toHaveLength(1);
    await act(async()=>{await flushCanvasSaves();});
  });
  it("flushes an edit made in the same React batch before a render commits", async () => {
    getCanvasDocument.mockResolvedValue(canvasDocumentWithImage());
    render(<CanvasPage/>, {wrapper:TestWrapper});
    await screen.findByText("Canvas objects: image-1");
    fireEvent.click(screen.getByRole("button", {name:"select image"}));
    await act(async()=>{
      fireEvent.click(screen.getByRole("button", {name:"move selection"}));
      await flushCanvasSaves();
    });
    expect(saveCanvasDocument.mock.calls[0][1].layers[0].objects[0]).toMatchObject({x:112,y:58});
  });

});
