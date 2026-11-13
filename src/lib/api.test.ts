import { beforeEach, describe, expect, it, vi } from "vitest";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import {
  clearLogs,
  onGenerationPartial,
  addPromptFavoriteToFolders,
  archiveConversation,
  createCanvasDocument,
  createModelProviderProfile,
  createConversation,
  createProject,
  createPromptFolder,
  createPromptFavorite,
  deleteCanvasDocument,
  permanentlyDeleteGeneration,
  deleteConversation,
  deleteModelProviderProfile,
  deletePromptFolder,
  deletePromptFavorite,
  getCanvasDocument,
  importCanvasAsset,
  getCanvasGenerationStatus,
  listCanvasDocuments,
  getConversations,
  getModelProviderProfiles,
  getPromptExtractions,
  getProjects,
  extractPromptFromImage,
  getPromptFavoriteFolders,
  getPromptFolders,
  getPromptFavorites,
  installUpdate,
  isUpdateSupported,
  pinConversation,
  renameConversation,
  renameProject,
  searchGenerations,
  removePromptFavoriteFromFolders,
  renameCanvasDocument,
  startPromptAgentSession,
  sendPromptAgentMessage,
  acceptPromptAgentDraft,
  cancelPromptAgentSession,
  getPromptAgentSession,
  saveCanvasDocument,
  saveCanvasExport,
  saveModelProviderProfiles,
  setActiveModelProvider,
  toAssetUrl,
  unpinConversation,
} from "./api";
import type { ModelProviderProfilesState } from "./api";

const tauriApi = vi.hoisted(() => ({
  createdChannels: [] as unknown[],
  convertFileSrc: vi.fn((path: string) => path),
  invoke: vi.fn(),
  Channel: class MockChannel<T = unknown> {
    onmessage: (message: T) => void;

    constructor(onmessage: (message: T) => void = () => {}) {
      this.onmessage = onmessage;
      tauriApi.createdChannels.push(this);
    }
  },
}));

vi.mock("@tauri-apps/api/core", () => tauriApi);

const tauriEvent = vi.hoisted(() => ({
  listen: vi.fn(async () => vi.fn()),
}));

vi.mock("@tauri-apps/api/event", () => tauriEvent);

const localStorageMock = (() => {
  const store = new Map<string, string>();
  return {
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, value);
    },
  };
})();

describe("api updater commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
    tauriApi.createdChannels.length = 0;
    tauriEvent.listen.mockClear();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke: tauriApi.invoke },
    });
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: localStorageMock,
    });
    localStorageMock.clear();
  });

  it("does not subscribe to desktop generation events in a browser preview", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: undefined,
    });
    const unlisten = await onGenerationPartial(vi.fn());
    expect(tauriEvent.listen).not.toHaveBeenCalled();
    expect(typeof unlisten).toBe("function");
  });

  it("passes a Tauri channel to the install update command", async () => {
    const onEvent = vi.fn();
    tauriApi.invoke.mockImplementation(async (_command, args) => {
      if (_command !== "download_update") return;
      const channel = (
        args as { onEvent: InstanceType<typeof tauriApi.Channel> }
      ).onEvent;
      channel.onmessage({
        event: "Progress",
        data: { chunkLength: 4, totalDownloaded: 12 },
      });
    });

    await installUpdate(onEvent);

    expect(tauriApi.createdChannels).toHaveLength(1);
    expect(tauriApi.invoke).toHaveBeenCalledWith("download_update", {
      onEvent: tauriApi.createdChannels[0],
    });
    expect(tauriEvent.listen).not.toHaveBeenCalled();
    expect(onEvent).toHaveBeenCalledWith({
      event: "Progress",
      data: { chunkLength: 4, totalDownloaded: 12 },
    });
  });

  it("checks whether updater commands are supported on the current platform", async () => {
    tauriApi.invoke.mockResolvedValue(false);

    await expect(isUpdateSupported()).resolves.toBe(false);

    expect(tauriApi.invoke).toHaveBeenCalledWith("is_update_supported");
  });
});

