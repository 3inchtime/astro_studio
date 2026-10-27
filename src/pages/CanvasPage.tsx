import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  createCanvasDocument,
  editImage,
  getImageModel,
  getCanvasDocument,
  getCanvasGenerationStatus,
  hasTauriRuntime,
  pickSourceImages,
  saveCanvasDocument,
  saveCanvasExport,
} from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import CanvasAssetSidebar from "../components/canvas/CanvasAssetSidebar";
import CanvasGenerationPanel from "../components/canvas/CanvasGenerationPanel";
import CanvasPropertiesPanel from "../components/canvas/CanvasPropertiesPanel";
import {
  completeCanvasGeneration,
  failCanvasGeneration,
} from "../lib/canvas/generation";
import { queueCanvasSave, waitForCanvasSaves } from "../lib/canvas/persistence";
import { useCanvasGenerationRecovery } from "../lib/canvas/useCanvasGenerationRecovery";
import {
  alignCanvasObjects,
  distributeCanvasObjects,
  scaleCanvasObjects,
  updateCanvasObjectStyle,
  type CanvasObjectStyle,
} from "../lib/canvas/editor";
import type { CanvasRect } from "../lib/canvas/bounds";
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
import {
  clampZoom,
  fitViewportToCanvasRect,
  resizeFrameByAspect,
} from "../lib/canvas/frame";
import {
  createHistory,
  pushHistory,
  redoHistory,
  replaceHistory,
  undoHistory,
} from "../lib/canvas/history";
import { reorderCanvasObjects } from "../lib/canvas/ordering";
import type { CanvasOrderDirection } from "../lib/canvas/ordering";
import {
  getSelectableCanvasObjects,
  reconcileSelectedObjectIds,
} from "../lib/canvas/selection";
import { translateCanvasObjects } from "../lib/canvas/transforms";
import { useCanvasDocumentsQuery } from "../lib/queries/canvasDocuments";
import type {
  CanvasDocumentContent,
  CanvasFrame,
  CanvasLayer,
  CanvasStrokeObject,
  CanvasObject,
  CanvasTool,
  ImageModel,
} from "../types";

