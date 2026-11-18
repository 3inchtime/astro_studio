import ConfirmDialog from "../components/common/ConfirmDialog";
import { DEFAULT_TEXT_STYLE, normalizeTextStyle } from "../lib/canvas/text";
import RecoveryNotice from "../components/generate/RecoveryNotice";
import { useEffect, useRef, useState } from "react";
import { PanelLeft, PanelRight } from "lucide-react";
import { useDialogFocus } from "../hooks/useDialogFocus";
import { useTranslation } from "react-i18next";
import { importCanvasAsset, pickSourceImages } from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import CanvasAssetSidebar from "../components/canvas/CanvasAssetSidebar";
import CanvasGenerationPanel from "../components/canvas/CanvasGenerationPanel";
import CanvasPropertiesPanel from "../components/canvas/CanvasPropertiesPanel";
import { alignCanvasObjects, distributeCanvasObjects, scaleCanvasObjects, updateCanvasObjectStyle, type CanvasObjectStyle } from "../lib/canvas/editor";
import type { CanvasRect } from "../lib/canvas/bounds";
import CanvasLayersPanel from "../components/canvas/CanvasLayersPanel";
import CanvasStage from "../components/canvas/CanvasStage";
import CanvasToolbar from "../components/canvas/CanvasToolbar";
import { copyCanvasObjects, pasteCanvasObjects } from "../lib/canvas/clipboard";
import type { CanvasClipboard } from "../lib/canvas/clipboard";
import { getCombinedCanvasBounds } from "../lib/canvas/bounds";
import { cloneCanvasDocumentContent, createCanvasLayer, createImageObject, getActiveLayer, removeCanvasObjects, resetImageObjectAspect, updateImageObject } from "../lib/canvas/document";
import { exportCanvasFrame, readImageSize } from "../lib/canvas/export";
import { clampZoom, fitViewportToCanvasRect, resizeFrameByAspect } from "../lib/canvas/frame";
import { reorderCanvasObjects } from "../lib/canvas/ordering";
import type { CanvasOrderDirection } from "../lib/canvas/ordering";
import { getSelectableCanvasObjects, reconcileSelectedObjectIds } from "../lib/canvas/selection";
import { translateCanvasObjects } from "../lib/canvas/transforms";
import type { CanvasDocumentContent, CanvasMissingAsset, CanvasFrame, CanvasLayer, CanvasStrokeObject, CanvasObject, CanvasTool } from "../types";

import { useCanvasDocumentSession } from "../features/canvas/useCanvasDocumentSession";
import { useCanvasGeneration } from "../features/canvas/useCanvasGeneration";

