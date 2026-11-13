import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ThemeCardPicker } from "./ThemeCardPicker";
import { THEME_CATALOG, getThemeDescription, getThemeName } from "../../lib/themes";

const t = (key: string, options?: { name: string }) =>
  key === "theme.select" ? `Select ${options?.name} theme` : key;

describe("ThemeCardPicker compact choices", () => {
  it("keeps all twelve names accessible while moving descriptions into native help", () => {
    const onSelect = vi.fn();
    render(<ThemeCardPicker selectedThemeId="pure-light" activeLabel="Active" t={t} compact onSelect={onSelect} />);
    expect(screen.getAllByRole("button")).toHaveLength(12);
    for (const theme of THEME_CATALOG) {
      const name = getThemeName(theme, t);
      const description = getThemeDescription(theme, t);
      const choice = screen.getByRole("button", { name: `Select ${name} theme` });
      expect(choice).toHaveAttribute("title", `${name}\n${description}`);
      expect(choice).toHaveAttribute("aria-description", description);
      expect(choice).not.toHaveTextContent(description);
      expect(choice).toHaveAttribute("aria-pressed", String(theme.id === "pure-light"));
    }
    fireEvent.click(screen.getByRole("button", { name: "Select Pure Black theme" }));
    expect(onSelect).toHaveBeenCalledWith("pure-black", expect.anything());
  });
});
