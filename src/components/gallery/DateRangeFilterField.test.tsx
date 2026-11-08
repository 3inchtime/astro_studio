import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import DateRangeFilterField from "./DateRangeFilterField";
import { getDayPickerLocale } from "../../lib/dayPickerLocale";

describe("DateRangeFilterField", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  function renderField(onChange = vi.fn()) {
    render(<DateRangeFilterField
      label="Created date"
      value={{ from: "", to: "" }}
      displayValue="All time"
      locale={getDayPickerLocale("en")}
      presets={{ today: "Today", last7Days: "Last 7 days", last30Days: "Last 30 days", thisMonth: "This month", clear: "Clear", done: "Done" }}
      onChange={onChange}
    />);
    const trigger = screen.getByRole("button", { name: "Created date: All time" });
    trigger.focus();
    fireEvent.click(trigger);
    return trigger;
  }

  it("contains keyboard focus and restores the trigger after Escape", () => {
    const trigger = renderField();
    const first = screen.getByRole("button", { name: "Today", exact: true });
    const done = screen.getByRole("button", { name: "Done" });

    expect(first).toHaveFocus();
    expect(screen.getByRole("dialog", { name: "Created date" })).toHaveAttribute("aria-modal", "true");
    fireEvent.keyDown(first, { key: "Tab", shiftKey: true });
    expect(done).toHaveFocus();
    fireEvent.keyDown(done, { key: "Tab" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "Escape" });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("keeps date changes and restores focus after Done", () => {
    const onChange = vi.fn();
    const trigger = renderField(onChange);
    fireEvent.click(screen.getByRole("button", { name: "Today", exact: true }));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ from: expect.any(String), to: expect.any(String) }));
    const done = screen.getByRole("button", { name: "Done" });
    done.focus();
    fireEvent.click(done);

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(trigger).toHaveFocus();
  });

  it("still dismisses on an outside pointer press", () => {
    renderField();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens on the previous month and current month", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 4, 8));

    render(
      <DateRangeFilterField
        label="Created date"
        value={{ from: "", to: "" }}
        displayValue="All time"
        locale={getDayPickerLocale("en")}
        presets={{
          today: "Today",
          last7Days: "Last 7 days",
          last30Days: "Last 30 days",
          thisMonth: "This month",
          clear: "Clear",
          done: "Done",
        }}
        onChange={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Created date: All time" }));

    expect(screen.getByRole("grid", { name: "April 2026" })).toBeInTheDocument();
    expect(screen.getByRole("grid", { name: "May 2026" })).toBeInTheDocument();
    expect(screen.queryByRole("grid", { name: "June 2026" })).not.toBeInTheDocument();
  });
});
