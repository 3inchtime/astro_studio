import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
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

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
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
        "canvas.layersTitle": "Layers",
        "canvas.frameAspect": "Frame",
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
    activeTool,
    onExport,
  }: {
    activeTool: string;
    onExport: () => Promise<string>;
  }) => (
    <div>
      <div>Canvas stage</div>
      <div>Active tool: {activeTool}</div>
      <button type="button" onClick={() => void onExport()}>
        export stage
      </button>
    </div>
  ),
}));

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
      "grid-cols-[264px_minmax(360px,1fr)_340px]",
    );
    await screen.findByText("Canvas stage");
    expect(screen.getByText("1 canvas")).toBeInTheDocument();
    expect(screen.getByText("0 objects")).toBeInTheDocument();
    expect(screen.getByLabelText("Generation and layers")).toBeInTheDocument();
    expect(screen.getByTestId("canvas-floating-toolbar")).toBeInTheDocument();
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
});
