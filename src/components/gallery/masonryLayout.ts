import type { GenerationResult } from "../../types";

// Greedy placement keeps existing records in their columns when another page is appended.
export function distributeMasonry(rows: GenerationResult[], columnCount: number, columnWidth: number): GenerationResult[][] {
  const columns: GenerationResult[][] = Array.from({ length: Math.max(1, columnCount) }, () => []);
  const heights = columns.map(() => 0);
  for (const row of rows) {
    const image = row.images[0];
    if (!image) continue;
    let shortest = 0;
    for (let index = 1; index < heights.length; index++) {
      if (heights[index] < heights[shortest]) shortest = index;
    }
    columns[shortest].push(row);
    heights[shortest] += columnWidth * (image.width > 0 && image.height > 0 ? image.height / image.width : 1) + 52;
  }
  return columns;
}
