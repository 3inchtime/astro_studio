import { invoke } from "@tauri-apps/api/core";
import { flushCanvasSaves } from "../../lib/canvas/persistence";
import { hasTauriRuntime } from "../../lib/desktop";
import type { CanvasDocument, CanvasDocumentContent, CanvasDocumentWithContent } from "../../types";

const BROWSER_CANVAS_STORAGE_KEY = "astro-studio.browser.canvas-documents.v1";
const BROWSER_DEFAULT_PROJECT_ID = "browser-default-project";

function defaultCanvasContent(): CanvasDocumentContent {
  return {
    version: 2,
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
      revision: 0,
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
          document.project_id === browserProjectId &&
          document.deleted_at === null,
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
    const document = readBrowserCanvasDocuments().find(
      (entry) => entry.id === id,
    );
    if (!document) {
      throw new Error(`Canvas document not found: ${id}`);
    }
    return {
      ...document,
      content: cloneCanvasContent(document.content),
      revision: document.revision ?? 0,
    };
  }

  return invoke("get_canvas_document", { id });
}

export async function saveCanvasDocument(
  id: string,
  content: CanvasDocumentContent,
  previewPngBase64: string | null | undefined,
  expectedRevision: number,
): Promise<CanvasDocument> {
  if (!hasTauriRuntime()) {
    const documents = readBrowserCanvasDocuments();
    const current = documents.find((document) => document.id === id);
    if (!current) throw new Error(`Canvas document not found: ${id}`);
    if ((current.revision ?? 0) !== expectedRevision) {
      throw { kind: "CanvasRevisionConflict", detail: { expected_revision: expectedRevision, actual_revision: current.revision ?? 0 } };
    }
    const nextDocuments = documents.map((document) =>
      document.id === id
        ? {
            ...document,
            revision: expectedRevision + 1,
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
    expectedRevision,
  });
}

export async function importCanvasAsset(path: string): Promise<{ id: string; file_path: string }> {
  return invoke("import_canvas_asset", { path });
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
  await flushCanvasSaves();
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

export interface CanvasGenerationStatus {
  error_code?: string | null;
  generation_id: string | null;
  status: string;
  error: string | null;
  images: import("../../types").GeneratedImage[];
}
export async function getCanvasGenerationStatus(
  documentId: string,
  requestId: string,
): Promise<CanvasGenerationStatus | null> {
  if (!hasTauriRuntime()) return null;
  return invoke("get_canvas_generation_status", { documentId, requestId });
}
