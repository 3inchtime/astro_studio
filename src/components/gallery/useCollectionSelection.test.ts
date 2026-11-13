import { act, renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import { useCollectionSelection } from "./useCollectionSelection";

it("selects loaded rows only, retains selection on append and removes successful actions", () => {
  const { result, rerender } = renderHook(({ ids }) => useCollectionSelection(ids), { initialProps: { ids: ["a", "b"] } });
  act(() => result.current.selectLoaded());
  expect([...result.current.selectedIds]).toEqual(["a", "b"]);
  rerender({ ids: ["a", "b", "c"] });
  expect([...result.current.selectedIds]).toEqual(["a", "b"]);
  act(() => result.current.remove(["a"]));
  expect([...result.current.selectedIds]).toEqual(["b"]);
  rerender({ ids: ["c"] });
  expect([...result.current.selectedIds]).toEqual([]);
});
