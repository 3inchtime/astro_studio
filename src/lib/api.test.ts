import { beforeEach, describe, expect, it, vi } from "vitest";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import {
  clearLogs,
  addPromptFavoriteToFolders,
  archiveConversation,
  createConversation,
  createProject,
  createPromptFolder,
  createPromptFavorite,
  deleteConversation,
  deletePromptFolder,
  deletePromptFavorite,
  getConversations,
  getProjects,
  getPromptFavoriteFolders,
  getPromptFolders,
  getPromptFavorites,
  pinConversation,
  renameConversation,
  renameProject,
  searchGenerations,
  removePromptFavoriteFromFolders,
  toAssetUrl,
  unpinConversation,
} from "./api";

const tauriApi = vi.hoisted(() => ({
  convertFileSrc: vi.fn((path: string) => path),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => tauriApi);

describe("api log commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
    tauriApi.convertFileSrc.mockClear();
  });

  it("preserves zero retention when clearing all logs", async () => {
    tauriApi.invoke.mockResolvedValue(0);

    await clearLogs(0);

    expect(tauriApi.invoke).toHaveBeenCalledWith("clear_logs", { beforeDays: 0 });
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
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(2, "delete_prompt_favorite", {
      id: "favorite-1",
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

describe("api gallery search commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("forwards advanced gallery filters through Tauri IPC", async () => {
    tauriApi.invoke.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });

    await searchGenerations("sunrise", 2, false, {
      model: "gpt-image-2",
      request_kind: "edit",
      status: "completed",
      size: "1024x1024",
      quality: "high",
      background: "transparent",
      output_format: "webp",
      moderation: "low",
      input_fidelity: "high",
      source_image_count: "2",
      created_from: "2026-04-01",
      created_to: "2026-04-30",
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith("search_generations", {
      query: "sunrise",
      page: 2,
      onlyDeleted: null,
      filters: {
        model: "gpt-image-2",
        request_kind: "edit",
        status: "completed",
        size: "1024x1024",
        quality: "high",
        background: "transparent",
        output_format: "webp",
        moderation: "low",
        input_fidelity: "high",
        source_image_count: "2",
        created_from: "2026-04-01",
        created_to: "2026-04-30",
      },
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
});

describe("asset URLs", () => {
  beforeEach(() => {
    tauriApi.convertFileSrc.mockClear();
  });

  it("lets Tauri encode Windows file paths without rewriting separators", () => {
    const imagePath = String.raw`C:\Users\Chen\AppData\Roaming\com.astrostudio.desktop\images\2026\04\28\image.png`;

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("passes macOS file paths to Tauri unchanged", () => {
    const imagePath = "/Users/chen/Library/Application Support/com.astrostudio.desktop/images/2026/04/28/image.png";

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("allows the Windows asset protocol host in the Tauri CSP", () => {
    const csp = tauriConfig.app.security.csp;

    expect(csp).toContain("asset:");
    expect(csp).toContain("http://asset.localhost");
  });
});