export default function CanvasPage() {
  const { t } = useTranslation();
  const [compactPanel, setCompactPanel] = useState<
    "documents" | "inspector" | null
  >(null);
  const documentsToggleRef = useRef<HTMLButtonElement>(null);
  const inspectorToggleRef = useRef<HTMLButtonElement>(null);
  const [dockLayout, setDockLayout] = useState(() => ({
    documents: window.matchMedia("(min-width: 1440px)").matches,
    inspector: window.matchMedia("(min-width: 1200px)").matches,
  }));
  const [inspectorTab, setInspectorTab] = useState<"properties" | "layers" | "generation">("properties");
  const previousCompactPanel = useRef(compactPanel);
  const drawerOpen = Boolean(compactPanel && !dockLayout[compactPanel]);
  const documentsPanelRef = useDialogFocus({
    open: !dockLayout.documents && compactPanel === "documents",
    onClose: () => setCompactPanel(null),
  });
  const inspectorPanelRef = useDialogFocus({
    open: !dockLayout.inspector && compactPanel === "inspector",
    onClose: () => setCompactPanel(null),
  });
  useEffect(() => {
    const documents = window.matchMedia("(min-width: 1440px)");
    const inspector = window.matchMedia("(min-width: 1200px)");
    const update = () => {
      const next = { documents: documents.matches, inspector: inspector.matches };
      setDockLayout(next);
      setCompactPanel((current) => current && next[current] ? null : current);
    };
    documents.addEventListener("change", update);
    inspector.addEventListener("change", update);
    return () => {
      documents.removeEventListener("change", update);
      inspector.removeEventListener("change", update);
    };
  }, []);
  useEffect(() => {
    const previous = previousCompactPanel.current;
    previousCompactPanel.current = compactPanel;
    if (previous && !compactPanel && !dockLayout[previous]) {
      (previous === "documents" ? documentsToggleRef : inspectorToggleRef).current?.focus();
    }
  }, [compactPanel, dockLayout]);
  const { activeProjectId } = useLayoutContext();
  const session = useCanvasDocumentSession(activeProjectId);
  const {
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
  } = session;
  const {
    prompt,
    setPrompt,
    imageModel,
    partialPreview,
    generationParameters,
    setGenerationParameters,
    isGenerating,
    latestRequest,
    retryGeneration, confirmRetry, cancelRetry, showRetryReview,
    handleGenerate,
    isDesktopRuntime,
  } = useCanvasGeneration(session, activeProjectId);
  const [importError, setImportError] = useState<string | null>(null);
  const [fillColor, setFillColor] = useState("#c7d2fe");
  const [objectOpacity, setObjectOpacity] = useState(1);
  const [fontSize, setFontSize] = useState(32);
  const [textStyle, setTextStyle] = useState({ ...DEFAULT_TEXT_STYLE, rotation: 0 });
  const [frameEditing, setFrameEditing] = useState(false);
  const [activeTool, setActiveTool] = useState<CanvasTool>("brush");
  const [strokeColor, setStrokeColor] = useState("#1f2937");
  const [strokeSize, setStrokeSize] = useState(6);
  const [clipboard, setClipboard] = useState<CanvasClipboard | null>(null);
  const [stageSize, setStageSize] = useState({ width: 960, height: 640 });
  const [relinkingAsset, setRelinkingAsset] = useState<string | null>(null);


  const activeLayer = getActiveLayer(content, activeLayerId);
  const frame = content.frame;


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
        paths.map(async (path) => {
          const asset = await importCanvasAsset(path);
          return { path: asset.file_path, assetId: asset.id, size: await readImageSize(asset.file_path) };
        }),
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
      const objects = images.map(({ path, assetId, size }) => {
        const width = Math.min(960, size.width),
          height = (size.height / size.width) * width;
        const object = createImageObject({
          image_path: path,
          asset_id: assetId,
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

  async function handleRelinkAsset(missing: CanvasMissingAsset) {
    const documentId = loadedDocumentIdRef.current;
    const token = loadRequestTokenRef.current;
    if (!documentId || relinkingAsset) return;
    setRelinkingAsset(missing.object_id);
    setImportError(null);
    try {
      const paths = await pickSourceImages();
      if (!paths.length) return;
      const asset = await importCanvasAsset(paths[0]);
      const size = await readImageSize(asset.file_path);
      if (!mountedRef.current || !isDocumentReady(documentId) || token !== loadRequestTokenRef.current) return;
      const next = {
        ...historyPresentRef.current,
        layers: historyPresentRef.current.layers.map((layer) => ({
          ...layer,
          objects: layer.objects.map((object) => object.id === missing.object_id && object.type === "image"
            ? { ...object, asset_id: asset.id, image_path: asset.file_path, original_width: size.width, original_height: size.height }
            : object),
        })),
      };
      if (updateContent(next, undefined, documentId)) await persistDocument(documentId, next);
    } catch {
      if (mountedRef.current && token === loadRequestTokenRef.current) setImportError(t("canvas.relinkError"));
    } finally {
      if (mountedRef.current) setRelinkingAsset(null);
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
    if (object.type === "text") setActiveTool("select");
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


  function handleZoom(direction: "in" | "out") {
    const factor = direction === "in" ? 1.12 : 0.9;
    handleViewportChange({
      ...content.viewport,
      scale: clampZoom(content.viewport.scale * factor),
    });
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (drawerOpen) return;
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
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
        if (shapeTools[key] === "text") setSelectedObjectIds([]);
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
    compactPanel,
    drawerOpen,
    clipboard,
    content,
    history.future.length,
    history.past.length,
    loadedDocumentId,
    selectedDocumentId,
    selectedObjectIds,
  ]);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden bg-background">
      <RecoveryNotice />
      <ConfirmDialog
        open={showRetryReview}
        title={t("generate.unknownRetryTitle")}
        description={t("generate.unknownRetryDescription")}
        confirmLabel={t("generate.confirmUnknownRetry")}
        cancelLabel={t("favorites.cancel")}
        onCancel={cancelRetry}
        onConfirm={confirmRetry}
      />
      <div className="flex h-12 shrink-0 items-center justify-between border-b border-border-subtle bg-surface px-3 min-[1440px]:hidden">
        <button
          ref={documentsToggleRef}
          type="button"
          aria-controls="canvas-documents"
          aria-expanded={compactPanel === "documents"}
          onClick={() =>
            setCompactPanel(compactPanel === "documents" ? null : "documents")
          }
          className="focus-ring flex h-9 items-center gap-2 rounded-lg px-3 text-sm text-foreground hover:bg-subtle"
        >
          <PanelLeft size={16} />
          {t("studio.canvas.documents", { defaultValue: "Documents" })}
        </button>
        <button
          ref={inspectorToggleRef}
          type="button"
          aria-controls="canvas-inspector"
          aria-expanded={compactPanel === "inspector"}
          onClick={() =>
            setCompactPanel(compactPanel === "inspector" ? null : "inspector")
          }
          className="focus-ring ml-auto flex h-9 items-center gap-2 rounded-lg px-3 text-sm text-foreground hover:bg-subtle min-[1200px]:hidden"
        >
          <PanelRight size={16} />
          {t("studio.canvas.inspector", { defaultValue: "Inspector" })}
        </button>
      </div>
      <div
        aria-label={t("canvas.workspaceLabel")}
        className="relative grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)] bg-background min-[1200px]:grid-cols-[minmax(0,1fr)_300px] min-[1440px]:grid-cols-[220px_minmax(0,1fr)_300px]"
      >
        {drawerOpen ? (
          <button
            type="button"
            aria-label={t("studio.canvas.closePanel", {
              defaultValue: "Close panel",
            })}
            onClick={() => setCompactPanel(null)}
            className="absolute inset-0 z-30 bg-background/40"
          />
        ) : null}
        <div
          ref={documentsPanelRef}
          tabIndex={-1}
          role={
            !dockLayout.documents && compactPanel === "documents" ? "dialog" : undefined
          }
          aria-modal={
            !dockLayout.documents && compactPanel === "documents" ? true : undefined
          }
          aria-label={t("studio.canvas.documents", {
            defaultValue: "Documents",
          })}
          data-layout={dockLayout.documents ? "docked" : "drawer"}
          id="canvas-documents"
          className={`${compactPanel === "documents" ? "block" : "hidden"} absolute inset-y-0 left-0 z-40 w-[220px] max-w-full bg-surface shadow-float min-[1440px]:static min-[1440px]:block min-[1440px]:min-h-0 min-[1440px]:shadow-none`}
        >
          <CanvasAssetSidebar
            documents={documents}
            selectedDocumentId={selectedDocumentId}
            onSelectDocument={(id) => {
              handleSelectDocument(id);
              setCompactPanel(null);
            }}
            onCreateDocument={() => void handleCreateDocument()}
          />
        </div>
        <section className="min-h-0 min-w-0 bg-canvas">
          {selectedDocumentId ? (
            isEditorReady ? (
              <div className="relative h-full min-h-0 overflow-hidden border-x border-border-subtle bg-canvas">
                <CanvasStage
                  content={content}
                  shortcutsDisabled={drawerOpen}
                  activeLayerId={activeLayerId}
                  activeTool={activeTool}
                  selectedObjectIds={selectedObjectIds}
                  strokeColor={strokeColor}
                  strokeSize={strokeSize}
                  fillColor={fillColor}
                  objectOpacity={objectOpacity}
                  fontSize={fontSize}
                  textStyle={textStyle}
                  onRotateText={(id, angle) => updateContent(updateCanvasObjectStyle(content, [id], { rotation: angle }))}
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
                  className="absolute right-5 top-5 z-10 flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface/88 px-3 py-2 text-xs font-medium text-muted shadow-card backdrop-blur-xl"
                >
                  <span className="max-w-xs">{hasSaveConflict ? t("canvas.saveConflict") : saveStatusLabel}</span>
                  {hasSaveError ? (
                    <button
                      type="button"
                      className="focus-ring cursor-pointer rounded-[7px] bg-primary/10 px-2 py-1 font-semibold text-primary hover:bg-primary/15"
                      onClick={hasSaveConflict ? () => void handleSaveCopy() : handleRetrySave}
                    >
                      {t(hasSaveConflict ? "canvas.saveCopy" : "canvas.retrySave")}
                    </button>
                  ) : null}
                </div>
                {missingAssets.some((asset) => content.layers.some((layer) => layer.objects.some((object) => object.id === asset.object_id && object.type === "image" && object.image_path === asset.image_path))) && (
                  <div role="status" className="absolute bottom-20 right-5 z-30 max-h-[40%] w-72 max-w-[calc(100%-5rem)] overflow-y-auto rounded-xl border border-border bg-surface p-3 text-body-sm text-foreground">
                    <p className="mb-2 text-muted">{t("canvas.missingAssets")}</p>
                    {missingAssets.filter((asset) => content.layers.some((layer) => layer.objects.some((object) => object.id === asset.object_id && object.type === "image" && object.image_path === asset.image_path))).map((asset) => (
                      <div key={asset.object_id} className="flex min-w-0 items-center gap-2 py-1">
                        <span className="min-w-0 flex-1 truncate" title={asset.image_path}>{asset.image_path.split(/[\\/]/).pop()}</span>
                        <button type="button" disabled={Boolean(relinkingAsset)} aria-label={`${t("canvas.relinkAsset")} ${asset.image_path.split(/[\\/]/).pop()}`} className="focus-ring shrink-0 rounded-lg border border-control-border px-3 py-2 disabled:opacity-50" onClick={() => void handleRelinkAsset(asset)}>{t(relinkingAsset === asset.object_id ? "canvas.relinkingAsset" : "canvas.relinkAsset")}</button>
                      </div>
                    ))}
                    {importError && <p role="alert" className="mt-2 text-error">{importError}</p>}
                  </div>
                )}
                <div
                  data-testid="canvas-floating-toolbar"
                  className="pointer-events-none absolute inset-0 z-20"
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
                      if (tool === "text") setSelectedObjectIds([]);
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
                  <p className="text-body-sm font-medium text-foreground">
                    {t("canvas.loadError")}
                  </p>
                  <button
                    type="button"
                    className="studio-control-primary focus-ring mt-3 cursor-pointer rounded-[9px] px-3 py-2 text-xs font-semibold"
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
                  className="text-body-sm font-medium text-muted"
                >
                  {t("canvas.loading")}
                </div>
              </div>
            )
          ) : (
            <div className="flex h-full min-h-0 items-center justify-center border-x border-border-subtle px-6">
              <div
                data-testid="canvas-empty-state-card"
                className="w-[min(360px,calc(100%-48px))] max-w-full rounded-xl border border-border-subtle bg-surface/92 px-8 py-10 text-center shadow-card"
              >
                <h1 className="text-[22px] font-semibold text-foreground">
                  {t("canvas.title")}
                </h1>
                <p className="mt-3 text-sm leading-6 text-muted">
                  {t("canvas.noDocumentSelected")}
                </p>
              </div>
            </div>
          )}
        </section>

        <aside
          ref={inspectorPanelRef}
          tabIndex={-1}
          role={
            !dockLayout.inspector && compactPanel === "inspector" ? "dialog" : undefined
          }
          aria-modal={
            !dockLayout.inspector && compactPanel === "inspector" ? true : undefined
          }
          data-layout={dockLayout.inspector ? "docked" : "drawer"}
          id="canvas-inspector"
          aria-label={t("canvas.inspectorLabel")}
          className={`${compactPanel === "inspector" ? "flex" : "hidden"} absolute inset-y-0 right-0 z-40 w-[300px] max-w-full min-h-0 flex-col border-l border-border-subtle bg-surface shadow-float min-[1200px]:static min-[1200px]:flex min-[1200px]:shadow-none`}
        >
          <div role="tablist" aria-label={t("studio.canvas.inspector", { defaultValue: "Inspector" })} className="flex shrink-0 border-b border-border-subtle px-2 pt-2">
            {(["properties", "layers", "generation"] as const).map((tab) => (
              <button
                key={tab}
                type="button"
                role="tab"
                id={`canvas-tab-${tab}`}
                aria-controls={`canvas-panel-${tab}`}
                aria-selected={inspectorTab === tab}
                tabIndex={inspectorTab === tab ? 0 : -1}
                onClick={() => setInspectorTab(tab)}
                onKeyDown={(event) => {
                  const tabs = ["properties", "layers", "generation"] as const;
                  let next = tabs.indexOf(tab);
                  if (event.key === "ArrowRight") next = (next + 1) % tabs.length;
                  else if (event.key === "ArrowLeft") next = (next + tabs.length - 1) % tabs.length;
                  else if (event.key === "Home") next = 0;
                  else if (event.key === "End") next = tabs.length - 1;
                  else return;
                  event.preventDefault();
                  setInspectorTab(tabs[next]);
                  document.getElementById(`canvas-tab-${tabs[next]}`)?.focus();
                }}
                className={`focus-ring flex min-h-10 flex-1 items-center justify-center border-b-2 px-2 text-xs font-medium transition-colors duration-[var(--motion-feedback)] ${inspectorTab === tab ? "border-primary text-primary" : "border-transparent text-muted hover:bg-subtle hover:text-foreground"}`}
              >
                {t(tab === "properties" ? "canvas.propertiesTitle" : tab === "layers" ? "canvas.layersTitle" : "canvas.generationTitle")}
              </button>
            ))}
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            <div id="canvas-panel-generation" role="tabpanel" aria-labelledby="canvas-tab-generation" hidden={inspectorTab !== "generation"}>
            <CanvasGenerationPanel
              prompt={prompt}
              imageModel={imageModel}
              parameters={generationParameters}
              onParametersChange={setGenerationParameters}
              frame={frame}
              disabled={
                !isEditorReady ||
                !prompt.trim() ||
                isGenerating ||
                !isDesktopRuntime
              }
              isGenerating={isGenerating}
              partialPreview={
                isGenerating &&
                partialPreview?.documentId === selectedDocumentId
                  ? partialPreview.url
                  : undefined
              }
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
              onRetry={() => void retryGeneration()}
              onFrameAspectChange={(aspect) =>
                handleFrameChange(resizeFrameByAspect(frame, aspect))
              }
              onFrameFromSelection={
                selectedObjectIds.length ? handleFrameFromSelection : undefined
              }
              onPromptChange={setPrompt}
              onGenerate={() => void handleGenerate()}
            />

            </div>
            {importError ? (
              <p role="alert" className="text-sm text-red-600">
                {importError}
              </p>
            ) : null}
            <div id="canvas-panel-properties" role="tabpanel" aria-labelledby="canvas-tab-properties" hidden={inspectorTab !== "properties"}>
            {isEditorReady && selectedObjectIds.length ? (
              <CanvasPropertiesPanel
                objects={getSelectableCanvasObjects(content)
                  .filter((e) => selectedObjectIds.includes(e.object.id))
                  .map((e) => e.object)}
                onChange={handleStyle}
              />
            ) : isEditorReady && activeTool === "text" ? (
              <CanvasPropertiesPanel
                objects={[{ type: "text", id: "new-text", text: "", x: 0, y: 0, width: 240, height: 40, ...textStyle, fontSize, color: strokeColor, opacity: objectOpacity }]}
                onChange={(style) => {
                  setTextStyle((current) => ({ ...normalizeTextStyle({ ...current, ...style }), rotation: style.rotation ?? current.rotation }));
                  if (style.fontSize !== undefined) setFontSize(style.fontSize);
                  if (style.color !== undefined) setStrokeColor(style.color);
                  if (style.opacity !== undefined) setObjectOpacity(style.opacity);
                }}
              />
            ) : (
              <div className="px-5 py-6 text-body-sm leading-6 text-muted">
                {t("studio.canvas.selectObjectHint", { defaultValue: "Select an object to adjust its appearance." })}
              </div>
            )}
            </div>
            <div id="canvas-panel-layers" role="tabpanel" aria-labelledby="canvas-tab-layers" hidden={inspectorTab !== "layers"}>
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
