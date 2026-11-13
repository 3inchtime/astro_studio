import { invoke } from "@tauri-apps/api/core";
export interface BatchExportResult {
  total: number;
  completed: number;
  succeeded_image_ids: string[];
  cancelled_image_ids: string[];
  failed: { image_id: string; generation_id: string; code: string }[];
  cancelled: boolean;
  retry_token: string | null;
}
export const exportGenerations = (
  generationIds: string[],
  includeRecipe: boolean,
  operationId: string,
  retryToken: string | null = null,
) =>
  invoke<BatchExportResult | null>("export_generations", {
    generationIds,
    includeRecipe,
    operationId,
    retryToken,
  });
export const cancelExport = (operationId: string) =>
  invoke<void>("cancel_operation", { operationId });
