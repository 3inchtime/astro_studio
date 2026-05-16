import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import type {
  CanvasDocument,
  CanvasDocumentContent,
  CanvasDocumentWithContent,
  EditSourceImage,
  GenerationParams,
  GenerationSearchFilters,
  SearchResult,
  Conversation,
  Project,
  GenerationResult,
  Folder,
  LogSearchResult,
  LogSettings,
  LogEntry,
  RuntimeLogEntry,
  GenerateResponse,
  TrashSettings,
  AppFontSize,
  ImageModel,
  ImageInputFidelity,
  EndpointSettings,
  ModelProviderProfile,
  ModelProviderProfilesState,
  PromptFavorite,
  PromptExtraction,
  LlmConfig,
  PromptAgentSession,
  PromptAgentTurnResponse,
} from "../types";

const BROWSER_CANVAS_STORAGE_KEY = "astro-studio.browser.canvas-documents.v1";
const BROWSER_DEFAULT_PROJECT_ID = "browser-default-project";

export interface UpdateMetadata {
  version: string;
  current_version: string;
  body: string | null;
  date: string | null;
}

export type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number; totalDownloaded: number } }
  | { event: "Finished" };

export function hasTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return true;
  }

  return Boolean(
    (window as typeof window & {
      __TAURI_INTERNALS__?: { invoke?: unknown };
    }).__TAURI_INTERNALS__?.invoke,
  );
}

export function toAssetUrl(filePath: string): string {
  if (!hasTauriRuntime()) {
    return filePath;
  }
  return convertFileSrc(filePath);
}

function defaultCanvasContent(): CanvasDocumentContent {
  return {
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
  };
}

function cloneCanvasContent(
  content: CanvasDocumentContent,
): CanvasDocumentContent {
  return JSON.parse(JSON.stringify(content)) as CanvasDocumentContent;
}

function readBrowserCanvasDocuments(): CanvasDocumentWithContent[] {
  if (typeof window === "undefined") {
    return [];
  }

  const raw = window.localStorage.getItem(BROWSER_CANVAS_STORAGE_KEY);
  if (!raw) {
    return [];
  }

  try {
    return JSON.parse(raw) as CanvasDocumentWithContent[];
  } catch {
    return [];
  }
}

function writeBrowserCanvasDocuments(documents: CanvasDocumentWithContent[]) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(
    BROWSER_CANVAS_STORAGE_KEY,
    JSON.stringify(documents),
  );
}

function browserCanvasProjectId(projectId?: string | null): string {
  return projectId?.trim() ? projectId : BROWSER_DEFAULT_PROJECT_ID;
}

function browserCanvasMetadata(
  document: CanvasDocumentWithContent,
): CanvasDocument {
  const { content, ...metadata } = document;
  return metadata;
}

export async function saveApiKey(key: string): Promise<void> {
  await invoke("save_api_key", { key });
}

export async function getApiKey(): Promise<string | null> {
  return invoke("get_api_key");
}

export async function saveBaseUrl(url: string): Promise<void> {
  await invoke("save_base_url", { url });
}

export async function getBaseUrl(): Promise<string> {
  return invoke("get_base_url");
}

export async function getEndpointSettings(): Promise<EndpointSettings> {
  return invoke("get_endpoint_settings");
}

export async function getModelApiKey(
  model: ImageModel,
): Promise<string | null> {
  return invoke("get_model_api_key", { model });
}

export async function saveModelApiKey(
  model: ImageModel,
  key: string,
): Promise<void> {
  await invoke("save_model_api_key", { model, key });
}

export async function saveEndpointSettings(
  settings: EndpointSettings,
): Promise<void> {
  await invoke("save_endpoint_settings", {
    mode: settings.mode,
    baseUrl: settings.base_url,
    generationUrl: settings.generation_url,
    editUrl: settings.edit_url,
  });
}

