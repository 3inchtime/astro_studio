import { invoke } from "@tauri-apps/api/core";
import type { PromptExtraction } from "../../types";

export async function extractPromptFromImage(
  imagePath: string,
  configId: string,
  language: string,
): Promise<PromptExtraction> {
  return invoke("extract_prompt_from_image", {
    imagePath,
    configId,
    language,
  });
}

export async function getPromptExtractions(
  limit = 20,
): Promise<PromptExtraction[]> {
  return invoke("get_prompt_extractions", { limit });
}
