import "../../i18n";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import DateFilterField from "./DateFilterField";

vi.mock("react-i18next", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-i18next")>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) =>
        ({
          "gallery.clearDate": "清除日期",
        })[key] ?? key,
    }),
  };
});

describe("DateFilterField", () => {
  it("keeps the clear button above the transparent date input and localizes its label", () => {
    const onChange = vi.fn();

    render(
      <DateFilterField
        label="Created after"
        value="2026-05-17"
        onChange={onChange}
      />,
    );

    const clearButton = screen.getByRole("button", { name: "清除日期" });

    expect(clearButton).toHaveClass("z-20");

    fireEvent.click(clearButton);

    expect(onChange).toHaveBeenCalledWith("");
  });
});