export async function getModelEndpointSettings(
  model: ImageModel,
): Promise<EndpointSettings> {
  return invoke("get_model_endpoint_settings", { model });
}

export async function saveModelEndpointSettings(
  model: ImageModel,
  settings: EndpointSettings,
): Promise<void> {
  await invoke("save_model_endpoint_settings", {
    model,
    mode: settings.mode,
    baseUrl: settings.base_url,
    generationUrl: settings.generation_url,
    editUrl: settings.edit_url,
  });
}

export async function getModelProviderProfiles(
  model: ImageModel,
): Promise<ModelProviderProfilesState> {
  return invoke("get_model_provider_profiles", { model });
}

export async function saveModelProviderProfiles(
  model: ImageModel,
  state: ModelProviderProfilesState,
): Promise<ModelProviderProfilesState> {
  return invoke("save_model_provider_profiles", {
    model,
    activeProviderId: state.active_provider_id,
    profiles: state.profiles,
  });
}

export async function createModelProviderProfile(
  model: ImageModel,
  name: string,
): Promise<ModelProviderProfilesState> {
  return invoke("create_model_provider_profile", { model, name });
}

export async function deleteModelProviderProfile(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("delete_model_provider_profile", { model, providerId });
}

export async function setActiveModelProvider(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("set_active_model_provider", { model, providerId });
}

export type { ModelProviderProfile, ModelProviderProfilesState };

export async function getImageModel(): Promise<ImageModel> {
  return invoke("get_image_model");
}

export async function saveImageModel(model: ImageModel): Promise<void> {
  await invoke("save_image_model", { model });
}

export async function generateImage(
  params: GenerationParams,
): Promise<GenerateResponse> {
  return invoke("generate_image", {
    prompt: params.prompt,
    model: params.model,
    size: params.size,
    quality: params.quality,
    background: params.background,
    outputFormat: params.outputFormat,
    outputCompression: params.outputCompression,
    moderation: params.moderation,
    imageCount: params.imageCount,
    conversationId: params.conversationId ?? null,
    projectId: params.projectId ?? null,
  });
}

export async function editImage(
  params: GenerationParams & {
    sourceImagePaths: string[];
    inputFidelity?: ImageInputFidelity;
  },
): Promise<GenerateResponse> {
  if (!hasTauriRuntime()) {
    throw new Error("Image editing is only available in the desktop app.");
  }

  return invoke("edit_image", {
    prompt: params.prompt,
    model: params.model,
    sourceImagePaths: params.sourceImagePaths,
    size: params.size,
    quality: params.quality,
    background: params.background,
    inputFidelity: params.inputFidelity,
    outputFormat: params.outputFormat,
    outputCompression: params.outputCompression,
    moderation: params.moderation,
    imageCount: params.imageCount,
    conversationId: params.conversationId ?? null,
    projectId: params.projectId ?? null,
  });
}

export async function pickSourceImages(): Promise<string[]> {
  if (!hasTauriRuntime()) {
    return pickBrowserSourceImages();
  }

  return invoke("pick_source_images");
}

async function pickBrowserSourceImages(): Promise<string[]> {
  if (typeof document === "undefined") {
    return [];
  }

  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.multiple = true;
    input.style.position = "fixed";
    input.style.left = "-9999px";
    document.body.appendChild(input);

    input.addEventListener(
      "change",
      async () => {
        const files = Array.from(input.files ?? []);
        const results = await Promise.all(files.map(fileToDataUrl));
        input.remove();
        resolve(results.filter((value): value is string => Boolean(value)));
      },
      { once: true },
    );

    input.addEventListener(
      "cancel",
      () => {
        input.remove();
        resolve([]);
      },
      { once: true },
    );

    input.click();
  });
}

async function fileToDataUrl(file: File): Promise<string | null> {
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () =>
      resolve(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => resolve(null);
    reader.readAsDataURL(file);
  });
}

