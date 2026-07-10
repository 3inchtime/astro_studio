import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { CanvasDocumentContent } from "../types";
import CanvasPage from "./CanvasPage";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
}

const listCanvasDocuments = vi.fn();
const createCanvasDocument = vi.fn();
const getCanvasDocument = vi.fn();
const saveCanvasDocument = vi.fn();
const saveCanvasExport = vi.fn();
const editImage = vi.fn();
const getImageModel = vi.fn();
const pickSourceImages = vi.fn();
const exportCanvasFrame = vi.fn();
const readImageSize = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number; number?: number; zoom?: number }) =>
      ({
        "canvas.title": "Infinite Canvas",
        "canvas.assetsTitle": "Canvas Assets",
        "canvas.assetsEmpty": "Create a project canvas to start sketching.",
        "canvas.emptyHint":
          "Sketch ideas, frame a region, and send that framed composition into image generation.",
        "canvas.generationTitle": "Generation",
        "canvas.generationEmpty": "Prompt controls and generated results will appear here.",
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
        "canvas.promptPlaceholder": "Describe how to develop this framed sketch...",
        "canvas.noDocumentSelected": "Choose or create a canvas document to begin.",
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
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  hasTauriRuntime: () => true,
  pickSourceImages: (...args: unknown[]) => pickSourceImages(...args),
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
        {content.layers.flatMap((layer) => layer.objects.map((object) => object.id)).join(",") ||
          "none"}
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
    getImageModel.mockReset();
    pickSourceImages.mockReset();
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

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
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
      screen.getByPlaceholderText("Describe how to develop this framed sketch..."),
      { target: { value: "Turn this sketch into a polished cinematic environment" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    expect(await screen.findByRole("status", { name: "Generating..." })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Generating..." })).toBeDisabled();

    await waitFor(() => {
      expect(saveCanvasExport).toHaveBeenCalledWith("canvas-1", expect.any(String));
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

  it("gives the canvas prompt editor a larger writing area", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    const promptEditor = await screen.findByPlaceholderText(
      "Describe how to develop this framed sketch...",
    );

    expect(promptEditor).toHaveClass("min-h-[320px]");
  });

  it("uses the professional editor layout with floating canvas tools", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByLabelText("Canvas workspace")).toHaveClass(
      "min-w-[836px]",
      "grid-cols-[220px_minmax(276px,1fr)_300px]",
    );
    await screen.findByText("Canvas stage");
    expect(screen.getByText("1 canvas")).toBeInTheDocument();
    expect(screen.getByText("0 objects")).toBeInTheDocument();
    expect(screen.getByLabelText("Generation and layers")).toBeInTheDocument();
    expect(screen.getByTestId("canvas-floating-toolbar")).toBeInTheDocument();
  });

  it("localizes the default name when adding a new layer", async () => {
    render(<CanvasPage />, { wrapper: TestWrapper });

    await screen.findByText("0 objects");
    fireEvent.click(screen.getByRole("button", { name: "New Layer" }));

    expect(screen.getByText("Canvas Layer 2")).toBeInTheDocument();
  });

  it("imports an image into the canvas document", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/reference.png"]);

    render(<CanvasPage />, { wrapper: TestWrapper });

    fireEvent.click(await screen.findByRole("button", { name: "Import Image" }));

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
                  image_path: "/tmp/reference.png",
                  original_width: 512,
                  original_height: 512,
                }),
              ],
            }),
          ],
        }),
        expect.any(String),
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

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    fireEvent.click(screen.getByRole("button", { name: "Paste" }));
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
    fireEvent.click(screen.getByRole("button", { name: "Fit Selection" }));

    expect(screen.getByText("400%")).toBeInTheDocument();
    await advanceAutosave();
    expect(saveCanvasDocument).toHaveBeenCalledWith(
      "canvas-1",
      expect.objectContaining({
        viewport: { x: -200, y: -100, scale: 4 },
      }),
      expect.any(String),
    );
  });

  it("brings the selected image to the front and autosaves the object order", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithTwoImages());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("2 objects")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "Bring to Front" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalled();
    const savedContent = saveCanvasDocument.mock.calls.at(-1)?.[1];
    expect(savedContent.layers[0].objects.map((object: { id: string }) => object.id)).toEqual([
      "image-2",
      "image-1",
    ]);
  });

  it("clears selection immediately when switching canvas documents", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = {
      ...canvasDocumentWithImage(),
      id: "canvas-2",
      name: "Second canvas",
      document_path: "/tmp/canvas-2.json",
    };
    let resolveSecondDocument: (document: typeof secondDocument) => void = () => {};

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
    expect(await screen.findByText("Selected objects: image-1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));

    expect(await screen.findByText("Loading canvas...")).toBeInTheDocument();
    expect(screen.queryByText("Canvas stage")).not.toBeInTheDocument();

    await act(async () => {
      resolveSecondDocument(secondDocument);
    });

    expect(await screen.findByText("Selected objects: none")).toBeInTheDocument();
  });

  it("ignores stale document loads that resolve after the selected document", async () => {
    const firstDocument = canvasDocumentWithIdentity("canvas-1", "Mood board", "image-a");
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
    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();

    await act(async () => {
      firstLoad.resolve(firstDocument);
      await firstLoad.promise;
    });

    expect(screen.getByText("Canvas objects: image-1")).toBeInTheDocument();
    expect(screen.getByText("Selected objects: image-1")).toBeInTheDocument();
    expect(screen.queryByText("Canvas objects: image-a")).not.toBeInTheDocument();
  });

  it("blocks edits and autosave until the selected document finishes loading", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");
    const secondLoad = createDeferred<typeof secondDocument>();

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id ? Promise.resolve(firstDocument) : secondLoad.promise,
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    vi.useFakeTimers();
    saveCanvasDocument.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
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
            objects: [expect.objectContaining({ image_path: "/tmp/image-1.png" })],
          }),
        ],
      }),
      expect.any(String),
    );
  });

  it("ignores tool shortcuts until the selected document finishes loading", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");
    const secondLoad = createDeferred<typeof secondDocument>();

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id ? Promise.resolve(firstDocument) : secondLoad.promise,
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
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

  it("serializes same-document saves so the newest snapshot is written last", async () => {
    const document = canvasDocumentWithImage();
    const firstSave = createDeferred<typeof document>();
    const secondSave = createDeferred<typeof document>();

    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument
      .mockImplementationOnce(() => firstSave.promise)
      .mockImplementationOnce(() => secondSave.promise);

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);

    firstSave.resolve(document);
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(saveCanvasDocument.mock.calls[1][0]).toBe("canvas-1");
    expect(saveCanvasDocument.mock.calls[1][1].layers[0].objects[0]).toMatchObject({
      x: 124,
      y: 66,
    });

    secondSave.resolve(document);
    await flushAsyncWork();

    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("flushes a dirty document before query reconciliation selects another", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(documentId === firstDocument.id ? firstDocument : secondDocument),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
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
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    getCanvasDocument.mockResolvedValue(document);
    saveCanvasDocument.mockRejectedValueOnce(
      new Error("secret backend credential must not be rendered"),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
    expect(screen.getByRole("button", { name: "Retry save" })).toBeInTheDocument();
    expect(screen.queryByText(/secret backend credential/i)).not.toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();

    const failedSnapshot = saveCanvasDocument.mock.calls[0][1];
    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(saveCanvasDocument.mock.calls[1][0]).toBe("canvas-1");
    expect(saveCanvasDocument.mock.calls[1][1]).toEqual(failedSnapshot);
    expect(saveCanvasDocument.mock.calls[1][1].layers[0].objects[0]).toMatchObject({
      x: 112,
      y: 58,
    });
    expect(screen.getByText("Saved")).toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();
  });

  it("keeps the old document editable when a switch flush fails", async () => {
    const firstDocument = canvasDocumentWithImage();
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(documentId === firstDocument.id ? firstDocument : secondDocument),
    );
    saveCanvasDocument.mockRejectedValueOnce(new Error("private switch flush failure"));

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));

    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent("Save failed");
    expect(screen.getByRole("button", { name: "Retry save" })).toBeInTheDocument();
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
    );
    expect(
      saveCanvasDocument.mock.calls.every(([documentId]) => documentId === "canvas-1"),
    ).toBe(true);
    expect(screen.queryByText(/private switch flush failure/i)).not.toBeInTheDocument();
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
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");
    const thirdDocument = canvasDocumentWithoutObjects("canvas-3", "Third canvas");
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

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
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
    const secondDocument = canvasDocumentWithoutObjects("canvas-2", "Second canvas");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    listCanvasDocuments.mockResolvedValue([firstDocument, secondDocument]);
    getCanvasDocument.mockImplementation((documentId: string) =>
      documentId === firstDocument.id
        ? Promise.resolve(firstDocument)
        : Promise.reject(new Error("secret load response")),
    );

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Second canvas" }));
    await flushAsyncWork();

    expect(screen.getByRole("alert")).toHaveTextContent("Couldn't load this canvas.");
    expect(screen.getByRole("button", { name: "Retry load" })).toBeInTheDocument();
    expect(screen.queryByText(/secret load response/i)).not.toBeInTheDocument();
    expect(consoleError).not.toHaveBeenCalled();

    getCanvasDocument.mockImplementation((documentId: string) =>
      Promise.resolve(documentId === firstDocument.id ? firstDocument : secondDocument),
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

    expect(await screen.findByText("Canvas objects: image-1")).toBeInTheDocument();
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();
    fireEvent.click(screen.getByRole("button", { name: "move selection" }));
    await advanceAutosave();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Saving...")).toBeInTheDocument();

    firstSave.resolve(document);
    await flushAsyncWork();

    expect(saveCanvasDocument).toHaveBeenCalledTimes(2);
    expect(screen.getByText("Saving...")).toBeInTheDocument();

    secondSave.resolve(document);
    await flushAsyncWork();
    expect(screen.getByText("Saved")).toBeInTheDocument();
  });

  it("clears external selection when its layer becomes locked", async () => {
    getCanvasDocument.mockResolvedValueOnce(canvasDocumentWithImage());

    render(<CanvasPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("1 object")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "select image" }));
    expect(await screen.findByText("Selected objects: image-1")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Lock Layer" }));

    expect(await screen.findByText("Selected objects: none")).toBeInTheDocument();
  });
});
