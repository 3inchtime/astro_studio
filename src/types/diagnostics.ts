import type { ImageModel } from "./index";
export interface DiagnosticCheck {
  status: "passed" | "failed" | "unverified";
  code: string;
}
export interface ProviderDiagnosticResult {
  local: DiagnosticCheck;
  transport: DiagnosticCheck;
  authentication: DiagnosticCheck;
  model: DiagnosticCheck;
  generation: DiagnosticCheck;
}
export interface ProviderSmokePlan {
  model: ImageModel;
  provider_id: string;
  provider_name: string;
  endpoint_host: string;
  fingerprint: string;
  prompt: string;
  image_count: number;
  size: string | null;
  quality: string | null;
  resolution: string | null;
  aspect_ratio: string | null;
  output_format: string;
}
