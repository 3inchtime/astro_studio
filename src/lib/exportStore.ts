import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import {
  cancelExport,
  exportGenerations,
  type BatchExportResult,
} from "./exportApi";
interface ExportState {
  busy: boolean;
  operationId: string | null;
  completed: number;
  total: number;
  result: BatchExportResult | null;
  error: string | null;
  start: (ids: string[], includeRecipe: boolean) => Promise<void>;
  retry: () => Promise<void>;
  cancel: () => Promise<void>;
  dismiss: () => void;
}
export const useExportStore = create<ExportState>((set, get) => {
  async function run(
    ids: string[],
    includeRecipe: boolean,
    retryToken: string | null,
  ) {
    if (get().busy) return;
    const id = crypto.randomUUID();
    set({
      busy: true,
      operationId: id,
      error: null,
      ...(!retryToken ? { result: null, completed: 0, total: 0 } : {}),
    });
    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<{
        operation_id: string;
        completed: number;
        total: number;
      }>("library:operation-progress", (event) => {
        if (event.payload.operation_id === id)
          set({
            completed: event.payload.completed,
            total: event.payload.total,
          });
      });
      const result = await exportGenerations(
        ids,
        includeRecipe,
        id,
        retryToken,
      );
      set({
        result,
        completed: result?.completed ?? 0,
        total: result?.total ?? 0,
      });
    } catch {
      set({ error: "exportFailed" });
    } finally {
      unlisten?.();
      set({ busy: false, operationId: null });
    }
  }
  return {
    busy: false,
    operationId: null,
    completed: 0,
    total: 0,
    result: null,
    error: null,
    start: (ids, includeRecipe) => run(ids, includeRecipe, null),
    retry: async () => {
      const token = get().result?.retry_token;
      if (token) await run([], false, token);
    },
    cancel: async () => {
      const id = get().operationId;
      if (id)
        try {
          await cancelExport(id);
        } catch {
          set({ error: "cancelFailed" });
        }
    },
    dismiss: () => {
      if (!get().busy)
        set({ result: null, error: null, completed: 0, total: 0 });
    },
  };
});