describe("api log commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
    tauriApi.convertFileSrc.mockClear();
  });

  it("preserves zero retention when clearing all logs", async () => {
    tauriApi.invoke.mockResolvedValue(0);

    await clearLogs(0);

    expect(tauriApi.invoke).toHaveBeenCalledWith("clear_logs", {
      beforeDays: 0,
    });
  });
});

describe("api model provider profile commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("wraps model provider profile IPC commands", async () => {
    const state: ModelProviderProfilesState = {
      active_provider_id: "provider-1",
      profiles: [
        {
          id: "provider-1",
          name: "OpenAI Official",
          api_key: "sk-provider-1",
          endpoint_settings: {
            mode: "base_url",
            base_url: "https://api.openai.com/v1",
            generation_url: "https://api.openai.com/v1/images/generations",
            edit_url: "https://api.openai.com/v1/images/edits",
          },
        },
      ],
    };

    tauriApi.invoke.mockResolvedValue(state);

    const readState = await getModelProviderProfiles("gpt-image-2");
    expect(JSON.stringify(readState)).not.toContain("sk-provider-1");
    await saveModelProviderProfiles("gpt-image-2", state);
    await createModelProviderProfile("gpt-image-2", "Company Gateway");
    await deleteModelProviderProfile("gpt-image-2", "provider-1");
    await setActiveModelProvider("gpt-image-2", "provider-1");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      1,
      "get_model_provider_profiles",
      { model: "gpt-image-2" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      2,
      "save_model_provider_profiles",
      {
        model: "gpt-image-2",
        activeProviderId: "provider-1",
        profiles: state.profiles.map(({ api_key, ...profile }) => ({ ...profile, credential_update: { action: "replace", value: api_key } })),
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      3,
      "create_model_provider_profile",
      { model: "gpt-image-2", name: "Company Gateway" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      4,
      "delete_model_provider_profile",
      { model: "gpt-image-2", providerId: "provider-1" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      5,
      "set_active_model_provider",
      { model: "gpt-image-2", providerId: "provider-1" },
    );
  });
});

describe("api prompt favorite commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("creates prompt favorites through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue({
      id: "favorite-1",
      prompt: "A silver forest",
      created_at: "2026-04-28T00:00:00Z",
      updated_at: "2026-04-28T00:00:00Z",
    });

    await createPromptFavorite("A silver forest");

    expect(tauriApi.invoke).toHaveBeenCalledWith("create_prompt_favorite", {
      prompt: "A silver forest",
    });
  });

  it("reads and deletes prompt favorites through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue([]);

    await getPromptFavorites("forest", "folder-1");
    await deletePromptFavorite("favorite-1");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(1, "get_prompt_favorites", {
      query: "forest",
      folderId: "folder-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      2,
      "delete_prompt_favorite",
      {
        id: "favorite-1",
      },
    );
  });

  it("extracts prompts from an image through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue({
      id: "extract-1",
      image_path: "/tmp/reference.png",
      prompt: "cinematic portrait",
      llm_config_id: "vision-1",
      created_at: "2026-05-09T00:00:00Z",
      updated_at: "2026-05-09T00:00:00Z",
    });

    await extractPromptFromImage("/tmp/reference.png", "vision-1", "zh-CN");

    expect(tauriApi.invoke).toHaveBeenCalledWith("extract_prompt_from_image", {
      imagePath: "/tmp/reference.png",
      configId: "vision-1",
      language: "zh-CN",
    });
  });

  it("reads prompt extraction history through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue([]);

    await getPromptExtractions(12);

    expect(tauriApi.invoke).toHaveBeenCalledWith("get_prompt_extractions", {
      limit: 12,
    });
  });

  it("manages prompt favorite folders through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue([]);

    await getPromptFolders();
    await createPromptFolder("Characters");
    await addPromptFavoriteToFolders("favorite-1", ["folder-1"]);
    await removePromptFavoriteFromFolders("favorite-1", ["folder-2"]);
    await getPromptFavoriteFolders("favorite-1");
    await deletePromptFolder("folder-3");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(1, "get_prompt_folders");
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(2, "create_prompt_folder", {
      name: "Characters",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      3,
      "add_prompt_favorite_to_folders",
      { favoriteId: "favorite-1", folderIds: ["folder-1"] },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      4,
      "remove_prompt_favorite_from_folders",
      { favoriteId: "favorite-1", folderIds: ["folder-2"] },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      5,
      "get_prompt_favorite_folders",
      { favoriteId: "favorite-1" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(6, "delete_prompt_folder", {
      id: "folder-3",
    });
  });
});

