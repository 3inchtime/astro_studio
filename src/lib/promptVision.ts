export function resolveVisualSourcePaths(sources: string[], selection: string[] | null): string[] {
  const unique = [...new Set(sources)];
  return selection === null
    ? unique.length <= 3 ? unique : []
    : [...new Set(selection)].filter((path) => unique.includes(path));
}

export function promptVisionErrorKey(error: unknown): string {
  const detail = error && typeof error === "object" && "detail" in error ? error.detail : null;
  const message = typeof error === "string" ? error
    : detail && typeof detail === "object" && "message" in detail ? detail.message
    : error instanceof Error ? error.message : "";
  const keys: Record<string, string> = {
    prompt_vision_select_up_to_three: "promptVision.selectUpToThree",
    prompt_vision_invalid_selection: "promptVision.invalidSelection",
    prompt_vision_multimodal_required: "promptVision.multimodalRequired",
    prompt_vision_image_unavailable: "promptVision.imageUnavailable",
    prompt_vision_analysis_failed: "promptVision.analysisFailed",
  };
  return typeof message === "string" ? keys[message] ?? "promptVision.analysisFailed" : "promptVision.analysisFailed";
}
