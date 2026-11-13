type BatchRecord = { generation: { id: string }; images: { id: string }[] };

export function selectLoadedGenerations<T extends BatchRecord>(rows: T[], selected: Set<string>): T[] {
  return rows.filter((row) => selected.has(row.generation.id));
}

/** Checkpoints are scoped to one confirmed action and survive partial-failure retries. */
export async function runGenerationBatch(
  rows: BatchRecord[],
  kind: "delete" | "favorite",
  operation: (id: string) => Promise<void>,
  checkpoint: Set<string>,
): Promise<{ succeeded: string[]; failed: string[] }> {
  const succeeded: string[] = [];
  const failed: string[] = [];
  for (const row of rows) {
    try {
      const ids = kind === "delete" ? [row.generation.id] : row.images.map((image) => image.id);
      for (const id of ids) {
        if (checkpoint.has(id)) continue;
        await operation(id);
        checkpoint.add(id);
      }
      succeeded.push(row.generation.id);
    } catch {
      failed.push(row.generation.id);
    }
  }
  return { succeeded, failed };
}
