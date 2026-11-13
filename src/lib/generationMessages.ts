import { IMAGE_MODEL_CATALOG } from "./modelCatalog";
import type {
  GeneratedImage,
  Generation,
  ImageModel,
  ImageSize,
  ImageQuality,
  ImageAdvancedOptions,
  RetryGenerationRequest,
  GenerateResponse,
  GenerationResult,
  Message,
  MessageImage,
} from "../types";

function persistedRetryRequest(
  generation: Generation,
): RetryGenerationRequest | undefined {
  if (
    !["failed", "interrupted"].includes(generation.status) ||
    !IMAGE_MODEL_CATALOG.some((entry) => entry.id === generation.engine) ||
    !generation.request_metadata
  )
    return undefined;
  try {
    const metadata = JSON.parse(generation.request_metadata);
    if (
      !metadata ||
      metadata.model !== generation.engine ||
      typeof metadata.project_id !== "string" ||
      typeof metadata.conversation_id !== "string"
    )
      return undefined;
    if (generation.size !== "auto" && !/^\d+x\d+$/.test(generation.size))
      return undefined;
    if (
      !["auto", "low", "medium", "high", "xhigh", "max"].includes(
        generation.quality,
      )
    )
      return undefined;
    const raw = metadata.advanced_options;
    if (
      raw !== undefined &&
      (!raw || typeof raw !== "object" || Array.isArray(raw))
    )
      return undefined;
    // Explicitly select supported keys; persisted metadata never becomes an arbitrary IPC payload.
    const options: ImageAdvancedOptions = {};
    if (raw) {
      if (["low", "medium", "high"].includes(raw.complexity))
        options.complexity = raw.complexity;
      for (const key of [
        "frameCount",
        "frameDurationMs",
        "partialImages",
      ] as const) {
        if (typeof raw[key] === "number" && Number.isSafeInteger(raw[key]))
          options[key] = raw[key];
      }
      if (typeof raw.stream === "boolean") options.stream = raw.stream;
      for (const key of ["markupPath", "maskPath"] as const)
        if (typeof raw[key] === "string") options[key] = raw[key];
      if (raw.svg && typeof raw.svg.model === "string") {
        options.svg = { model: raw.svg.model };
        if (
          ["none", "minimal", "low", "medium", "high", "xhigh", "max"].includes(
            raw.svg.reasoning?.effort,
          )
        )
          options.svg.reasoning = { effort: raw.svg.reasoning.effort };
      }
    }
    return {
      prompt: generation.prompt,
      sourceGenerationId: generation.id,
      aspectRatio:
        typeof metadata.aspect_ratio === "string"
          ? metadata.aspect_ratio
          : undefined,
      resolution:
        typeof metadata.resolution === "string"
          ? metadata.resolution
          : undefined,
      model: generation.engine as ImageModel,
      size: generation.size as ImageSize,
      quality: generation.quality as ImageQuality,
      background: generation.background,
      outputFormat: generation.output_format,
      outputCompression: generation.output_compression,
      moderation: generation.moderation,
      inputFidelity: generation.input_fidelity,
      imageCount: generation.image_count,
      projectId: metadata.project_id,
      conversationId: metadata.conversation_id,
      advancedOptions: options,
      editSources: generation.source_image_paths.map((path, index) => ({
        id: `${generation.id}-source-${index}`,
        path,
        label: path.split(/[\\/]/).pop() ?? path,
      })),
    };
  } catch {
    return undefined;
  }
}

function generatedImagesToMessageImages(
  images: GeneratedImage[],
): MessageImage[] {
  return images.map((img) => ({
    imageId: img.id,
    generationId: img.generation_id,
    path: img.file_path,
    thumbnailPath: img.thumbnail_path,
  }));
}

export function generationsToMessages(
  generations: GenerationResult[],
): Message[] {
  const messages: Message[] = [];

  for (const gr of generations) {
    const retryRequest = persistedRetryRequest(gr.generation);
    const images = generatedImagesToMessageImages(gr.images);

    messages.push({
      id: `user-${gr.generation.id}`,
      role: "user",
      content: gr.generation.prompt,
      sourceImages: gr.generation.source_image_paths.map((path, index) => ({
        imageId: `${gr.generation.id}-source-${index}`,
        generationId: gr.generation.id,
        path,
        thumbnailPath: path,
      })),
      status: "complete",
      createdAt: gr.generation.created_at,
    });
    messages.push({
      id: `assistant-${gr.generation.id}`,
      role: "assistant",
      content: "",
      generationId: gr.generation.id,
      images,
      error: gr.generation.error_message ?? undefined,
      ...(retryRequest ? { retryRequest } : {}),
      status:
        gr.generation.status === "completed"
          ? "complete"
          : ["failed", "interrupted"].includes(gr.generation.status)
            ? "failed"
            : "processing",
      createdAt: gr.generation.created_at,
    });
  }

  return messages;
}

export function completeGenerationMessage(
  messages: Message[],
  localGenerationId: string,
  result: GenerateResponse,
): Message[] {
  return messages.map((message) => {
    const matchesLocalId = message.id === `assistant-${localGenerationId}`;
    const matchesGenerationId =
      message.generationId === result.generation_id ||
      message.id === `assistant-${result.generation_id}`;

    if (!matchesLocalId && !matchesGenerationId) {
      return message;
    }

    return {
      ...message,
      id: `assistant-${result.generation_id}`,
      generationId: result.generation_id,
      images: generatedImagesToMessageImages(result.images),
      status: "complete" as const,
      partialPreview: undefined,
    };
  });
}

export function failGenerationMessage(
  messages: Message[],
  localGenerationId: string,
  error: unknown,
  retryRequest: Message["retryRequest"],
): Message[] {
  return messages.map((message) =>
    message.id === `assistant-${localGenerationId}`
      ? {
          ...message,
          status: "failed" as const,
          error: String(error),
          retryRequest,
        }
      : message,
  );
}

export interface GenerationPartialEvent {
  generation_id: string;
  client_request_id?: string | null;
  partial_image_index: number;
  b64_json: string;
  output_format?: string;
}
export function applyGenerationPartial(
  messages: Message[],
  event: GenerationPartialEvent,
): Message[] {
  if (
    !event.b64_json ||
    !["png", "jpeg", "webp"].includes(event.output_format ?? "png")
  )
    return messages;
  return messages.map((message) =>
    message.role === "assistant" &&
    message.status === "processing" &&
    (message.generationId === event.generation_id ||
      (!!event.client_request_id &&
        message.id === `assistant-${event.client_request_id}`))
      ? {
          ...message,
          generationId: event.generation_id,
          partialPreview: `data:image/${event.output_format ?? "png"};base64,${event.b64_json}`,
        }
      : message,
  );
}
