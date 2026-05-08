import type { GenerationResult, Message, MessageImage } from "../types";

export function generationsToMessages(
  generations: GenerationResult[],
): Message[] {
  const messages: Message[] = [];

  for (const gr of generations) {
    const images: MessageImage[] = gr.images.map((img) => ({
      imageId: img.id,
      generationId: img.generation_id,
      path: img.file_path,
      thumbnailPath: img.thumbnail_path,
    }));

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
