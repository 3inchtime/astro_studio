import type {
  GeneratedImage,
  GenerateResponse,
  GenerationResult,
  Message,
  MessageImage,
} from "../types";

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
      status:
        gr.generation.status === "completed"
          ? "complete"
          : gr.generation.status === "failed"
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
