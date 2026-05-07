import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GallerySearchBar from "./GallerySearchBar";
import type { GallerySearchConfig } from "../../lib/galleryFilterConfig";

const config: GallerySearchConfig = {
  title: "Gallery",
  searchLabel: "Prompt",
  searchPlaceholder: "Search prompts...",
  applyFilters: "Apply",
  resetFilters: "Reset",
  fields: [
    {
      type: "select",
      key: "model",
      label: "Model",
      value: "",
      options: [
        { value: "", label: "All models" },
        { value: "gpt-image-2", label: "GPT Image 2" },
      ],
      onChange: vi.fn(),
    },
    {
      type: "date",
      key: "created_from",
      label: "Created from",
      value: "",
      onChange: vi.fn(),
    },
    {
      type: "date",
      key: "created_to",
      label: "Created to",
      value: "",
      onChange: vi.fn(),
    },
  ],
};

describe("GallerySearchBar", () => {
  it("keeps prompt, model, date filters, and actions in one compact filter row", () => {
    const { container } = render(
      <GallerySearchBar
        config={config}
        total={42}
        query=""
        hasActiveFilters={false}
        onQueryChange={vi.fn()}
        onSearch={vi.fn()}
        onReset={vi.fn()}
      />,
    );

    const filterRow = screen.getByRole("search", {
      name: "Gallery filters",
    });

    expect(filterRow).toHaveClass("flex", "flex-wrap", "items-end");
    expect(filterRow).not.toHaveClass("grid");
    expect(within(filterRow).getByPlaceholderText("Search prompts...")).toBeInTheDocument();
    expect(within(filterRow).getByLabelText("Model")).toBeInTheDocument();
    expect(within(filterRow).getByLabelText("Created from")).toBeInTheDocument();
    expect(within(filterRow).getByLabelText("Created to")).toBeInTheDocument();
    expect(within(filterRow).getByRole("button", { name: "Apply" })).toBeInTheDocument();
    expect(within(filterRow).getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(container.querySelector(".grid")).not.toBeInTheDocument();
  });
});
