import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GenerationDetailPanel from "./GenerationDetailPanel";
import type { GenerationResult } from "../../types";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("../../lib/api", () => ({
  saveImageToFile: vi.fn(),
  toAssetUrl: (path: string) => path,
}));

function buildResult(): GenerationResult {
  return {
    generation: {
      id: "generation-1",
      prompt: "Glass observatory",
      engine: "gpt-image-2",
      request_kind: "generate",
      size: "1024x1024",
      quality: "auto",
      background: "auto",
      output_format: "png",
      output_compression: 100,
      moderation: "auto",
      input_fidelity: "high",
      image_count: 2,
      source_image_count: 0,
      source_image_paths: [],
      status: "completed",
      created_at: "2026-05-07T00:00:00Z",
      deleted_at: null,
    },
    images: [
      {
        id: "image-1",
        generation_id: "generation-1",
        file_path: "/tmp/full-1.png",
        thumbnail_path: "/tmp/thumb-1.png",
        width: 1024,
        height: 1024,
        file_size: 1024,
      },
      {
        id: "image-2",
        generation_id: "generation-1",
        file_path: "/tmp/full-2.png",
        thumbnail_path: "/tmp/thumb-2.png",
        width: 1536,
        height: 1024,
        file_size: 2048,
      },
    ],
  };
}

describe("GenerationDetailPanel", () => {
  it("presents an accessible drawer with a named close action on smaller windows", () => {
    const onClose = vi.fn();
    render(<GenerationDetailPanel result={buildResult()} title="Detail" onClose={onClose} onDelete={vi.fn()} />);
    expect(screen.getByRole("dialog", { name: "Detail" })).toHaveAttribute("aria-modal", "true");
    fireEvent.click(screen.getByRole("button", { name: "studio.collection.closeDetail" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("opens the preview for the currently selected detail image", () => {
    const onPreview = vi.fn();

    render(
      <GenerationDetailPanel
        result={buildResult()}
        title="Detail"
        onClose={vi.fn()}
        onDelete={vi.fn()}
        onPreview={onPreview}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "lightbox.preview 2" }));
    fireEvent.click(screen.getByRole("button", {
      name: "lightbox.preview Glass observatory",
    }));

    expect(onPreview).toHaveBeenCalledWith(1, expect.objectContaining({ imageId: "image-2" }));
  });
});
