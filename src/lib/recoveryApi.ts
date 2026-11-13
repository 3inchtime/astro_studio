import { invoke } from "@tauri-apps/api/core";

export interface RecoveryEntry {
  generation_id: string;
  conversation_id: string;
  prompt: string;
  status: string;
  can_recover: boolean;
  result_unknown: boolean;
}
export interface RecoverySummary {
  recovered: number;
  interrupted: number;
  failed: number;
  cancelled: number;
  pending: number;
  entries: RecoveryEntry[];
}
export const getGenerationRecoverySummary = () =>
  invoke<RecoverySummary>("get_generation_recovery_summary");
export const recoverGeneration = (generationId: string) =>
  invoke<RecoverySummary>("recover_generation", { generationId });
