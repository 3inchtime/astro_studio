import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import DateRangeFilterField from "./DateRangeFilterField";
import { getDayPickerLocale } from "../../lib/dayPickerLocale";

describe("DateRangeFilterField", () => {
  afterEach(() => {
    vi.useRealTimers();
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