export async function createPromptFavorite(
  prompt: string,
): Promise<PromptFavorite> {
  return invoke("create_prompt_favorite", { prompt });
}

export async function getPromptFavorites(
  query?: string,
  folderId?: string,
): Promise<PromptFavorite[]> {
  return invoke("get_prompt_favorites", {
    query: query || null,
    folderId: folderId || null,
  });
}

export async function deletePromptFavorite(id: string): Promise<void> {
  await invoke("delete_prompt_favorite", { id });
}

export async function extractPromptFromImage(
  imagePath: string,
  configId: string,
  language: string,
): Promise<PromptExtraction> {
  return invoke("extract_prompt_from_image", {
    imagePath,
    configId,
    language,
  });
}

export async function getPromptExtractions(
  limit = 20,
): Promise<PromptExtraction[]> {
  return invoke("get_prompt_extractions", { limit });
}

export async function createPromptFolder(name: string): Promise<Folder> {
  return invoke("create_prompt_folder", { name });
}

export async function renamePromptFolder(
  id: string,
  name: string,
): Promise<void> {
  return invoke("rename_prompt_folder", { id, name });
}

export async function deletePromptFolder(id: string): Promise<void> {
  await invoke("delete_prompt_folder", { id });
}

export async function getPromptFolders(): Promise<Folder[]> {
  return invoke("get_prompt_folders");
}

export async function addPromptFavoriteToFolders(
  favoriteId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("add_prompt_favorite_to_folders", { favoriteId, folderIds });
}

export async function removePromptFavoriteFromFolders(
  favoriteId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("remove_prompt_favorite_from_folders", { favoriteId, folderIds });
}

export async function getPromptFavoriteFolders(
  favoriteId: string,
): Promise<string[]> {
  return invoke("get_prompt_favorite_folders", { favoriteId });
}

export async function searchGenerations(
  query?: string,
  page?: number,
  onlyDeleted?: boolean,
  filters?: GenerationSearchFilters,
  projectId?: string | null,
): Promise<SearchResult> {
  return invoke("search_generations", {
    query: query || null,
    page,
    onlyDeleted: onlyDeleted || null,
    filters: filters || null,
    projectId: projectId || null,
  });
}

export async function deleteGeneration(id: string): Promise<void> {
  await invoke("delete_generation", { id });
}

export async function restoreGeneration(id: string): Promise<void> {
  await invoke("restore_generation", { id });
}

export async function permanentlyDeleteGeneration(id: string): Promise<void> {
  await invoke("permanently_delete_generation", { id });
}

export async function createConversation(
  title?: string,
  projectId?: string | null,
): Promise<Conversation> {
  return invoke("create_conversation", {
    title: title || null,
    projectId: projectId || null,
  });
}

export async function getConversations(
  query?: string,
  projectId?: string | null,
  includeArchived?: boolean,
): Promise<Conversation[]> {
  return invoke("get_conversations", {
    query: query || null,
    projectId: projectId || null,
    includeArchived: includeArchived || null,
  });
}

export async function getConversationGenerations(
  conversationId: string,
): Promise<GenerationResult[]> {
  return invoke("get_conversation_generations", { conversationId });
}

export async function renameConversation(
  id: string,
  title: string,
): Promise<void> {
  await invoke("rename_conversation", { id, title });
}

export async function moveConversationToProject(
  id: string,
  projectId: string,
): Promise<void> {
  await invoke("move_conversation_to_project", { id, projectId });
}

export async function archiveConversation(id: string): Promise<void> {
  await invoke("archive_conversation", { id });
}

export async function unarchiveConversation(id: string): Promise<void> {
  await invoke("unarchive_conversation", { id });
}

export async function pinConversation(id: string): Promise<void> {
  await invoke("pin_conversation", { id });
}

export async function unpinConversation(id: string): Promise<void> {
  await invoke("unpin_conversation", { id });
}

