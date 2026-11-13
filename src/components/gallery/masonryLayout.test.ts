import { describe, expect, it } from "vitest";
import { distributeMasonry } from "./masonryLayout";
import type { GenerationResult } from "../../types";
const row = (id: number, width = 100, height = 100) => ({ generation: { id: String(id) }, images: [{ id: `image-${id}`, width, height }] }) as GenerationResult;
describe("distributeMasonry", () => {
  it("occupies every available column even with only six records", () => {
    const columns = distributeMasonry(Array.from({ length: 6 }, (_, i) => row(i)), 5, 220);
    expect(columns.map((column) => column.length)).toEqual([2, 1, 1, 1, 1]);
  });
  it("retains column assignments while appending a thousand mixed-aspect records", () => {
    const rows = Array.from({ length: 1000 }, (_, i) => row(i, i % 2 ? 150 : 100, i % 3 ? 100 : 150));
    const before = distributeMasonry(rows.slice(0, 20), 4, 240);
    const after = distributeMasonry(rows, 4, 240);
    before.forEach((column, i) => expect(after[i].slice(0, column.length)).toEqual(column));
    expect(after.flat()).toHaveLength(1000);
    expect(new Set(after.flat().map((entry) => entry.generation.id)).size).toBe(1000);
  });
  it("excludes records without a displayable image", () => {
    expect(distributeMasonry([{ generation: { id: "failed" }, images: [] } as unknown as GenerationResult, row(1)], 4, 220).flat()).toEqual([row(1)]);
  });
});
