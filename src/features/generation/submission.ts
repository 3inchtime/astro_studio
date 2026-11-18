import type { TFunction } from "i18next";
import type { Message, RetryGenerationRequest } from "../../types";
import { editImage, generateImage, type EnqueueGenerationResult } from "../../lib/api";
import { editSourcesToMessageImages } from "../../lib/generatePageHelpers";
import { buildEditParams, buildGenerationParams, modelSupportsEdit } from "../../lib/generationRequests";
import { image26ValidationError } from "../../lib/image26Options";
import { getImageModelCatalogEntry } from "../../lib/modelCatalog";

type Submission = RetryGenerationRequest & { clientRequestId: string };

export function prepareSubmission(request: RetryGenerationRequest): Submission {
  return {
    ...request, prompt: request.prompt.trim(), clientRequestId: crypto.randomUUID(),
    editSources: modelSupportsEdit(request.model) ? request.editSources.map(source => ({ ...source })) : [],
  };
}

export function pendingMessages(request: Submission): Message[] {
  const id = request.clientRequestId;
  const createdAt = new Date().toISOString();
  return [
    {
      id: `user-${id}`, role: "user", content: request.prompt, status: "complete", createdAt,
      sourceImages: editSourcesToMessageImages(request.editSources, id),
    },
    {
      id: `assistant-${id}`, role: "assistant", content: "", generationId: id, createdAt,
      status: "processing", retryRequest: request,
    },
  ];
}

export function dispatchSubmission(request: Submission, t: TFunction, onEnqueued?: (receipt: EnqueueGenerationResult) => void) {
  const invalid = image26ValidationError({
    model: request.model, format: request.outputFormat, size: request.size, count: request.imageCount,
    background: request.background, editing: request.editSources.length > 0,
    options: request.advancedOptions ?? {}, compression: request.outputCompression,
  });
  if (invalid) throw new Error(t(`generate.advanced.${invalid}`));
  const limit = getImageModelCatalogEntry(request.model).maxReferenceImages ?? 16;
  if (request.editSources.length > limit) throw new Error(t("generate.tooManyReferences", { count: limit }));
  const callbacks = onEnqueued ? [onEnqueued] as const : [] as const;
  return request.editSources.length
    ? editImage(buildEditParams(request), ...callbacks)
    : generateImage(buildGenerationParams(request), ...callbacks);
}
