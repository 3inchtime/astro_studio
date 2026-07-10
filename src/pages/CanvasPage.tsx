import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  createCanvasDocument,
  editImage,
  getImageModel,
  getCanvasDocument,
  hasTauriRuntime,
  pickSourceImages,
  saveCanvasDocument,
  saveCanvasExport,
} from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import CanvasAssetSidebar from "../components/canvas/CanvasAssetSidebar";
import CanvasGenerationPanel from "../components/canvas/CanvasGenerationPanel";
import CanvasLayersPanel from "../components/canvas/CanvasLayersPanel";
import CanvasStage from "../components/canvas/CanvasStage";
import CanvasToolbar from "../components/canvas/CanvasToolbar";
import { copyCanvasObjects, pasteCanvasObjects } from "../lib/canvas/clipboard";
import type { CanvasClipboard } from "../lib/canvas/clipboard";
import { getCombinedCanvasBounds } from "../lib/canvas/bounds";
import {
  cloneCanvasDocumentContent,
  createCanvasDocumentContent,
  createCanvasLayer,
  createImageObject,
  getActiveLayer,
  removeCanvasObjects,
  resetImageObjectAspect,
  sanitizeCanvasDocumentContent,
  updateImageObject,
} from "../lib/canvas/document";
import { exportCanvasFrame, readImageSize } from "../lib/canvas/export";
import { clampZoom, fitViewportToCanvasRect } from "../lib/canvas/frame";
import {
  createHistory,
  pushHistory,
  redoHistory,
  replaceHistory,
  undoHistory,
} from "../lib/canvas/history";
import { reorderCanvasObjects } from "../lib/canvas/ordering";
import type { CanvasOrderDirection } from "../lib/canvas/ordering";
import { reconcileSelectedObjectIds } from "../lib/canvas/selection";
import { translateCanvasObjects } from "../lib/canvas/transforms";
import { useCanvasDocumentsQuery } from "../lib/queries/canvasDocuments";
import type {
  CanvasDocumentContent,
  CanvasFrame,
  CanvasLayer,
  CanvasStrokeObject,
  CanvasTool,
  ImageModel,
} from "../types";

