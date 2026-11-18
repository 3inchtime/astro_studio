import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime, listenDesktopEvent } from "../../lib/desktop";
import type { GenerationParams, GenerateResponse, ImageInputFidelity } from "../../types";

export type GenerationJobStatus = "queued" | "running" | "completed" | "failed" | "cancelled" | "interrupted";
export type GenerationJobStage = "migration_unknown" | "queued" | "preparing" | "provider_request" | "retry_backoff" | "response_ready" | "local_processing" | "startup_reconciliation" | "legacy_response_recovery" | "terminal";

export interface EnqueueGenerationResult {
  job_id: string;
  generation_id: string;
  conversation_id: string;
  status: GenerationJobStatus;
  error_code: string | null;
  error_message: string | null;
  queued_at: string;
  finished_at: string | null;
  cancel_requested_at: string | null;
  retryable: boolean;
}

export interface GenerationJobEvent extends EnqueueGenerationResult {
  client_request_id: string;
  project_id: string;
  stage: GenerationJobStage;
  source_kind: "generate" | "edit" | "canvas";
  source_ref: { id?: string; document_id?: string; project_id?: string; conversation_id?: string };
  queue_position: number | null;
  started_at: string | null;
}

export interface GenerationJob extends Omit<GenerationJobEvent, "job_id" | "queue_position" | "project_id" | "conversation_id"> {
  id: string;
  request: { prompt: string; model: string; conversation_id: string; project_id: string; kind: "generate" | "edit"; source_image_paths: string[] };
}
export interface GenerationJobFilter {
  project_id?: string;
  conversation_id?: string;
  document_id?: string;
  statuses?: GenerationJobStatus[];
  source_kind?: GenerationJobEvent["source_kind"];
  source_ref_id?: string;
  generation_id?: string;
  limit?: number;
  cursor?: string;
}

type EditGenerationParams = GenerationParams & { sourceImagePaths: string[]; inputFidelity?: ImageInputFidelity };
type OnEnqueued = (result: EnqueueGenerationResult) => void;

export async function enqueueGeneration(params: GenerationParams | EditGenerationParams): Promise<EnqueueGenerationResult> {
  if (!hasTauriRuntime()) throw new Error("Image generation is only available in the desktop app.");
  const kind = "sourceImagePaths" in params ? "edit" : "generate";
  return invoke("enqueue_generation", { request: { ...params, clientRequestId: params.clientRequestId ?? crypto.randomUUID(), kind } });
}

export function getGenerationJob(jobId: string): Promise<GenerationJob> {
  return invoke("get_generation_job", { jobId });
}

export function listGenerationJobs(filter?: GenerationJobFilter): Promise<{ items: GenerationJob[]; next_cursor: string | null }> {
  if (!hasTauriRuntime()) return Promise.resolve({ items: [], next_cursor: null });
  return invoke("list_generation_jobs", { filter });
}

export function cancelGenerationJob(jobId: string): Promise<GenerationJob> {
  return invoke("cancel_generation_job", { jobId });
}

export { recoverGeneration as retryGenerationSave } from "../../lib/recoveryApi";

export function onGenerationJob(handler: (event: GenerationJobEvent) => void) {
  return listenDesktopEvent("generation:job", handler);
}

async function submitGeneration(params: GenerationParams | EditGenerationParams, onEnqueued?: OnEnqueued): Promise<GenerateResponse> {
  const ack = await enqueueGeneration(params);
  onEnqueued?.(ack);
  return invoke("wait_generation_result", { jobId: ack.job_id });
}

export function generateImage(params: GenerationParams, onEnqueued?: OnEnqueued): Promise<GenerateResponse> {
  return submitGeneration(params, onEnqueued);
}

export function editImage(params: EditGenerationParams, onEnqueued?: OnEnqueued): Promise<GenerateResponse> {
  return submitGeneration(params, onEnqueued);
}

export function onGenerationProgress(
  handler: (data: { generation_id: string; status: string }) => void,
) {
  return listenDesktopEvent("generation:progress", handler);
}

export function onGenerationPartial(
  handler: (
    data: import("../../lib/generationMessages").GenerationPartialEvent,
  ) => void,
) {
  return listenDesktopEvent("generation:partial", handler);
}

export function onGenerationComplete(
  handler: (data: { generation_id: string; status: string; job_id?: string }) => void,
) {
  return listenDesktopEvent("generation:complete", handler);
}

export function onGenerationFailed(
  handler: (data: { generation_id: string; error: string }) => void,
) {
  return listenDesktopEvent("generation:failed", handler);
}
