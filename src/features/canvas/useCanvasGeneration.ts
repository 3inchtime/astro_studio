import { useEffect, useRef, useState } from "react";
import { generationErrorMessage } from "../../lib/generationMessages";
import { image26ValidationError } from "../../lib/image26Options";
import { normalizeAdvancedOptions } from "../../lib/generationRequests";
import { DEFAULT_IMAGE_MODEL, supportsAdvancedImageOptions } from "../../lib/modelCatalog";
import { useTranslation } from "react-i18next";
import { editImage, getImageModel, getCanvasGenerationStatus, hasTauriRuntime, onGenerationPartial, saveCanvasExport } from "../../lib/api";
import { completeCanvasGeneration, failCanvasGeneration } from "../../lib/canvas/generation";
import { useCanvasGenerationRecovery } from "../../lib/canvas/useCanvasGenerationRecovery";
import { cloneCanvasDocumentContent } from "../../lib/canvas/document";
import { exportCanvasFrame } from "../../lib/canvas/export";
import type { CanvasDocumentContent, ImageModel, GenerationParams } from "../../types";

import type { useCanvasDocumentSession } from "./useCanvasDocumentSession";

export function useCanvasGeneration(session: ReturnType<typeof useCanvasDocumentSession>, activeProjectId: string | null) {
  const { t } = useTranslation();
  const {
    selectedDocumentId,
    loadedDocumentId,
    documents,
    content,
    isEditorReady,
    isDocumentReady,
    updateContent,
    persistDocument,
    mountedRef,
    loadedDocumentIdRef,
    historyPresentRef,
  } = session;
  const [prompt, setPrompt] = useState("");
  const [imageModel, setImageModel] = useState<ImageModel>(
    DEFAULT_IMAGE_MODEL,
  );
  const [partialPreview, setPartialPreview] = useState<{
    documentId: string;
    url: string;
  } | null>(null);
  const [generationParameters, setGenerationParameters] =
    useState<GenerationParams>({
      prompt: "",
      size: "auto",
      outputFormat: "png",
      quality: "auto",
    });
  const [submittingDocumentId, setSubmittingDocumentId] = useState<
    string | null
  >(null);
  const [retryReview, setRetryReview] = useState<{ documentId: string; requestId: string } | null>(null);
  useEffect(() => setRetryReview(null), [selectedDocumentId]);
  const liveRequestsRef = useRef(new Set<string>());

  const isDesktopRuntime = hasTauriRuntime();

  useEffect(() => {
    getImageModel()
      .then((model) => setImageModel(model))
      .catch(() => {});
  }, []);

  useEffect(() => {
    setGenerationParameters((current) =>
      current.advancedOptions?.markupPath || current.advancedOptions?.maskPath
        ? {
            ...current,
            advancedOptions: {
              ...current.advancedOptions,
              markupPath: undefined,
              maskPath: undefined,
            },
          }
        : current,
    );
  }, [selectedDocumentId, content.frame, content.layers]);
  useEffect(() => {
    if (!isDesktopRuntime) return;
    let disposed = false;
    const subscription = onGenerationPartial((event) => {
      if (
        disposed ||
        !event.client_request_id ||
        (event.document_id && event.document_id !== loadedDocumentIdRef.current)
      )
        return;
      const documentId = loadedDocumentIdRef.current;
      if (
        !documentId ||
        !historyPresentRef.current.generation_requests?.some(
          (r) =>
            r.request_id === event.client_request_id && r.status === "pending",
        )
      )
        return;
      if (!["png", "jpeg", "webp"].includes(event.output_format ?? "png"))
        return;
      setPartialPreview({
        documentId,
        url: `data:image/${event.output_format ?? "png"};base64,${event.b64_json}`,
      });
    }).catch(() => undefined);
    return () => {
      disposed = true;
      void subscription.then(unlisten => unlisten?.());
    };
  }, [isDesktopRuntime]);

  const isGenerating =
    Boolean(
      submittingDocumentId && submittingDocumentId === loadedDocumentId,
    ) ||
    Boolean(content.generation_requests?.some((r) => r.status === "pending"));
  const latestRequest =
    content.generation_requests?.[content.generation_requests.length - 1];

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

  function retryIsCurrent(documentId: string, requestId: string) {
    const requests = historyPresentRef.current.generation_requests;
    const last = requests?.[requests.length - 1];
    return isDocumentReady(documentId) && last?.request_id === requestId && (last.status === "failed" || last.status === "cancelled");
  }

  async function retryGeneration() {
    const documentId = selectedDocumentId;
    const request = latestRequest;
    if (!documentId || !request || isGenerating || !retryIsCurrent(documentId, request.request_id)) return;
    let unknown = request.error_code === "provider_outcome_unknown";
    if (!request.error_code) {
      // A document saved by an earlier version can predate the durable error code.
      try {
        const saved = await getCanvasGenerationStatus(documentId, request.request_id);
        if (saved && !["failed", "cancelled", "interrupted"].includes(saved.status)) return;
        unknown = saved?.error_code === "provider_outcome_unknown";
      } catch {
        unknown = true;
      }
    }
    if (!retryIsCurrent(documentId, request.request_id)) return;
    if (unknown) {
      setRetryReview({ documentId, requestId: request.request_id });
      return;
    }
    await handleGenerate(request.prompt);
  }

  function confirmRetry() {
    setRetryReview(null);
    if (retryReview && latestRequest && retryIsCurrent(retryReview.documentId, retryReview.requestId)) {
      void handleGenerate(latestRequest.prompt);
    }
  }

  async function handleGenerate(retryPrompt?: string) {
    const documentId = selectedDocumentId;
    const submittedPrompt = (retryPrompt ?? prompt).trim();
    if (
      !isDocumentReady(documentId) ||
      !submittedPrompt ||
      isGenerating ||
      historyPresentRef.current.generation_requests?.some((request) => request.status === "pending") ||
      !isDesktopRuntime
    )
      return;
    const retrySourcePath = retryPrompt
      ? latestRequest?.source_image_path
      : undefined;
    const parameters =
      retryPrompt && latestRequest?.parameters
        ? latestRequest.parameters
        : { ...generationParameters, model: imageModel };
    const snapshot = cloneCanvasDocumentContent(historyPresentRef.current);
    const requestId = crypto.randomUUID();
    const projectId =
      documents.find((d) => d.id === documentId)?.project_id ?? activeProjectId;
    const request = {
      request_id: requestId,
      parameters,
      project_id: projectId,
      frame: {
        ...(retrySourcePath && latestRequest
          ? latestRequest.frame
          : snapshot.frame),
      },
      source_image_path: retrySourcePath,
      prompt: submittedPrompt,
      status: "pending" as const,
      image_ids: [],
      created_at: new Date().toISOString(),
    };
    setPartialPreview(null);
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
      const invalid = image26ValidationError({
        model: parameters.model ?? imageModel,
        format: parameters.outputFormat ?? "png",
        size: parameters.size ?? "auto",
        count: parameters.imageCount ?? 1,
        background: parameters.background ?? "auto",
        editing: true,
        options: parameters.advancedOptions ?? {},
        compression: parameters.outputCompression,
      });
      if (invalid) throw new Error(t(`generate.advanced.${invalid}`));
      await persistDocument(documentId, pending);
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      let exportedPath = retrySourcePath;
      if (!exportedPath) {
        const pngBase64 = await exportCanvasFrame(snapshot);
        if (!mountedRef.current || !isDocumentReady(documentId)) return;
        exportedPath = await saveCanvasExport(documentId, pngBase64);
        if (!mountedRef.current || !isDocumentReady(documentId)) return;
      }
      const withSource = {
        ...historyPresentRef.current,
        generation_requests: historyPresentRef.current.generation_requests?.map(
          (r) =>
            r.request_id === requestId
              ? { ...r, source_image_path: exportedPath }
              : r,
        ),
      };
      updateContent(withSource, { replace: true }, documentId);
      await persistDocument(documentId, withSource);
      if (!mountedRef.current || !isDocumentReady(documentId)) return;
      submissionStarted = true;
      const result = await editImage({
        ...(supportsAdvancedImageOptions(parameters.model ?? imageModel)
          ? {
              ...parameters,
              advancedOptions: normalizeAdvancedOptions(
                parameters.advancedOptions ?? {},
                parameters.outputFormat ?? "png",
                true,
              ),
              outputCompression: ["jpeg", "webp"].includes(
                parameters.outputFormat ?? "png",
              )
                ? parameters.outputCompression
                : undefined,
            }
          : {}),
        prompt: submittedPrompt,
        clientRequestId: requestId,
        documentId,
        model: parameters.model ?? imageModel,
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
            generationErrorMessage(error) ?? t("generate.generationFailed"),
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
              true,
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
              status.error ?? generationErrorMessage(error) ?? t("generate.generationFailed"),
              status.status === "cancelled" ? "cancelled" : "failed",
              status.generation_id ?? undefined,
              status.error_code ?? undefined,
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
                    error: generationErrorMessage(error) ?? t("generate.generationFailed"),
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

  return {
    prompt,
    setPrompt,
    imageModel,
    partialPreview,
    generationParameters,
    setGenerationParameters,
    isGenerating,
    latestRequest,
    retryGeneration, confirmRetry,
    cancelRetry: () => setRetryReview(null),
    showRetryReview: !!retryReview && retryReview.documentId === selectedDocumentId && retryIsCurrent(retryReview.documentId, retryReview.requestId),
    handleGenerate,
    isDesktopRuntime,
  };
}
