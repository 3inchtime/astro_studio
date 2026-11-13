import { invoke } from "@tauri-apps/api/core";
import type { ImageModel, GenerateResponse } from "../types";
import type {
  ProviderDiagnosticResult,
  ProviderSmokePlan,
} from "../types/diagnostics";
export const diagnoseImageProvider = (model: ImageModel, providerId: string) =>
  invoke<ProviderDiagnosticResult>("diagnose_image_provider", {
    model,
    providerId,
  });
export const getProviderSmokePlan = (model: ImageModel, providerId: string) =>
  invoke<ProviderSmokePlan>("get_provider_smoke_plan", { model, providerId });
export const runProviderSmokeTest = (
  model: ImageModel,
  providerId: string,
  fingerprint: string,
  clientRequestId: string,
) =>
  invoke<GenerateResponse>("run_provider_smoke_test", {
    model,
    providerId,
    fingerprint,
    clientRequestId,
  });
