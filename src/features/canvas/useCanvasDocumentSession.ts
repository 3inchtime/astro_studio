import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { createCanvasDocument, getCanvasDocument, saveCanvasDocument } from "../../lib/api";
import { queueCanvasSave, waitForCanvasSaves, registerCanvasDraftFlusher, acknowledgeCanvasSaveRecovery, getConflictedCanvasDraft } from "../../lib/canvas/persistence";
import { createCanvasDocumentContent, sanitizeCanvasDocumentContent } from "../../lib/canvas/document";
import { exportCanvasFrame } from "../../lib/canvas/export";
import { createHistory, pushHistory, redoHistory, replaceHistory, undoHistory } from "../../lib/canvas/history";
import { reconcileSelectedObjectIds } from "../../lib/canvas/selection";
import { useCanvasDocumentsQuery } from "../../lib/queries/canvasDocuments";
import type { CanvasDocumentContent, CanvasMissingAsset } from "../../types";

export function useCanvasDocumentSession(activeProjectId: string | null) {
  const { t } = useTranslation();
  const [selectedDocumentId, setSelectedDocumentId] = useState<string | null>(
    null,
  );

  const mountedRef = useRef(true);

  const [isSaving, setIsSaving] = useState(false);
  const [isDirty, setIsDirty] = useState(false);
  const [hasSaveConflict, setHasSaveConflict] = useState(false);
  const recoveryCopyRef = useRef<{ sourceId: string; copyId: string } | null>(null);
  const [hasSaveError, setHasSaveError] = useState(false);
  const [hasLoadError, setHasLoadError] = useState(false);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [isTransitioning, setIsTransitioning] = useState(false);

  const [activeLayerId, setActiveLayerId] = useState<string | null>(null);
  const [loadedDocumentId, setLoadedDocumentId] = useState<string | null>(null);
  const [selectedObjectIds, setSelectedObjectIds] = useState<string[]>([]);

  const [history, setHistory] = useState(() =>
    createHistory<CanvasDocumentContent>(createCanvasDocumentContent()),
  );
  const revisionsRef = useRef(new Map<string, number>());
  const missingAssetsRef = useRef<CanvasMissingAsset[]>([]);

  const [missingAssets, setMissingAssets] = useState<CanvasMissingAsset[]>([]);
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
    const unregister = registerCanvasDraftFlusher(async () => {
      clearSaveTimer();
      // Repeat if an edit arrived while preview rendering or IPC was pending.
      while (loadedDocumentIdRef.current && isDirtyRef.current) {
        const id = loadedDocumentIdRef.current;
        const snapshot = historyPresentRef.current;
        await persistDocument(id, snapshot);
        if (historyPresentRef.current === snapshot) break;
      }
    });
    return () => {
      unregister();
      mountedRef.current = false;
      clearSaveTimer();
      const documentId = loadedDocumentIdRef.current;
      if (documentId && isDirtyRef.current) {
        void persistDocument(documentId, historyPresentRef.current).catch(() => {});
      }
    };
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
    setHasSaveConflict(false);
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
      revisionsRef.current.set(requestedDocumentId, document.revision ?? 0);
      missingAssetsRef.current = document.missing_assets ?? [];
      setMissingAssets(document.missing_assets ?? []);
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
      if (!isCurrentLoad()) return;
      const recovered = getConflictedCanvasDraft(requestedDocumentId);
      if (!recovered) { setHasLoadError(true); return; }
      // A late conflict must remain editable after navigating away and back.
      const nextContent = sanitizeCanvasDocumentContent(recovered.content);
      revisionsRef.current.set(requestedDocumentId, recovered.revision);
      historyPresentRef.current = nextContent;
      loadedDocumentIdRef.current = requestedDocumentId;
      setHistory(createHistory(nextContent));
      setActiveLayerId(nextContent.layers[0]?.id ?? null);
      setLoadedDocumentId(requestedDocumentId);
      setDirtyState(true);
      setHasSaveError(true);
      setHasSaveConflict(true);
      setHasLoadError(false);
    });
    return () => {
      disposed = true;
    };
  }, [loadAttempt, selectedDocumentId]);

  useEffect(() => {
    if (
      !selectedDocumentId ||
      loadedDocumentId !== selectedDocumentId ||
      !isDirty || hasSaveConflict
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
  }, [history.present, isDirty, hasSaveConflict, loadedDocumentId, selectedDocumentId]);

  const content = history.present;

  const isEditorReady = Boolean(
    selectedDocumentId &&
      loadedDocumentId === selectedDocumentId &&
      !isTransitioning,
  );

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
    setHasSaveConflict(false);
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

    const capturedMissingAssets = missingAssetsRef.current;
    const queuedSave = queueCanvasSave(
      documentId,
      async () => {
        // A legacy missing image must not prevent saving the rest of the draft.
        const missing = capturedMissingAssets;
        const previewContent = missing.length ? {
          ...snapshot,
          layers: snapshot.layers.map((layer) => ({ ...layer, objects: layer.objects.filter((object) =>
            object.type !== "image" || !missing.some((asset) => asset.object_id === object.id && asset.image_path === object.image_path)
          ) })),
        } : snapshot;
        const previewPngBase64 = await exportCanvasFrame(previewContent).catch(() => null);
        const saved = await saveCanvasDocument(
          documentId, snapshot, previewPngBase64,
          revisionsRef.current.get(documentId) ?? 0,
        );
        revisionsRef.current.set(documentId, saved.revision);
      },
      saveQueueRef.current,
      snapshot,
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
          setHasSaveConflict(Boolean(error && typeof error === "object" && "kind" in error && error.kind === "CanvasRevisionConflict"));
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

  async function handleSaveCopy() {
    const sourceId = loadedDocumentIdRef.current;
    if (!sourceId || !isDocumentReady(sourceId)) return;
    clearSaveTimer();
    setTransitionState(true);
    try {
      if (recoveryCopyRef.current?.sourceId !== sourceId) {
        const copy = await createCanvasDocument(activeProjectId, t("canvas.recoveredCanvas"));
        revisionsRef.current.set(copy.id, copy.revision ?? 0);
        recoveryCopyRef.current = { sourceId, copyId: copy.id };
      }
      const copyId = recoveryCopyRef.current.copyId;
      await persistDocument(copyId, historyPresentRef.current);
      acknowledgeCanvasSaveRecovery(sourceId);
      setDirtyState(false);
      setHasSaveError(false);
      setHasSaveConflict(false);
      setTransitionState(false);
      await transitionToDocument(copyId);
      await refetch();
      recoveryCopyRef.current = null;
    } catch {
      setHasSaveError(true);
    } finally {
      setTransitionState(false);
    }
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

  return {
    selectedDocumentId,
    loadedDocumentId,
    documents,
    history,
    content,
    activeLayerId,
    setActiveLayerId,
    selectedObjectIds,
    setSelectedObjectIds,
    isEditorReady,
    isDocumentReady,
    updateContent,
    persistDocument,
    mountedRef,
    loadedDocumentIdRef,
    historyPresentRef,
    loadRequestTokenRef,
    handleUndo,
    handleRedo,
    handleSaveCopy,
    handleCreateDocument,
    handleRetrySave,
    handleRetryLoad,
    handleSelectDocument,
    saveStatusLabel,
    hasSaveConflict,
    hasSaveError,
    hasLoadError,
    isTransitioning,
    missingAssets,
  };
}