export default function CanvasPage() {
  const { t } = useTranslation();
  const { activeProjectId } = useLayoutContext();
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(
    null,
  );
  const [prompt, setPrompt] = useState("");
  const [imageModel, setImageModel] = useState<ImageModel>("gpt-image-2");
  const [submittingDocumentId, setSubmittingDocumentId] = useState<
    string | null
  >(null);
  const liveRequestsRef = useRef(new Set<string>());
  const mountedRef = useRef(true);
  const [importError, setImportError] = useState<string | null>(null);
  const [fillColor, setFillColor] = useState("#c7d2fe");
  const [objectOpacity, setObjectOpacity] = useState(1);
  const [fontSize, setFontSize] = useState(32);
  const [frameEditing, setFrameEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isDirty, setIsDirty] = useState(false);
  const [hasSaveError, setHasSaveError] = useState(false);
  const [hasLoadError, setHasLoadError] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [isTransitioning, setIsTransitioning] = useState(false);
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
  const retryLoadDocumentIdRef = useRef<string | null>(null);
  const saveOperationTokenRef = useRef(0);
  const selectedDocumentIdRef = useRef<string | null>(selectedDocumentId);
  const loadedDocumentIdRef = useRef<string | null>(loadedDocumentId);
  const historyPresentRef = useRef(history.present);
  const isDirtyRef = useRef(isDirty);
  const isTransitioningRef = useRef(isTransitioning);
  const saveQueueRef = useRef<Promise<void>>(Promise.resolve());
  const activeSaveRef = useRef<{
    documentId: string;
    snapshot: CanvasDocumentContent;
    token: number;
    promise: Promise<void>;
  } | null>(null);
  const activeTransitionRef = useRef<{
    targetDocumentId: string | null;
    promise: Promise<boolean>;
  } | null>(null);
  const { data: documents = [], refetch } =
    useCanvasDocumentsQuery(activeProjectId);

  selectedDocumentIdRef.current = selectedDocumentId;
  loadedDocumentIdRef.current = loadedDocumentId;
  historyPresentRef.current = history.present;
  isDirtyRef.current = isDirty;
  isTransitioningRef.current = isTransitioning;

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    getImageModel()
      .then((model) => setImageModel(model))
      .catch(() => {});
  }, []);

  useEffect(() => {
    const currentDocumentId = selectedDocumentIdRef.current;
    const nextDocumentId =
      currentDocumentId &&
      documents.some((document) => document.id === currentDocumentId)
        ? currentDocumentId
        : (documents[0]?.id ?? null);

    if (nextDocumentId !== currentDocumentId) {
      void transitionToDocument(nextDocumentId).catch(() => {});
    }
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
    setHasSaveError(false);
    setHasLoadError(false);

    if (!selectedDocumentId) {
      return;
    }

    const requestedDocumentId = selectedDocumentId;
    const retryFailedSave =
      retryLoadDocumentIdRef.current === requestedDocumentId;
    retryLoadDocumentIdRef.current = null;
    let disposed = false;
    const isCurrentLoad = () =>
      !disposed &&
      mountedRef.current &&
      loadRequestTokenRef.current === requestToken &&
      selectedDocumentIdRef.current === requestedDocumentId;
    void (async () => {
      await waitForCanvasSaves(
        requestedDocumentId,
        retryFailedSave,
        isCurrentLoad,
      );
      if (!isCurrentLoad()) return;
      const document = await getCanvasDocument(requestedDocumentId);
      if (!isCurrentLoad()) return;
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
      setHasSaveError(false);
      setHasLoadError(false);
    })().catch(() => {
      if (isCurrentLoad()) setHasLoadError(true);
    });
    return () => {
      disposed = true;
    };
  }, [loadAttempt, selectedDocumentId]);

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
        void persistDocument(documentId, snapshot).catch(() => {});
      }
    }, 500);

    return clearSaveTimer;
  }, [history.present, isDirty, loadedDocumentId, selectedDocumentId]);

  const content = history.present;
  const isEditorReady = Boolean(
    selectedDocumentId &&
      loadedDocumentId === selectedDocumentId &&
      !isTransitioning,
  );
  const isGenerating =
    Boolean(
      submittingDocumentId && submittingDocumentId === loadedDocumentId,
    ) ||
    Boolean(content.generation_requests?.some((r) => r.status === "pending"));
  const latestRequest =
    content.generation_requests?.[content.generation_requests.length - 1];
  const activeLayer = getActiveLayer(content, activeLayerId);
  const frame = content.frame;
  useCanvasGenerationRecovery({
    documentId: loadedDocumentId,
    enabled: isEditorReady && isDesktopRuntime,
    getContent: () => historyPresentRef.current,
    isCurrent: isDocumentReady,
    liveRequests: liveRequestsRef.current,
    apply: applyGenerationContent,
  });

  function applyGenerationContent(
    documentId: string,
    next: CanvasDocumentContent,
  ) {
    if (!mountedRef.current || !isDocumentReady(documentId)) return;
    if (updateContent(next, undefined, documentId))
      void persistDocument(documentId, next).catch(() => {});
  }

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
    if (hasSaveError) {
      return t("canvas.saveStatus.error");
    }
    if (isSaving) {
      return t("canvas.saveStatus.saving");
    }
    if (isDirty) {
      return t("canvas.saveStatus.dirty");
    }
    return t("canvas.saveStatus.saved");
  }, [hasSaveError, isDirty, isSaving, t]);

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

  function setTransitionState(nextIsTransitioning: boolean) {
    isTransitioningRef.current = nextIsTransitioning;
    setIsTransitioning(nextIsTransitioning);
  }

  function isDocumentReady(documentId: string | null): documentId is string {
    return Boolean(
      documentId &&
        selectedDocumentIdRef.current === documentId &&
        loadedDocumentIdRef.current === documentId &&
        !isTransitioningRef.current,
    );
  }

  function transitionToDocument(
    targetDocumentId: string | null,
  ): Promise<boolean> {
    const activeTransition = activeTransitionRef.current;
    if (activeTransition) {
      if (activeTransition.targetDocumentId === targetDocumentId) {
        return activeTransition.promise;
      }

      return activeTransition.promise.then(
        (didTransition) =>
          didTransition ? transitionToDocument(targetDocumentId) : false,
        () => false,
      );
    }

    if (selectedDocumentIdRef.current === targetDocumentId) {
      return Promise.resolve(true);
    }

    const promise = performDocumentTransition(targetDocumentId);
    activeTransitionRef.current = { targetDocumentId, promise };
    void promise.then(
      () => {
        if (activeTransitionRef.current?.promise === promise) {
          activeTransitionRef.current = null;
        }
      },
      () => {
        if (activeTransitionRef.current?.promise === promise) {
          activeTransitionRef.current = null;
        }
      },
    );
    return promise;
  }

  async function performDocumentTransition(
    targetDocumentId: string | null,
  ): Promise<boolean> {
    const previousSelectedDocumentId = selectedDocumentIdRef.current;
    const previousLoadedDocumentId = loadedDocumentIdRef.current;
    const previousSnapshot = historyPresentRef.current;
    clearSaveTimer();
    setTransitionState(true);
    setSelectedObjectIds([]);

    if (
      previousLoadedDocumentId &&
      previousLoadedDocumentId === previousSelectedDocumentId &&
      isDirtyRef.current
    ) {
      try {
        await persistDocument(previousLoadedDocumentId, previousSnapshot);
      } catch {
        setTransitionState(false);
        return false;
      }
    }

    selectedDocumentIdRef.current = targetDocumentId;
    loadedDocumentIdRef.current = null;
    setLoadedDocumentId(null);
    setActiveLayerId(null);
    setDirtyState(false);
    setIsSaving(false);
    setHasSaveError(false);
    setHasLoadError(false);
    setSelectedDocumentId(targetDocumentId);
    setTransitionState(false);
    return true;
  }

  function handleSelectDocument(documentId: string) {
    void transitionToDocument(documentId).catch(() => {});
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
      options?.replace
        ? replaceHistory(current, nextContent)
        : pushHistory(current, nextContent),
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

    const result = pasteCanvasObjects(
      content,
      clipboard,
      activeLayer?.id ?? null,
    );
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
    const bounds =
      getCombinedCanvasBounds(content, selectedObjectIds) ?? content.frame;
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
      mountedRef.current &&
      saveOperationTokenRef.current === saveToken &&
      selectedDocumentIdRef.current === documentId &&
      loadedDocumentIdRef.current === documentId &&
      historyPresentRef.current === snapshot;

    if (isCurrentSnapshot()) {
      setIsSaving(true);
      setHasSaveError(false);
    }

    const queuedSave = queueCanvasSave(
      documentId,
      async () => {
        const previewPngBase64 = await exportCanvasFrame(snapshot);
        await saveCanvasDocument(documentId, snapshot, previewPngBase64);
      },
      saveQueueRef.current,
    ).promise;
    const promise = (async () => {
      try {
        await queuedSave;
        if (mountedRef.current) await refetch();
        if (isCurrentSnapshot()) {
          setDirtyState(false);
          setHasSaveError(false);
        }
      } catch (error) {
        if (isCurrentSnapshot()) {
          setDirtyState(true);
          setHasSaveError(true);
        }
        throw error;
      } finally {
        if (activeSaveRef.current?.token === saveToken) {
          activeSaveRef.current = null;
        }
        if (isCurrentSnapshot()) {
          setIsSaving(false);
        }
      }
    })();

    saveQueueRef.current = promise.catch(() => {});
    activeSaveRef.current = {
      documentId,
      snapshot,
      token: saveToken,
      promise,
    };
    return promise;
  }

  function handleRetrySave() {
    const documentId = loadedDocumentIdRef.current;
    const snapshot = historyPresentRef.current;
    if (
      !documentId ||
      selectedDocumentIdRef.current !== documentId ||
      !isDirtyRef.current
    ) {
      return;
    }

    void persistDocument(documentId, snapshot).catch(() => {});
  }

  function handleRetryLoad() {
    if (!selectedDocumentIdRef.current) {
      return;
    }

    retryLoadDocumentIdRef.current = selectedDocumentIdRef.current;
    setHasLoadError(false);
    setLoadAttempt((current) => current + 1);
  }

  async function handleCreateDocument() {
    const created = await createCanvasDocument(activeProjectId, null);
    await refetch();
    await transitionToDocument(created.id);
  }

  async function handleGenerate(retryPrompt?: string) {
    const documentId = selectedDocumentId;
    const submittedPrompt = (retryPrompt ?? prompt).trim();
    if (
      !isDocumentReady(documentId) ||
      !submittedPrompt ||
      isGenerating ||
      !isDesktopRuntime
    )
      return;
    const snapshot = cloneCanvasDocumentContent(historyPresentRef.current);
    const requestId = crypto.randomUUID();
    const projectId =
      documents.find((d) => d.id === documentId)?.project_id ?? activeProjectId;
    const request = {
      request_id: requestId,
      project_id: projectId,
      frame: { ...snapshot.frame },
      prompt: submittedPrompt,
      status: "pending" as const,
      image_ids: [],
      created_at: new Date().toISOString(),
    };
    liveRequestsRef.current.add(requestId);
    setSubmittingDocumentId(documentId);
    const pending = {
      ...historyPresentRef.current,
      generation_requests: [
        ...(historyPresentRef.current.generation_requests ?? []),
        request,
      ],
    };
    updateContent(pending, { replace: true }, documentId);
    let submissionStarted = false;
    try {
      await persistDocument(documentId, pending);
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      const pngBase64 = await exportCanvasFrame(snapshot);
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      const exportedPath = await saveCanvasExport(documentId, pngBase64);
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      submissionStarted = true;
      const result = await editImage({
        prompt: submittedPrompt,
        clientRequestId: requestId,
        model: imageModel,
        sourceImagePaths: [exportedPath],
        projectId,
      });
      if (mountedRef.current && isDocumentReady(documentId))
        applyGenerationContent(
          documentId,
          completeCanvasGeneration(
            historyPresentRef.current,
            requestId,
            result.generation_id,
            result.images,
          ),
        );
    } catch (error) {
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      if (!submissionStarted) {
        applyGenerationContent(
          documentId,
          failCanvasGeneration(
            historyPresentRef.current,
            requestId,
            String(error),
          ),
        );
      } else {
        // A rejected IPC reply does not prove the durable job failed. Preserve
        // the request identity until its status lookup confirms a terminal state.
        let status = null;
        try {
          status = await getCanvasGenerationStatus(documentId, requestId);
        } catch {
          /* recovery will retry the lookup */
        }
        if (!mountedRef.current || !isDocumentReady(documentId)) return;
        const current = historyPresentRef.current;
        if (status?.status === "completed" || status?.status === "succeeded") {
          applyGenerationContent(
            documentId,
            completeCanvasGeneration(
              current,
              requestId,
              status.generation_id ?? "",
              status.images,
            ),
          );
        } else if (
          status &&
          ["failed", "cancelled", "interrupted"].includes(status.status)
        ) {
          applyGenerationContent(
            documentId,
            failCanvasGeneration(
              current,
              requestId,
              status.error ?? String(error),
            ),
          );
        } else if (
          current.generation_requests?.some(
            (r) => r.request_id === requestId && r.status === "pending",
          )
        ) {
          applyGenerationContent(documentId, {
            ...current,
            generation_requests: current.generation_requests.map((r) =>
              r.request_id === requestId
                ? {
                    ...r,
                    error: String(error),
                    ...(status?.generation_id
                      ? { generation_id: status.generation_id }
                      : {}),
                  }
                : r,
            ),
          });
        }
      }
    } finally {
      liveRequestsRef.current.delete(requestId);
      if (mountedRef.current)
        setSubmittingDocumentId((current) =>
          current === documentId ? null : current,
        );
    }
  }

  function handleViewportChange(viewport: CanvasDocumentContent["viewport"]) {
    updateContent(
      {
        ...cloneCanvasDocumentContent(content),
        viewport,
      },
      { replace: true },
    );
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
    transform: {
      x: number;
      y: number;
      width: number;
      height: number;
      rotation?: number;
    },
  ) {
    updateContent(updateImageObject(content, objectId, transform));
  }

  function handleResetImageAspect(objectId: string) {
    updateContent(resetImageObjectAspect(content, objectId));
  }

  async function handleImportImage() {
    const documentId = selectedDocumentId;
    const layerId = activeLayer?.id;
    const token = loadRequestTokenRef.current;
    if (!isDocumentReady(documentId) || !layerId) return;
    setImportError(null);
    try {
      const paths = await pickSourceImages();
      const images = await Promise.all(
        paths.map(async (path) => ({ path, size: await readImageSize(path) })),
      );
      if (
        !mountedRef.current ||
        !isDocumentReady(documentId) ||
        token !== loadRequestTokenRef.current ||
        !images.length
      )
        return;
      const latest = historyPresentRef.current;
      const layer = latest.layers.find((l) => l.id === layerId);
      if (!layer || !layer.visible || layer.locked) return;
      let x = latest.frame.x;
      const objects = images.map(({ path, size }) => {
        const width = Math.min(960, size.width),
          height = (size.height / size.width) * width;
        const object = createImageObject({
          image_path: path,
          x,
          y: latest.frame.y,
          width,
          height,
          original_width: size.width,
          original_height: size.height,
        });
        x += width + 32;
        return object;
      });
      const next = {
        ...latest,
        layers: latest.layers.map((l) =>
          l.id === layerId ? { ...l, objects: [...l.objects, ...objects] } : l,
        ),
      };
      if (updateContent(next, undefined, documentId)) {
        setActiveTool("select");
        await persistDocument(documentId, next);
      }
    } catch {
      if (
        mountedRef.current &&
        isDocumentReady(documentId) &&
        token === loadRequestTokenRef.current
      )
        setImportError(t("canvas.importError"));
    }
  }

  function handleAddObject(object: CanvasObject) {
    const latest = historyPresentRef.current;
    const layer = getActiveLayer(latest, activeLayerId);
    if (!layer || layer.locked || !layer.visible) return;
    updateContent({
      ...latest,
      layers: latest.layers.map((l) =>
        l.id === layer.id ? { ...l, objects: [...l.objects, object] } : l,
      ),
    });
    setSelectedObjectIds([object.id]);
  }
  function handleUpdateText(
    id: string,
    text: string,
    width: number,
    height: number,
  ) {
    updateContent({
      ...content,
      layers: content.layers.map((l) =>
        l.locked || !l.visible
          ? l
          : {
              ...l,
              objects: l.objects.map((o) =>
                o.id === id && o.type === "text"
                  ? { ...o, text, width, height }
                  : o,
              ),
            },
      ),
    });
  }
  function handleStyle(style: CanvasObjectStyle) {
    if (selectedObjectIds.length)
      updateContent(updateCanvasObjectStyle(content, selectedObjectIds, style));
  }
  function handleSelectAll() {
    updateSelection(
      getSelectableCanvasObjects(content).map((e) => e.object.id),
    );
  }
  function handleDuplicateSelection() {
    const result = pasteCanvasObjects(
      content,
      copyCanvasObjects(content, selectedObjectIds),
      activeLayer?.id ?? null,
    );
    if (result.pastedObjectIds.length && updateContent(result.content))
      setSelectedObjectIds(result.pastedObjectIds);
  }
  function handleFrameFromSelection() {
    const bounds = getCombinedCanvasBounds(content, selectedObjectIds);
    if (bounds) handleFrameChange({ ...bounds, aspect: "custom" });
  }
  function handleFrameChange(nextFrame: CanvasFrame) {
    updateContent({ ...content, frame: nextFrame });
  }
  function handleTransformSelection(rect: CanvasRect) {
    updateContent(scaleCanvasObjects(content, selectedObjectIds, rect));
  }
  function handleFitAll() {
    handleViewportChange(
      fitViewportToCanvasRect(
        getCombinedCanvasBounds(
          content,
          content.layers
            .filter((l) => l.visible)
            .flatMap((l) => l.objects.map((o) => o.id)),
        ) ?? content.frame,
        stageSize,
      ),
    );
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
    });
  }

  function handleToggleLayerLock(layerId: string) {
    updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: toggleLayer(content.layers, layerId, "locked"),
    });
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

        if (key === "a" || key === "d") {
          event.preventDefault();
          if (!canEdit) return;
          if (key === "a") handleSelectAll();
          else handleDuplicateSelection();
          return;
        }
        if (key === "0") {
          event.preventDefault();
          if (canEdit) handleViewportChange({ x: 0, y: 0, scale: 1 });
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

      if (!canEdit) {
        return;
      }

      if (key === "escape") {
        event.preventDefault();
        setSelectedObjectIds([]);
        return;
      }

      if (["arrowleft", "arrowright", "arrowup", "arrowdown"].includes(key)) {
        event.preventDefault();
        const step = event.shiftKey ? 10 : 1;
        handleMoveSelection({
          dx: key === "arrowleft" ? -step : key === "arrowright" ? step : 0,
          dy: key === "arrowup" ? -step : key === "arrowdown" ? step : 0,
        });
        return;
      }
      if (key === "f") {
        event.preventDefault();
        if (event.shiftKey) handleFitSelection();
        else handleFitAll();
        return;
      }
      const shapeTools: Record<string, CanvasTool> = {
        r: "rectangle",
        o: "ellipse",
        l: "line",
        a: "arrow",
        t: "text",
      };
      if (shapeTools[key]) {
        event.preventDefault();
        setFrameEditing(false);
        setActiveTool(shapeTools[key]);
        return;
      }
      const tool =
        key === "v"
          ? "select"
          : key === "b" || key === "p"
            ? "brush"
            : key === "e"
              ? "eraser"
              : key === "h"
                ? "pan"
                : null;
      if (tool) {
        event.preventDefault();
        setFrameEditing(false);
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
          {selectedDocumentId ? (
            isEditorReady ? (
              <div className="relative h-full min-h-0 overflow-hidden border-x border-border-subtle bg-canvas">
                <CanvasStage
                  content={content}
                  activeLayerId={activeLayerId}
                  activeTool={activeTool}
                  selectedObjectIds={selectedObjectIds}
                  strokeColor={strokeColor}
                  strokeSize={strokeSize}
                  fillColor={fillColor}
                  objectOpacity={objectOpacity}
                  fontSize={fontSize}
                  frameEditing={frameEditing}
                  onFrameChange={handleFrameChange}
                  onAddObject={handleAddObject}
                  onUpdateText={handleUpdateText}
                  onTransformSelection={handleTransformSelection}
                  onCopySelection={handleCopySelection}
                  onPasteSelection={handlePasteSelection}
                  onDuplicateSelection={handleDuplicateSelection}
                  onDeleteSelection={handleDeleteSelection}
                  onReorderSelection={handleReorderSelection}
                  onFrameFromSelection={handleFrameFromSelection}
                  onSelectAll={handleSelectAll}
                  canPaste={Boolean(clipboard?.entries.length)}
                  onViewportChange={handleViewportChange}
                  onAddStroke={handleAddStroke}
                  onTransformImage={handleTransformImage}
                  onResetImageAspect={handleResetImageAspect}
                  onSelectionChange={updateSelection}
                  onMoveSelection={handleMoveSelection}
                  onStageSizeChange={setStageSize}
                  onExport={() => exportCanvasFrame(content)}
                />
                <div
                  role={hasSaveError ? "alert" : undefined}
                  className="absolute right-5 top-5 z-10 flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface/88 px-3 py-2 text-[12px] font-medium text-muted shadow-card backdrop-blur-xl"
                >
                  <span>{saveStatusLabel}</span>
                  {hasSaveError ? (
                    <button
                      type="button"
                      className="focus-ring cursor-pointer rounded-[7px] bg-primary/10 px-2 py-1 font-semibold text-primary hover:bg-primary/15"
                      onClick={handleRetrySave}
                    >
                      {t("canvas.retrySave")}
                    </button>
                  ) : null}
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
                    onToolChange={(tool) => {
                      setActiveTool(tool);
                      setFrameEditing(false);
                    }}
                    fillColor={fillColor}
                    objectOpacity={objectOpacity}
                    fontSize={fontSize}
                    frameEditing={frameEditing}
                    onFillColorChange={(value) => {
                      setFillColor(value);
                      handleStyle({ fill: value });
                    }}
                    onOpacityChange={(value) => {
                      setObjectOpacity(value);
                      handleStyle({ opacity: value });
                    }}
                    onFontSizeChange={(value) => {
                      setFontSize(value);
                      handleStyle({ fontSize: value });
                    }}
                    onFitAll={handleFitAll}
                    onResetZoom={() =>
                      handleViewportChange({ x: 0, y: 0, scale: 1 })
                    }
                    onSelectAll={handleSelectAll}
                    onDuplicateSelection={handleDuplicateSelection}
                    onFrameFromSelection={handleFrameFromSelection}
                    onEditFrame={() => setFrameEditing((value) => !value)}
                    onAlignSelection={(alignment) =>
                      updateContent(
                        alignCanvasObjects(
                          content,
                          selectedObjectIds,
                          alignment,
                        ),
                      )
                    }
                    onDistributeSelection={(axis) =>
                      updateContent(
                        distributeCanvasObjects(
                          content,
                          selectedObjectIds,
                          axis,
                        ),
                      )
                    }
                    onColorChange={(value) => {
                      setStrokeColor(value);
                      handleStyle({ color: value });
                    }}
                    onSizeChange={(value) => {
                      setStrokeSize(value);
                      handleStyle({ size: value });
                    }}
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
            ) : hasLoadError && !isTransitioning ? (
              <div className="flex h-full min-h-0 items-center justify-center border-x border-border-subtle px-6">
                <div
                  role="alert"
                  className="rounded-[14px] border border-border-subtle bg-surface/92 px-6 py-5 text-center shadow-card"
                >
                  <p className="text-[13px] font-medium text-foreground">
                    {t("canvas.loadError")}
                  </p>
                  <button
                    type="button"
                    className="studio-control-primary focus-ring mt-3 cursor-pointer rounded-[9px] px-3 py-2 text-[12px] font-semibold"
                    onClick={handleRetryLoad}
                  >
                    {t("canvas.retryLoad")}
                  </button>
                </div>
              </div>
            ) : (
              <div className="flex h-full min-h-0 items-center justify-center border-x border-border-subtle px-6">
                <div
                  role="status"
                  className="text-[13px] font-medium text-muted"
                >
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
                !isEditorReady ||
                !prompt.trim() ||
                isGenerating ||
                !isDesktopRuntime
              }
              isGenerating={isGenerating}
              environmentHint={
                isDesktopRuntime
                  ? null
                  : t("canvas.generationUnavailableBrowser")
              }
              error={
                latestRequest?.status === "completed"
                  ? null
                  : (latestRequest?.error ?? null)
              }
              results={
                latestRequest?.status === "completed"
                  ? content.layers
                      .flatMap((l) => l.objects)
                      .filter(
                        (o) =>
                          o.type === "image" &&
                          latestRequest.image_ids.some(
                            (id) =>
                              o.id ===
                              `canvas-result:${latestRequest.request_id}:${id}`,
                          ),
                      )
                      .map((o) => (o.type === "image" ? o.image_path : ""))
                  : []
              }
              onRetry={() => void handleGenerate(latestRequest?.prompt)}
              onFrameAspectChange={(aspect) =>
                handleFrameChange(resizeFrameByAspect(frame, aspect))
              }
              onFrameFromSelection={
                selectedObjectIds.length ? handleFrameFromSelection : undefined
              }
              onPromptChange={setPrompt}
              onGenerate={() => void handleGenerate()}
            />

            {importError ? (
              <p role="alert" className="text-sm text-red-600">
                {importError}
              </p>
            ) : null}
            {isEditorReady ? (
              <CanvasPropertiesPanel
                objects={getSelectableCanvasObjects(content)
                  .filter((e) => selectedObjectIds.includes(e.object.id))
                  .map((e) => e.object)}
                onChange={handleStyle}
              />
            ) : null}
            {selectedDocumentId && isEditorReady ? (
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