export async function deleteConversation(id: string): Promise<void> {
  await invoke("delete_conversation", { id });
}

export async function createProject(name?: string): Promise<Project> {
  return invoke("create_project", { name: name || null });
}

export async function getProjects(
  includeArchived?: boolean,
): Promise<Project[]> {
  return invoke("get_projects", { includeArchived: includeArchived || null });
}

export async function renameProject(
  id: string,
  name: string,
): Promise<void> {
  await invoke("rename_project", { id, name });
}

export async function archiveProject(id: string): Promise<void> {
  await invoke("archive_project", { id });
}

export async function unarchiveProject(id: string): Promise<void> {
  await invoke("unarchive_project", { id });
}

export async function pinProject(id: string): Promise<void> {
  await invoke("pin_project", { id });
}

export async function unpinProject(id: string): Promise<void> {
  await invoke("unpin_project", { id });
}

export async function deleteProject(id: string): Promise<void> {
  await invoke("delete_project", { id });
}

export async function createCanvasDocument(
  projectId?: string | null,
  name?: string | null,
): Promise<CanvasDocument> {
  if (!hasTauriRuntime()) {
    const documents = readBrowserCanvasDocuments();
    const timestamp = new Date().toISOString();
    const browserProjectId = browserCanvasProjectId(projectId);
    const document: CanvasDocumentWithContent = {
      id: crypto.randomUUID(),
      project_id: browserProjectId,
      name: name?.trim() || "Untitled Canvas",
      document_path: `browser-canvas://${crypto.randomUUID()}.json`,
      preview_path: null,
      width: 1024,
      height: 1024,
      created_at: timestamp,
      updated_at: timestamp,
      deleted_at: null,
      content: defaultCanvasContent(),
    };
    writeBrowserCanvasDocuments([document, ...documents]);
    return browserCanvasMetadata(document);
  }

  return invoke("create_canvas_document", {
    projectId: projectId || null,
    name: name || null,
  });
}

export async function listCanvasDocuments(
  projectId?: string | null,
): Promise<CanvasDocument[]> {
  if (!hasTauriRuntime()) {
    const browserProjectId = browserCanvasProjectId(projectId);
    return readBrowserCanvasDocuments()
      .filter(
        (document) =>
          document.project_id === browserProjectId && document.deleted_at === null,
      )
      .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
      .map(browserCanvasMetadata);
  }

  return invoke("list_canvas_documents", {
    projectId: projectId || null,
  });
}

export async function getCanvasDocument(
  id: string,
): Promise<CanvasDocumentWithContent> {
  if (!hasTauriRuntime()) {
    const document = readBrowserCanvasDocuments().find((entry) => entry.id === id);
    if (!document) {
      throw new Error(`Canvas document not found: ${id}`);
    }
    return {
      ...document,
      content: cloneCanvasContent(document.content),
    };
  }

  return invoke("get_canvas_document", { id });
}

export async function saveCanvasDocument(
  id: string,
  content: CanvasDocumentContent,
  previewPngBase64?: string | null,
): Promise<CanvasDocument> {
  if (!hasTauriRuntime()) {
    const documents = readBrowserCanvasDocuments();
    const nextDocuments = documents.map((document) =>
      document.id === id
        ? {
            ...document,
            preview_path: previewPngBase64 || document.preview_path,
            width: Math.round(content.frame.width),
            height: Math.round(content.frame.height),
            updated_at: new Date().toISOString(),
            content: cloneCanvasContent(content),
          }
        : document,
    );
    writeBrowserCanvasDocuments(nextDocuments);
    const updated = nextDocuments.find((document) => document.id === id);
    if (!updated) {
      throw new Error(`Canvas document not found: ${id}`);
    }
    return browserCanvasMetadata(updated);
  }

  return invoke("save_canvas_document", {
    id,
    content,
    previewPngBase64: previewPngBase64 || null,
  });
}

