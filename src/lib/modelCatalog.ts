import type {
  ImageModel,
  ImageModelCatalogEntry,
} from "../types";

const OPENAI_BASE_URL = "https://api.openai.com/v1";
const OPENAI_GENERATION_URL = "https://api.openai.com/v1/images/generations";
const OPENAI_EDIT_URL = "https://api.openai.com/v1/images/edits";
const GEMINI_BASE_URL = "https://generativelanguage.googleapis.com/v1beta/models";
const GEMINI_NANO_BANANA_MODEL_ID = "gemini-2.5-flash-image";
const GEMINI_NANO_BANANA_2_MODEL_ID = "gemini-3.1-flash-image-preview";
const GEMINI_NANO_BANANA_PRO_MODEL_ID = "gemini-3-pro-image-preview";

const GPT_IMAGE_2_ENTRY: ImageModelCatalogEntry = {
  id: "gpt-image-2",
  label: "GPT Image 2",
  i18nKey: "models.gptImage2",
  provider: "openai",
  providerModelId: "gpt-image-2",
  supportsEdit: true,
  connectionDefaults: {
    baseUrl: OPENAI_BASE_URL,
    generationUrl: OPENAI_GENERATION_URL,
    editUrl: OPENAI_EDIT_URL,
  },
  parameterDefaults: {
    size: "auto",
    quality: "auto",
    background: "auto",
    outputFormat: "png",
    moderation: "auto",
    inputFidelity: "high",
    imageCount: 1,
  },
  parameterCapabilities: {
    sizes: [
      "auto",
      "1024x1024",
      "1536x1024",
      "1024x1536",
      "2048x2048",
      "2048x1152",
      "3840x2160",
      "2160x3840",
    ],
    qualities: ["auto", "high", "medium", "low"],
    backgrounds: ["auto", "opaque", "transparent"],
    outputFormats: ["png", "jpeg", "webp"],
    moderationLevels: ["auto"],
    inputFidelityOptions: ["high"],
    imageCounts: [1, 2, 3, 4],
  },
};

function createGeminiModelEntry(
  model: Exclude<ImageModel, "gpt-image-2">,
  label: string,
  i18nKey: string,
  providerModelId: string,
): ImageModelCatalogEntry {
  const generationUrl = `${GEMINI_BASE_URL}/${providerModelId}:generateContent`;

  return {
    id: model,
    label,
    i18nKey,
    provider: "google",
    providerModelId,
    supportsEdit: true,
    connectionDefaults: {
      baseUrl: GEMINI_BASE_URL,
      generationUrl,
      editUrl: generationUrl,
    },
    parameterDefaults: {
      size: "1024x1024",
      quality: "auto",
      background: "auto",
      outputFormat: "png",
      moderation: "auto",
      inputFidelity: "high",
      imageCount: 1,
    },
    parameterCapabilities: {
      sizes: ["1024x1024", "1536x1024", "1024x1536"],
      qualities: ["auto"],
      backgrounds: ["auto"],
      outputFormats: ["png"],
      moderationLevels: ["auto"],
      inputFidelityOptions: ["high"],
      imageCounts: [1, 2, 3, 4],
    },
  };
}

export const IMAGE_MODEL_CATALOG: ImageModelCatalogEntry[] = [
  GPT_IMAGE_2_ENTRY,
  createGeminiModelEntry(
    "nano-banana",
    "Nano Banana",
    "models.nanoBanana",
    GEMINI_NANO_BANANA_MODEL_ID,
  ),
  createGeminiModelEntry(
    "nano-banana-2",
    "Nano Banana 2",
    "models.nanoBanana2",
    GEMINI_NANO_BANANA_2_MODEL_ID,
  ),
  createGeminiModelEntry(
    "nano-banana-pro",
    "Nano Banana Pro",
    "models.nanoBananaPro",
    GEMINI_NANO_BANANA_PRO_MODEL_ID,
  ),
];

const IMAGE_MODEL_CATALOG_BY_ID = new Map(
  IMAGE_MODEL_CATALOG.map((entry) => [entry.id, entry] as const),
);

function reportUnknownImageModel(model: string): void {
  console.error(
    new Error(
      `Unknown image model "${model}" was requested from the frontend model catalog.`,
    ),
  );
}

export function getImageModelCatalogEntry(model: string): ImageModelCatalogEntry {
  const entry = IMAGE_MODEL_CATALOG_BY_ID.get(model as ImageModel);

  if (entry) {
    return entry;
  }

  reportUnknownImageModel(model);
  return GPT_IMAGE_2_ENTRY;
}