describe("api prompt agent commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("starts a prompt agent session with snake_case request fields", async () => {
    tauriApi.invoke.mockResolvedValueOnce({ session: {}, messages: [] });

    await startPromptAgentSession({
      prompt: "A quiet glass observatory",
      configId: "llm-a",
      conversationId: "conv-a",
      projectId: "project-a",
      sourceImagePaths: ["/tmp/ref.png", "/tmp/other.png"],
      visualSourceImagePaths: ["/tmp/ref.png"],
      textOnly: false,
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("start_prompt_agent_session", {
      request: {
        prompt: "A quiet glass observatory",
        config_id: "llm-a",
        conversation_id: "conv-a",
        project_id: "project-a",
        source_image_paths: ["/tmp/ref.png", "/tmp/other.png"],
        visual_source_image_paths: ["/tmp/ref.png"],
        text_only: false,
      },
    });
  });

  it("wraps prompt agent follow-up, accept, cancel, and history commands", async () => {
    tauriApi.invoke.mockResolvedValue({});

    await sendPromptAgentMessage({
      sessionId: "session-a",
      message: "Make it dusk",
      configId: "llm-a",
      sourceImagePaths: ["/tmp/ref.png"],
      visualSourceImagePaths: [],
      textOnly: true,
    });
    await acceptPromptAgentDraft("session-a", "Final prompt");
    await cancelPromptAgentSession("session-a");
    await getPromptAgentSession("session-a");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      1,
      "send_prompt_agent_message",
      {
        request: {
          session_id: "session-a",
          message: "Make it dusk",
          config_id: "llm-a",
          source_image_paths: ["/tmp/ref.png"],
          visual_source_image_paths: [],
          text_only: true,
        },
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      2,
      "accept_prompt_agent_draft",
      {
        sessionId: "session-a",
        acceptedPrompt: "Final prompt",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      3,
      "cancel_prompt_agent_session",
      {
        sessionId: "session-a",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      4,
      "get_prompt_agent_session",
      {
        sessionId: "session-a",
      },
    );
  });
});

describe("api gallery search commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("forwards simplified gallery filters and optional project scope through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });

    await searchGenerations(
      "sunrise",
      2,
      false,
      {
        model: "gpt-image-2",
        created_from: "2026-05-01",
        created_to: "2026-05-31",
      },
      "project-1",
    );

    expect(tauriApi.invoke).toHaveBeenCalledWith("search_generations", {
      query: "sunrise",
      page: 2,
      onlyDeleted: null,
      filters: {
        model: "gpt-image-2",
        created_from: "2026-05-01",
        created_to: "2026-05-31",
      },
      projectId: "project-1",
    });
  });
});

