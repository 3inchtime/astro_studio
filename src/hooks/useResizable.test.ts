import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it } from "vitest";
import { useResizable } from "./useResizable";
const panels = [{ min: 180, default: 260, max: 400 }, { min: 400, default: 600, max: null }];
beforeEach(() => localStorage.clear());
it("rejects invalid stored widths and clamps out of range preferences", () => {
  localStorage.setItem("astro-layout-widths", '["bad",-42]');
  const { result } = renderHook(() => useResizable(panels));
  expect(result.current.widths).toEqual([260, 400]);
});
it("supports keyboard resizing within bounds", () => {
  const { result } = renderHook(() => useResizable(panels));
  act(() => result.current.setPanelWidth(0, 999));
  expect(result.current.widths[0]).toBe(400);
  act(() => result.current.setPanelWidth(0, -5));
  expect(result.current.widths[0]).toBe(180);
});