export async function renameCanvasDocument(
  id: string,
  name: string,
): Promise<CanvasDocument> {
  if (!hasTauriRuntime()) {
    const documents = readBrowserCanvasDocuments();
    const nextDocuments = documents.map((document) =>
      document.id === id
        ? {
            ...document,
            name,
            updated_at: new Date().toISOString(),
          }
        : document,
    );
    writeBrowserCanvasDocuments(nextDocuments);
    const updated = nextDocuments.find((document) => document.id === id);
    if (!updated) {
      throw new Error(`Canvas document not found: ${id}`);
    }
    return browserCanvasMetadata(updated);
  }

  return invoke("rename_canvas_document", { id, name });
}

export async function deleteCanvasDocument(id: string): Promise<void> {
  if (!hasTauriRuntime()) {
    const timestamp = new Date().toISOString();
    writeBrowserCanvasDocuments(
      readBrowserCanvasDocuments().map((document) =>
        document.id === id
          ? { ...document, deleted_at: timestamp, updated_at: timestamp }
          : document,
      ),
    );
    return;
  }

  await invoke("delete_canvas_document", { id });
}

export async function saveCanvasExport(
  documentId: string,
  pngBase64: string,
): Promise<string> {
  if (!hasTauriRuntime()) {
    return pngBase64;
  }

  return invoke("save_canvas_export", { documentId, pngBase64 });
}

export async function copyImageToClipboard(imagePath: string): Promise<void> {
  await invoke("copy_image_to_clipboard", { imagePath });
}

export async function saveImageToFile(imagePath: string): Promise<void> {
  await invoke("save_image_to_file", { imagePath });
}

export function messageImageToEditSource(image: {
  path: string;
  imageId?: string;
  generationId?: string;
}): EditSourceImage {
  const normalizedPath = image.path.replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").pop() || "source-image";

  return {
    id: `${image.imageId ?? image.generationId ?? "source"}:${normalizedPath}`,
    path: image.path,
    label: fileName,
    imageId: image.imageId,
    generationId: image.generationId,
  };
}

function onGenerationEvent<T>(event: string, handler: (data: T) => void) {
  return listen(event, (e) => handler(e.payload as T));
}

export function onGenerationProgress(
  handler: (data: { generation_id: string; status: string }) => void,
) {
  return onGenerationEvent("generation:progress", handler);
}

export function onGenerationComplete(
  handler: (data: { generation_id: string; status: string }) => void,
) {
  return onGenerationEvent("generation:complete", handler);
}

export function onGenerationFailed(
  handler: (data: { generation_id: string; error: string }) => void,
) {
  return onGenerationEvent("generation:failed", handler);
}

export function onRuntimeLog(handler: (data: RuntimeLogEntry) => void) {
  return onGenerationEvent("runtime-log:new", handler);
}

export async function createFolder(name: string): Promise<Folder> {
  return invoke("create_folder", { name });
}

export async function renameFolder(id: string, name: string): Promise<void> {
  return invoke("rename_folder", { id, name });
}

export async function deleteFolder(id: string): Promise<void> {
  return invoke("delete_folder", { id });
}

export async function getFolders(): Promise<Folder[]> {
  return invoke("get_folders");
}

export async function addImageToFolders(
  imageId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("add_image_to_folders", { imageId, folderIds });
}

export async function removeImageFromFolders(
  imageId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("remove_image_from_folders", { imageId, folderIds });
}

export async function getImageFolders(imageId: string): Promise<string[]> {
  return invoke("get_image_folders", { imageId });
}

export async function getFavoriteImages(
  folderId?: string,
  query?: string,
  page?: number,
): Promise<SearchResult> {
  return invoke("get_favorite_images", {
    folderId: folderId || null,
    query: query || null,
    page,
  });
}

export async function getLogs(
  logType?: string,
  level?: string,
  page?: number,
  pageSize?: number,
): Promise<LogSearchResult> {
  return invoke("get_logs", {
    logType: logType || null,
    level: level || null,
    page,
    pageSize,
  });
}