describe("api project and conversation commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("passes project context through generation history commands", async () => {
    tauriApi.invoke.mockResolvedValue([]);

    await createConversation("Mood board", "project-1");
    await getConversations("forest", "project-1", true);

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(1, "create_conversation", {
      title: "Mood board",
      projectId: "project-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(2, "get_conversations", {
      query: "forest",
      projectId: "project-1",
      includeArchived: true,
    });
  });

  it("wraps conversation management IPC commands", async () => {
    tauriApi.invoke.mockResolvedValue(undefined);

    await renameConversation("conversation-1", "New name");
    await pinConversation("conversation-1");
    await unpinConversation("conversation-1");
    await archiveConversation("conversation-1");
    await deleteConversation("conversation-1");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(1, "rename_conversation", {
      id: "conversation-1",
      title: "New name",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(2, "pin_conversation", {
      id: "conversation-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(3, "unpin_conversation", {
      id: "conversation-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(4, "archive_conversation", {
      id: "conversation-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(5, "delete_conversation", {
      id: "conversation-1",
    });
  });

  it("wraps project IPC commands", async () => {
    tauriApi.invoke.mockResolvedValue([]);

    await getProjects(true);
    await createProject("Launch visuals");
    await renameProject("project-1", "Renamed project");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(1, "get_projects", {
      includeArchived: true,
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(2, "create_project", {
      name: "Launch visuals",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(3, "rename_project", {
      id: "project-1",
      name: "Renamed project",
    });
  });

  it("wraps canvas document IPC commands", async () => {
    tauriApi.invoke.mockResolvedValue({
      id: "canvas-1",
      project_id: "project-1",
      name: "Mood board",
      document_path: "/tmp/canvas-1.json",
      preview_path: "/tmp/canvas-1.png",
      width: 1024,
      height: 1024,
      created_at: "2026-05-12T00:00:00Z",
      updated_at: "2026-05-12T00:00:00Z",
      deleted_at: null,
    });

    await createCanvasDocument("project-1", "Mood board");
    await listCanvasDocuments("project-1");
    await getCanvasDocument("canvas-1");
    await saveCanvasDocument("canvas-1", {
      version: 1,
      viewport: { x: 0, y: 0, scale: 1 },
      frame: { x: 0, y: 0, width: 1024, height: 1024, aspect: "1:1" },
      layers: [],
    }, null, 0);
    await renameCanvasDocument("canvas-1", "Renamed canvas");
    await deleteCanvasDocument("canvas-1");
    await saveCanvasExport("canvas-1", "data:image/png;base64,Zm9v");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      1,
      "create_canvas_document",
      {
        projectId: "project-1",
        name: "Mood board",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      2,
      "list_canvas_documents",
      {
        projectId: "project-1",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(3, "get_canvas_document", {
      id: "canvas-1",
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(4, "save_canvas_document", {
      id: "canvas-1",
      content: {
        version: 1,
        viewport: { x: 0, y: 0, scale: 1 },
        frame: { x: 0, y: 0, width: 1024, height: 1024, aspect: "1:1" },
        layers: [],
      },
      previewPngBase64: null,
      expectedRevision: 0,
    });
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      5,
      "rename_canvas_document",
      {
        id: "canvas-1",
        name: "Renamed canvas",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      6,
      "delete_canvas_document",
      {
        id: "canvas-1",
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(7, "save_canvas_export", {
      documentId: "canvas-1",
      pngBase64: "data:image/png;base64,Zm9v",
    });
  });

  it("falls back to browser storage for canvas documents without Tauri", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: undefined,
    });

    const created = await createCanvasDocument("project-1", "Browser canvas");
    const listed = await listCanvasDocuments("project-1");
    const loaded = await getCanvasDocument(created.id);
    const saved = await saveCanvasDocument(
      created.id,
      {
        version: 1,
        viewport: { x: 0, y: 0, scale: 1 },
        frame: { x: 0, y: 0, width: 768, height: 512, aspect: "3:2" },
        layers: [],
      },
      "data:image/png;base64,Zm9v",
      loaded.revision,
    );
    const exported = await saveCanvasExport(
      created.id,
      "data:image/png;base64,Zm9v",
    );

    expect(created.name).toBe("Browser canvas");
    expect(listed).toHaveLength(1);
    expect(loaded.id).toBe(created.id);
    expect(saved.preview_path).toBe("data:image/png;base64,Zm9v");
    expect(saved.width).toBe(768);
    expect(exported).toBe("data:image/png;base64,Zm9v");
    expect(tauriApi.invoke).not.toHaveBeenCalled();
  });
});

describe("canvas asset and revision contracts", () => {
  it("imports the chosen path through native managed storage", async () => {
    tauriApi.invoke.mockResolvedValueOnce({ id: "asset-1", file_path: "/managed/asset.png" });
    await expect(importCanvasAsset("/external/reference.png")).resolves.toEqual({ id: "asset-1", file_path: "/managed/asset.png" });
    expect(tauriApi.invoke).toHaveBeenLastCalledWith("import_canvas_asset", { path: "/external/reference.png" });
  });
  it("rejects a stale browser save without losing the latest stored content", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: undefined });
    const doc = await createCanvasDocument("revision-test", "revision-test");
    const loaded = await getCanvasDocument(doc.id);
    const latest = { ...loaded.content, viewport: { x: 20, y: 30, scale: 2 } };
    const saved = await saveCanvasDocument(doc.id, latest, null, 0);
    expect(saved.revision).toBe(1);
    await expect(saveCanvasDocument(doc.id, loaded.content, null, 0)).rejects.toMatchObject({ kind: "CanvasRevisionConflict", detail: { expected_revision: 0, actual_revision: 1 } });
    expect((await getCanvasDocument(doc.id)).content.viewport).toEqual(latest.viewport);
  });
});

describe("asset URLs", () => {
  beforeEach(() => {
    tauriApi.convertFileSrc.mockClear();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke: tauriApi.invoke },
    });
  });

  it("lets Tauri encode Windows file paths without rewriting separators", () => {
    const imagePath = String.raw`C:\Users\Chen\AppData\Roaming\com.astrostudio.desktop\images\2026\04\28\image.png`;

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("passes macOS file paths to Tauri unchanged", () => {
    const imagePath =
      "/Users/chen/Library/Application Support/com.astrostudio.desktop/images/2026/04/28/image.png";

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("allows the Windows asset protocol host in the Tauri CSP", () => {
    const csp = tauriConfig.app.security.csp;

    expect(csp).toMatch(/img-src[^;]*data:/);
    expect(csp).toContain("asset:");
    expect(csp).toContain("http://asset.localhost");
  });
});

describe("canvas generation recovery API", () => {
  it("looks up only the exact canvas request for recovery", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: { invoke: tauriApi.invoke },
    });
    tauriApi.invoke.mockReset().mockResolvedValue(null);
    await getCanvasGenerationStatus("document-id", "request-id");
    expect(tauriApi.invoke).toHaveBeenCalledWith(
      "get_canvas_generation_status",
      { documentId: "document-id", requestId: "request-id" },
    );
  });
});

describe("deletion waits for canvas references", () => {
  it("waits for a pending canvas preview and save before deleting", async () => {
    const { queueCanvasSave } = await import("./canvas/persistence");
    let resolve!:()=>void;
    queueCanvasSave("delete-pending",()=>new Promise<void>(yes=>{resolve=yes;}));
    tauriApi.invoke.mockClear();
    const deletion=permanentlyDeleteGeneration("generation-1");
    await vi.waitFor(()=>expect(resolve).toBeTypeOf("function"));
    expect(tauriApi.invoke).not.toHaveBeenCalledWith("permanently_delete_generation",expect.anything());
    resolve();await deletion;
    expect(tauriApi.invoke).toHaveBeenCalledWith("permanently_delete_generation",{id:"generation-1"});
  });
  it("blocks deletion when a canvas save failed", async () => {
    const { queueCanvasSave, waitForCanvasSaves } = await import("./canvas/persistence");
    const write=vi.fn().mockRejectedValueOnce(new Error("disk full")).mockResolvedValue(undefined);
    await expect(queueCanvasSave("delete-failed",write).promise).rejects.toThrow("disk full");
    tauriApi.invoke.mockClear();
    await expect(permanentlyDeleteGeneration("generation-2")).rejects.toThrow("disk full");
    expect(tauriApi.invoke).not.toHaveBeenCalledWith("permanently_delete_generation",expect.anything());
    await waitForCanvasSaves("delete-failed",true,()=>true);
  });
});
