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
import {
  cloneCanvasDocumentContent,
  createCanvasDocumentContent,
  createCanvasLayer,
  createImageObject,
  getActiveLayer,
  resetImageObjectAspect,
  sanitizeCanvasDocumentContent,
  updateImageObject,
} from "../lib/canvas/document";
import { exportCanvasFrame, readImageSize } from "../lib/canvas/export";
import { clampZoom } from "../lib/canvas/frame";
import {
  createHistory,
  pushHistory,
  redoHistory,
  replaceHistory,
  undoHistory,
} from "../lib/canvas/history";
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
  const isDesktopRuntime = hasTauriRuntime();
  const [history, setHistory] = useState(() =>
    createHistory<CanvasDocumentContent>(createCanvasDocumentContent()),
  );
  const saveTimerRef = useRef<number | null>(null);
  const loadingDocumentIdRef = useRef<string | null>(null);
  const { data: documents = [], refetch } = useCanvasDocumentsQuery(activeProjectId);

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
    if (!selectedDocumentId) {
      return;
    }

    if (loadingDocumentIdRef.current === selectedDocumentId) {
      return;
    }

    loadingDocumentIdRef.current = selectedDocumentId;
    getCanvasDocument(selectedDocumentId)
      .then((document) => {
        const nextContent = sanitizeCanvasDocumentContent(document.content);
        setHistory(createHistory(nextContent));
        setActiveLayerId(nextContent.layers[0]?.id ?? null);
        setIsDirty(false);
      })
      .catch(() => {})
      .finally(() => {
        loadingDocumentIdRef.current = null;
      });
  }, [selectedDocumentId]);

  useEffect(() => {
    if (!selectedDocumentId || !isDirty) {
      return;
    }

    if (saveTimerRef.current) {
      window.clearTimeout(saveTimerRef.current);
    }

    saveTimerRef.current = window.setTimeout(() => {
      void persistDocument(history.present);
    }, 500);

    return () => {
      if (saveTimerRef.current) {
        window.clearTimeout(saveTimerRef.current);
      }
    };
  }, [history.present, isDirty, selectedDocumentId]);

  const selectedDocument =
    documents.find((document) => document.id === selectedDocumentId) ?? null;
  const content = history.present;
  const activeLayer = getActiveLayer(content, activeLayerId);
  const frame = content.frame;

  const saveStatusLabel = useMemo(() => {
    if (isSaving) {
      return t("canvas.saveStatus.saving");
    }
    if (isDirty) {
      return t("canvas.saveStatus.dirty");
    }
    return t("canvas.saveStatus.saved");
  }, [isDirty, isSaving, t]);

  function updateContent(nextContent: CanvasDocumentContent, options?: { replace?: boolean }) {
    setHistory((current) =>
      options?.replace ? replaceHistory(current, nextContent) : pushHistory(current, nextContent),
    );
    setIsDirty(true);
  }

  async function persistDocument(nextContent: CanvasDocumentContent) {
    if (!selectedDocumentId) {
      return;
    }

    setIsSaving(true);
    try {
      const previewPngBase64 = await exportCanvasFrame(nextContent);
      await saveCanvasDocument(selectedDocumentId, nextContent, previewPngBase64);
      await refetch();
      setIsDirty(false);
    } finally {
      setIsSaving(false);
    }
  }

  async function handleCreateDocument() {
    const created = await createCanvasDocument(activeProjectId, null);
    await refetch();
    setSelectedDocumentId(created.id);
  }

  async function handleGenerate() {
    if (!selectedDocument || !prompt.trim() || isGenerating || !isDesktopRuntime) return;

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
    if (!selectedDocumentId || !activeLayer) {
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

    updateContent(nextContent);
    setActiveTool("select");
    await persistDocument(nextContent);
  }

  function handleAddLayer() {
    const layer = createCanvasLayer({
      name: `Layer ${content.layers.length + 1}`,
    });
    updateContent({
      ...cloneCanvasDocumentContent(content),
      layers: [layer, ...content.layers],
    });
    setActiveLayerId(layer.id);
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
    setHistory((current) => undoHistory(current));
    setIsDirty(true);
  }

  function handleRedo() {
    setHistory((current) => redoHistory(current));
    setIsDirty(true);
  }

  function handleZoom(direction: "in" | "out") {
    const factor = direction === "in" ? 1.12 : 0.9;
    handleViewportChange({
      ...content.viewport,
      scale: clampZoom(content.viewport.scale * factor),
    });
  }

  return (
    <div className="h-full min-h-0 overflow-x-auto bg-background">
      <div
        aria-label={t("canvas.workspaceLabel")}
        className="grid h-full min-w-[964px] min-h-0 grid-cols-[264px_minmax(360px,1fr)_340px] gap-0 bg-[linear-gradient(180deg,_rgba(255,255,255,0.72),_rgba(248,247,244,0.94))]"
      >
        <CanvasAssetSidebar
          documents={documents}
          selectedDocumentId={selectedDocumentId}
          onSelectDocument={setSelectedDocumentId}
          onCreateDocument={() => void handleCreateDocument()}
        />

        <section className="min-h-0 bg-background">
          {selectedDocument ? (
            <div className="relative h-full min-h-0 overflow-hidden border-x border-border-subtle bg-canvas">
                <CanvasStage
                  content={content}
                  activeLayerId={activeLayerId}
                  activeTool={activeTool}
                  strokeColor={strokeColor}
                  strokeSize={strokeSize}
                  onViewportChange={handleViewportChange}
                  onAddStroke={handleAddStroke}
                  onTransformImage={handleTransformImage}
                  onResetImageAspect={handleResetImageAspect}
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
                    onToolChange={setActiveTool}
                    onColorChange={setStrokeColor}
                    onSizeChange={setStrokeSize}
                    onUndo={handleUndo}
                    onRedo={handleRedo}
                    onZoomIn={() => handleZoom("in")}
                    onZoomOut={() => handleZoom("out")}
                    onImportImage={() => void handleImportImage()}
                  />
                </div>
              </div>
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
              disabled={!selectedDocument || !prompt.trim() || isGenerating || !isDesktopRuntime}
              isGenerating={isGenerating}
              environmentHint={
                isDesktopRuntime ? null : t("canvas.generationUnavailableBrowser")
              }
              onPromptChange={setPrompt}
              onGenerate={() => void handleGenerate()}
            />

            {selectedDocument ? (
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
