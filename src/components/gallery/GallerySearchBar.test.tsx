import { render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GallerySearchBar from "./GallerySearchBar";
import type { GallerySearchConfig } from "../../lib/galleryFilterConfig";
import { getDayPickerLocale } from "../../lib/dayPickerLocale";

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
      type: "date-range",
      key: "created_range",
      label: "Created date",
      value: {
        from: "",
        to: "",
      },
      displayValue: "All time",
      locale: getDayPickerLocale("en"),
      presets: {
        today: "Today",
        last7Days: "Last 7 days",
        last30Days: "Last 30 days",
        thisMonth: "This month",
        clear: "Clear",
        done: "Done",
      },
      onChange: vi.fn(),
    },
  ],
};

describe("GallerySearchBar", () => {
  it("keeps prompt, model, range filters, and actions in one compact filter row", () => {
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

    expect(filterRow).toHaveClass("flex", "flex-wrap", "items-center");
    expect(filterRow).not.toHaveClass("grid");
    expect(within(filterRow).getByPlaceholderText("Search prompts...")).toBeInTheDocument();
    expect(within(filterRow).getByLabelText("Model")).toBeInTheDocument();
    expect(
      within(filterRow).getByRole("button", { name: /Created date/i }),
    ).toBeInTheDocument();
    expect(within(filterRow).queryByLabelText("Created from")).not.toBeInTheDocument();
    expect(within(filterRow).queryByLabelText("Created to")).not.toBeInTheDocument();
    expect(within(filterRow).getByRole("button", { name: "Apply" })).toBeInTheDocument();
    expect(within(filterRow).getByRole("button", { name: "Reset" })).toBeInTheDocument();
    expect(container.querySelector(".grid")).not.toBeInTheDocument();
    expect(within(filterRow).queryByText("Prompt")).not.toBeInTheDocument();
    expect(within(filterRow).queryByText("Model")).not.toBeInTheDocument();
    expect(within(filterRow).queryByText("Created date")).not.toBeInTheDocument();
    expect(within(filterRow).queryByText("Last 7 days")).not.toBeInTheDocument();
  });
});