export async function getRuntimeLogs(limit?: number): Promise<RuntimeLogEntry[]> {
  return invoke("get_runtime_logs", { limit });
}

export async function getLogDetail(id: string): Promise<LogEntry> {
  return invoke("get_log_detail", { id });
}

export async function readLogResponseFile(path: string): Promise<string> {
  return invoke("read_log_response_file", { path });
}

export async function clearLogs(beforeDays?: number): Promise<number> {
  return invoke("clear_logs", { beforeDays: beforeDays ?? null });
}

export async function getLogSettings(): Promise<LogSettings> {
  return invoke("get_log_settings");
}

export async function saveLogSettings(
  enabled: boolean,
  retentionDays: number,
): Promise<void> {
  await invoke("save_log_settings", { enabled, retentionDays });
}

export async function getTrashSettings(): Promise<TrashSettings> {
  return invoke("get_trash_settings");
}

export async function saveTrashSettings(retentionDays: number): Promise<void> {
  await invoke("save_trash_settings", { retentionDays });
}

export async function getFontSize(): Promise<AppFontSize> {
  return invoke("get_font_size");
}

export async function saveFontSize(fontSize: AppFontSize): Promise<void> {
  await invoke("save_font_size", { fontSize });
}

export async function getLlmConfigs(): Promise<LlmConfig[]> {
  return invoke("get_llm_configs");
}

export async function saveLlmConfigs(configs: LlmConfig[]): Promise<void> {
  return invoke("save_llm_configs", { configs });
}

export async function optimizePrompt(
  prompt: string,
  configId: string,
  imagePaths?: string[],
): Promise<string> {
  return invoke("optimize_prompt", {
    prompt,
    configId,
    imagePaths: imagePaths ?? null,
  });
}

export async function startPromptAgentSession(params: {
  prompt: string;
  configId: string;
  conversationId?: string | null;
  projectId?: string | null;
  sourceImagePaths?: string[];
}): Promise<PromptAgentTurnResponse> {
  return invoke("start_prompt_agent_session", {
    request: {
      prompt: params.prompt,
      config_id: params.configId,
      conversation_id: params.conversationId ?? null,
      project_id: params.projectId ?? null,
      source_image_paths: params.sourceImagePaths ?? [],
    },
  });
}

export async function sendPromptAgentMessage(params: {
  sessionId: string;
  message: string;
  configId: string;
  sourceImagePaths?: string[];
}): Promise<PromptAgentTurnResponse> {
  return invoke("send_prompt_agent_message", {
    request: {
      session_id: params.sessionId,
      message: params.message,
      config_id: params.configId,
      source_image_paths: params.sourceImagePaths ?? [],
    },
  });
}

export async function acceptPromptAgentDraft(
  sessionId: string,
  acceptedPrompt: string,
): Promise<PromptAgentSession> {
  return invoke("accept_prompt_agent_draft", {
    sessionId,
    acceptedPrompt,
  });
}

export async function cancelPromptAgentSession(
  sessionId: string,
): Promise<PromptAgentSession> {
  return invoke("cancel_prompt_agent_session", { sessionId });
}

export async function getPromptAgentSession(
  sessionId: string,
): Promise<PromptAgentTurnResponse> {
  return invoke("get_prompt_agent_session", { sessionId });
}

export async function checkForUpdate(): Promise<UpdateMetadata | null> {
  return invoke("check_update");
}

export async function isUpdateSupported(): Promise<boolean> {
  return invoke("is_update_supported");
}

export async function installUpdate(
  onEvent: (event: DownloadEvent) => void,
): Promise<void> {
  const channel = new Channel<DownloadEvent>(onEvent);
  await invoke("install_update", { onEvent: channel });
}

export async function relaunchApp(): Promise<void> {
  await invoke("relaunch_app");
}