export default function CanvasPage() {
  const { t } = useTranslation();
  const { activeProjectId } = useLayoutContext();
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [imageModel, setImageModel] = useState<ImageModel>("gpt-image-2");
  const [isGenerating, setIsGenerating] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isDirty, setIsDirty] = useState(false);
  const [activeTool, setActiveTool] = useState<CanvasTool>("brush");
  const [strokeColor, setStrokeColor] = useState("#1f2937");
  const [strokeSize, setStrokeSize] = useState(6);
  const [activeLayerId, setActiveLayerId] = useState<string | null>(null);
  const [loadedDocumentId, setLoadedDocumentId] = useState<string | null>(null);
  const [selectedObjectIds, setSelectedObjectIds] = useState<string[]>([]);
  const [clipboard, setClipboard] = useState<CanvasClipboard | null>(null);
  const [stageSize, setStageSize] = useState({ width: 960, height: 640 });
  const isDesktopRuntime = hasTauriRuntime();
  const [history, setHistory] = useState(() =>
    createHistory<CanvasDocumentContent>(createCanvasDocumentContent()),
  );
  const saveTimerRef = useRef<number | null>(null);
  const loadRequestTokenRef = useRef(0);
  const saveOperationTokenRef = useRef(0);
  const selectedDocumentIdRef = useRef<string | null>(selectedDocumentId);
  const loadedDocumentIdRef = useRef<string | null>(loadedDocumentId);
  const historyPresentRef = useRef(history.present);
  const isDirtyRef = useRef(isDirty);
  const activeSaveRef = useRef<{
    documentId: string;
    snapshot: CanvasDocumentContent;
    token: number;
    promise: Promise<void>;
  } | null>(null);
  const { data: documents = [], refetch } = useCanvasDocumentsQuery(activeProjectId);

  selectedDocumentIdRef.current = selectedDocumentId;
  loadedDocumentIdRef.current = loadedDocumentId;
  historyPresentRef.current = history.present;
  isDirtyRef.current = isDirty;

  useEffect(() => {
    getImageModel()
      .then((model) => setImageModel(model))
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!documents.length) {
      setSelectedDocumentId(null);
      return;
    }

    setSelectedDocumentId((current) =>
      current && documents.some((document) => document.id === current)
        ? current
        : documents[0].id,
    );
  }, [documents]);

  useEffect(() => {
    const requestToken = ++loadRequestTokenRef.current;
    clearSaveTimer();
    setSelectedObjectIds((current) => (current.length ? [] : current));
    loadedDocumentIdRef.current = null;
    setLoadedDocumentId(null);
    setActiveLayerId(null);
    setDirtyState(false);
    setIsSaving(false);

    if (!selectedDocumentId) {
      return;
    }

    const requestedDocumentId = selectedDocumentId;
    getCanvasDocument(requestedDocumentId)
      .then((document) => {
        if (
          loadRequestTokenRef.current !== requestToken ||
          selectedDocumentIdRef.current !== requestedDocumentId
        ) {
          return;
        }

        const nextContent = sanitizeCanvasDocumentContent(document.content);
        historyPresentRef.current = nextContent;
        loadedDocumentIdRef.current = requestedDocumentId;
        isDirtyRef.current = false;
        setHistory(createHistory(nextContent));
        setActiveLayerId(nextContent.layers[0]?.id ?? null);
        setLoadedDocumentId(requestedDocumentId);
        setSelectedObjectIds([]);
        setIsDirty(false);
        setIsSaving(false);
      })
      .catch(() => {});
  }, [selectedDocumentId]);

  useEffect(() => {
    if (
      !selectedDocumentId ||
      loadedDocumentId !== selectedDocumentId ||
      !isDirty
    ) {
      return;
    }

    const documentId = loadedDocumentId;
    const snapshot = history.present;
    clearSaveTimer();

    saveTimerRef.current = window.setTimeout(() => {
      saveTimerRef.current = null;
      if (
        selectedDocumentIdRef.current === documentId &&
        loadedDocumentIdRef.current === documentId &&
        historyPresentRef.current === snapshot &&
        isDirtyRef.current
      ) {
        void persistDocument(documentId, snapshot);
      }
    }, 500);

    return clearSaveTimer;
  }, [history.present, isDirty, loadedDocumentId, selectedDocumentId]);

  const selectedDocument =
    documents.find((document) => document.id === selectedDocumentId) ?? null;
  const content = history.present;
  const isEditorReady = Boolean(
    selectedDocumentId && loadedDocumentId === selectedDocumentId,
  );
  const activeLayer = getActiveLayer(content, activeLayerId);
  const frame = content.frame;

  useEffect(() => {
    setSelectedObjectIds((current) => {
      const reconciled = reconcileSelectedObjectIds(content, current);
      const isUnchanged =
        reconciled.length === current.length &&
        reconciled.every((objectId, index) => objectId === current[index]);
      return isUnchanged ? current : reconciled;
    });
  }, [content]);

  const saveStatusLabel = useMemo(() => {
    if (isSaving) {
      return t("canvas.saveStatus.saving");
    }
    if (isDirty) {
      return t("canvas.saveStatus.dirty");
    }
    return t("canvas.saveStatus.saved");
  }, [isDirty, isSaving, t]);

  function clearSaveTimer() {
    if (saveTimerRef.current !== null) {
      window.clearTimeout(saveTimerRef.current);
      saveTimerRef.current = null;
    }
  }

  function setDirtyState(nextIsDirty: boolean) {
    isDirtyRef.current = nextIsDirty;
    setIsDirty(nextIsDirty);
  }

  function isDocumentReady(documentId: string | null): documentId is string {
    return Boolean(
      documentId &&
        selectedDocumentIdRef.current === documentId &&
        loadedDocumentIdRef.current === documentId,
    );
  }

  function handleSelectDocument(documentId: string) {
    if (selectedDocumentIdRef.current === documentId) {
      return;
    }

    const previousDocumentId = loadedDocumentIdRef.current;
    const previousSnapshot = historyPresentRef.current;
    clearSaveTimer();
    if (
      previousDocumentId &&
      previousDocumentId === selectedDocumentIdRef.current &&
      isDirtyRef.current
    ) {
      void persistDocument(previousDocumentId, previousSnapshot);
    }

    selectedDocumentIdRef.current = documentId;
    loadedDocumentIdRef.current = null;
    setLoadedDocumentId(null);
    setActiveLayerId(null);
    setSelectedObjectIds([]);
    setDirtyState(false);
    setIsSaving(false);
    setSelectedDocumentId(documentId);
  }

  function updateContent(
    nextContent: CanvasDocumentContent,
    options?: { replace?: boolean },
    documentId = selectedDocumentId,
  ): boolean {
    if (!isDocumentReady(documentId)) {
      return false;
    }

    historyPresentRef.current = nextContent;
    setHistory((current) =>
      options?.replace ? replaceHistory(current, nextContent) : pushHistory(current, nextContent),
    );
    setDirtyState(true);
    return true;
  }

  function updateSelection(candidateIds: string[]) {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    setSelectedObjectIds(reconcileSelectedObjectIds(content, candidateIds));
  }

  function handleDeleteSelection() {
    if (!isDocumentReady(selectedDocumentId) || !selectedObjectIds.length) {
      return;
    }

    if (updateContent(removeCanvasObjects(content, selectedObjectIds))) {
      setSelectedObjectIds([]);
    }
  }

  function handleCopySelection() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    setClipboard(copyCanvasObjects(content, selectedObjectIds));
  }

  function handlePasteSelection() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }

    const result = pasteCanvasObjects(content, clipboard, activeLayer?.id ?? null);
    if (!result.pastedObjectIds.length) {
      return;
    }

    if (updateContent(result.content)) {
      setSelectedObjectIds(result.pastedObjectIds);
    }
  }

  function handleMoveSelection(delta: { dx: number; dy: number }) {
    if (!isDocumentReady(selectedDocumentId) || !selectedObjectIds.length) {
      return;
    }

    updateContent(translateCanvasObjects(content, selectedObjectIds, delta));
  }

  function handleReorderSelection(direction: CanvasOrderDirection) {
    if (!isDocumentReady(selectedDocumentId) || !selectedObjectIds.length) {
      return;
    }

    updateContent(reorderCanvasObjects(content, selectedObjectIds, direction));
  }

  function handleFitFrame() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    handleViewportChange(fitViewportToCanvasRect(content.frame, stageSize));
  }

  function handleFitSelection() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    const bounds = getCombinedCanvasBounds(content, selectedObjectIds) ?? content.frame;
    handleViewportChange(fitViewportToCanvasRect(bounds, stageSize));
  }

  function persistDocument(
    documentId: string,
    snapshot: CanvasDocumentContent,
  ): Promise<void> {
    const activeSave = activeSaveRef.current;
    if (
      activeSave?.documentId === documentId &&
      activeSave.snapshot === snapshot
    ) {
      return activeSave.promise;
    }

    const saveToken = ++saveOperationTokenRef.current;
    const isCurrentSnapshot = () =>
      saveOperationTokenRef.current === saveToken &&
      selectedDocumentIdRef.current === documentId &&
      loadedDocumentIdRef.current === documentId &&
      historyPresentRef.current === snapshot;

    if (isCurrentSnapshot()) {
      setIsSaving(true);
    }

    const promise = (async () => {
      try {
        const previewPngBase64 = await exportCanvasFrame(snapshot);
        await saveCanvasDocument(documentId, snapshot, previewPngBase64);
        await refetch();
        if (isCurrentSnapshot()) {
          setDirtyState(false);
        }
      } finally {
        if (activeSaveRef.current?.token === saveToken) {
          activeSaveRef.current = null;
        }
        if (isCurrentSnapshot()) {
          setIsSaving(false);
        }
      }
    })();

    activeSaveRef.current = {
      documentId,
      snapshot,
      token: saveToken,
      promise,
    };
    return promise;
  }

  async function handleCreateDocument() {
    const created = await createCanvasDocument(activeProjectId, null);
    await refetch();
    handleSelectDocument(created.id);
  }

  async function handleGenerate() {
    if (
      !selectedDocument ||
      !isDocumentReady(selectedDocument.id) ||
      !prompt.trim() ||
      isGenerating ||
      !isDesktopRuntime
    ) return;

    setIsGenerating(true);
    try {
      const pngBase64 = await exportCanvasFrame(content);
      const exportedPath = await saveCanvasExport(selectedDocument.id, pngBase64);

      await editImage({
        prompt: prompt.trim(),
        model: imageModel,
        sourceImagePaths: [exportedPath],
        projectId: activeProjectId,
      });
    } finally {
      setIsGenerating(false);
    }
  }

  function handleViewportChange(viewport: CanvasDocumentContent["viewport"]) {
    updateContent({
      ...cloneCanvasDocumentContent(content),
      viewport,
    }, { replace: true });
  }

  function handleAddStroke(stroke: CanvasStrokeObject) {
    if (!activeLayer) {
      return;
    }

    updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: content.layers.map((layer) =>
        layer.id === activeLayer.id
          ? { ...layer, objects: [...layer.objects, stroke] }
          : layer,
      ),
    });
  }

  function handleTransformImage(
    objectId: string,
    transform: { x: number; y: number; width: number; height: number; rotation?: number },
  ) {
    updateContent(updateImageObject(content, objectId, transform), { replace: true });
  }

  function handleResetImageAspect(objectId: string) {
    updateContent(resetImageObjectAspect(content, objectId), { replace: true });
  }

  async function handleImportImage() {
    const documentId = selectedDocumentId;
    if (!isDocumentReady(documentId) || !activeLayer) {
      return;
    }

    const paths = await pickSourceImages();
    if (!paths.length) {
      return;
    }

    let nextContent = cloneCanvasDocumentContent(content);
    for (const path of paths) {
      const size = await readImageSize(path);
      const targetWidth = Math.min(960, size.width);
      const targetHeight = Math.round((size.height / size.width) * targetWidth);
      const imageObject = createImageObject({
        image_path: path,
        x: frame.x + (frame.width - targetWidth) / 2,
        y: frame.y + (frame.height - targetHeight) / 2,
        width: targetWidth,
        height: targetHeight,
        original_width: size.width,
        original_height: size.height,
      });

      nextContent = {
        ...nextContent,
        layers: nextContent.layers.map((layer) =>
          layer.id === activeLayer.id
            ? { ...layer, objects: [...layer.objects, imageObject] }
            : layer,
        ),
      };
    }

    if (!isDocumentReady(documentId) || !updateContent(nextContent, undefined, documentId)) {
      return;
    }
    setActiveTool("select");
    await persistDocument(documentId, nextContent);
  }

  function handleAddLayer() {
    const layerNumber = content.layers.length + 1;
    const layer = createCanvasLayer({
      name: t("canvas.defaultLayerName", { number: layerNumber }),
    });
    const didUpdate = updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: [layer, ...content.layers],
    });
    if (didUpdate) {
      setActiveLayerId(layer.id);
    }
  }

  function handleToggleLayerVisibility(layerId: string) {
    updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: toggleLayer(content.layers, layerId, "visible"),
    }, { replace: true });
  }

  function handleToggleLayerLock(layerId: string) {
    updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: toggleLayer(content.layers, layerId, "locked"),
    }, { replace: true });
  }

  function handleUndo() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    setHistory((current) => {
      const nextHistory = undoHistory(current);
      historyPresentRef.current = nextHistory.present;
      return nextHistory;
    });
    setDirtyState(true);
  }

  function handleRedo() {
    if (!isDocumentReady(selectedDocumentId)) {
      return;
    }
    setHistory((current) => {
      const nextHistory = redoHistory(current);
      historyPresentRef.current = nextHistory.present;
      return nextHistory;
    });
    setDirtyState(true);
  }

  function handleZoom(direction: "in" | "out") {
    const factor = direction === "in" ? 1.12 : 0.9;
    handleViewportChange({
      ...content.viewport,
      scale: clampZoom(content.viewport.scale * factor),
    });
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      const key = event.key.toLowerCase();
      const hasCommandModifier = event.metaKey || event.ctrlKey;
      const canEdit = isDocumentReady(selectedDocumentId);

      if (hasCommandModifier) {
        if (key === "z") {
          event.preventDefault();
          if (!canEdit) return;
          if (event.shiftKey) {
            if (history.future.length) handleRedo();
          } else if (history.past.length) {
            handleUndo();
          }
          return;
        }

        if (key === "y") {
          event.preventDefault();
          if (!canEdit) return;
          if (history.future.length) handleRedo();
          return;
        }

        if (key === "c") {
          event.preventDefault();
          if (!canEdit) return;
          handleCopySelection();
          return;
        }

        if (key === "v") {
          event.preventDefault();
          if (!canEdit) return;
          handlePasteSelection();
          return;
        }

        return;
      }

      if (key === "delete" || key === "backspace") {
        event.preventDefault();
        if (!canEdit) return;
        handleDeleteSelection();
        return;
      }

      if (key === "escape") {
        event.preventDefault();
        setSelectedObjectIds([]);
        return;
      }

      const tool =
        key === "v"
          ? "select"
          : key === "b"
            ? "brush"
            : key === "e"
              ? "eraser"
              : key === "h"
                ? "pan"
                : null;
      if (tool) {
        event.preventDefault();
        setActiveTool(tool);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    activeLayer?.id,
    clipboard,
    content,
    history.future.length,
    history.past.length,
    loadedDocumentId,
    selectedDocumentId,
    selectedObjectIds,
  ]);

  return (
    <div className="h-full min-h-0 overflow-x-auto bg-background">
      <div
        aria-label={t("canvas.workspaceLabel")}
        className="grid h-full min-w-[836px] min-h-0 grid-cols-[220px_minmax(276px,1fr)_300px] gap-0 bg-[linear-gradient(180deg,_rgba(255,255,255,0.72),_rgba(248,247,244,0.94))]"
      >
        <CanvasAssetSidebar
          documents={documents}
          selectedDocumentId={selectedDocumentId}
          onSelectDocument={handleSelectDocument}
          onCreateDocument={() => void handleCreateDocument()}
        />

        <section className="min-h-0 bg-background">
          {selectedDocument ? (
            isEditorReady ? (
              <div className="relative h-full min-h-0 overflow-hidden border-x border-border-subtle bg-canvas">
                <CanvasStage
                  content={content}
                  activeLayerId={activeLayerId}
                  activeTool={activeTool}
                  selectedObjectIds={selectedObjectIds}
                  strokeColor={strokeColor}
                  strokeSize={strokeSize}
                  onViewportChange={handleViewportChange}
                  onAddStroke={handleAddStroke}
                  onTransformImage={handleTransformImage}
                  onResetImageAspect={handleResetImageAspect}
                  onSelectionChange={updateSelection}
                  onMoveSelection={handleMoveSelection}
                  onStageSizeChange={setStageSize}
                  onExport={() => exportCanvasFrame(content)}
                />
                <div className="absolute right-5 top-5 z-10 rounded-[10px] border border-border-subtle bg-surface/88 px-3 py-2 text-[12px] font-medium text-muted shadow-card backdrop-blur-xl">
                  {saveStatusLabel}
                </div>
                <div
                  data-testid="canvas-floating-toolbar"
                  className="pointer-events-none absolute inset-x-0 bottom-5 z-20 flex justify-center px-5"
                >
                  <CanvasToolbar
                    activeTool={activeTool}
                    strokeColor={strokeColor}
                    strokeSize={strokeSize}
                    canUndo={history.past.length > 0}
                    canRedo={history.future.length > 0}
                    selectedObjectCount={selectedObjectIds.length}
                    zoomPercent={Math.round(content.viewport.scale * 100)}
                    canPaste={Boolean(clipboard?.entries.length)}
                    onToolChange={setActiveTool}
                    onColorChange={setStrokeColor}
                    onSizeChange={setStrokeSize}
                    onUndo={handleUndo}
                    onRedo={handleRedo}
                    onZoomIn={() => handleZoom("in")}
                    onZoomOut={() => handleZoom("out")}
                    onImportImage={() => void handleImportImage()}
                    onDeleteSelection={handleDeleteSelection}
                    onCopySelection={handleCopySelection}
                    onPasteSelection={handlePasteSelection}
                    onReorderSelection={handleReorderSelection}
                    onFitFrame={handleFitFrame}
                    onFitSelection={handleFitSelection}
                  />
                </div>
              </div>
            ) : (
              <div className="flex h-full min-h-0 items-center justify-center border-x border-border-subtle px-6">
                <div role="status" className="text-[13px] font-medium text-muted">
                  {t("canvas.loading")}
                </div>
              </div>
            )
          ) : (
            <div className="flex h-full min-h-0 items-center justify-center border-x border-border-subtle px-6">
              <div
                data-testid="canvas-empty-state-card"
                className="w-[min(360px,calc(100%-48px))] max-w-full rounded-[18px] border border-border-subtle bg-surface/92 px-8 py-10 text-center shadow-float"
              >
                <h1 className="text-[22px] font-semibold text-foreground">
                  {t("canvas.title")}
                </h1>
                <p className="mt-3 text-[14px] leading-6 text-muted">
                  {t("canvas.noDocumentSelected")}
                </p>
              </div>
            </div>
          )}
        </section>

        <aside
          aria-label={t("canvas.inspectorLabel")}
          className="min-h-0 overflow-y-auto bg-surface-muted/72 px-4 py-4"
        >
          <div className="flex min-h-full flex-col gap-4">
            <CanvasGenerationPanel
              prompt={prompt}
              imageModel={imageModel}
              frame={frame}
              disabled={
                !isEditorReady || !prompt.trim() || isGenerating || !isDesktopRuntime
              }
              isGenerating={isGenerating}
              environmentHint={
                isDesktopRuntime ? null : t("canvas.generationUnavailableBrowser")
              }
              onPromptChange={setPrompt}
              onGenerate={() => void handleGenerate()}
            />

            {selectedDocument && isEditorReady ? (
              <CanvasLayersPanel
                layers={content.layers}
                activeLayerId={activeLayerId}
                onSelectLayer={setActiveLayerId}
                onAddLayer={handleAddLayer}
                onToggleLayerVisibility={handleToggleLayerVisibility}
                onToggleLayerLock={handleToggleLayerLock}
              />
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}

function toggleLayer(
  layers: CanvasLayer[],
  layerId: string,
  field: "visible" | "locked",
) {
  return layers.map((layer) =>
    layer.id === layerId ? { ...layer, [field]: !layer[field] } : layer,
  );
}

export function getFrameSummary(frame: CanvasFrame) {
  return `${Math.round(frame.width)} x ${Math.round(frame.height)}`;
}
