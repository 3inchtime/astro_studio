import type { ImageModel, ImageModelCatalogEntry } from "../types";

const OPENAI_BASE_URL = "https://api.openai.com/v1";
const OPENAI_GENERATION_URL = "https://api.openai.com/v1/images/generations";
const OPENAI_EDIT_URL = "https://api.openai.com/v1/images/edits";
const GEMINI_BASE_URL =
  "https://generativelanguage.googleapis.com/v1beta/models";
const GEMINI_NANO_BANANA_MODEL_ID = "gemini-2.5-flash-image";
const GEMINI_NANO_BANANA_2_MODEL_ID = "gemini-3.1-flash-image";
const GEMINI_NANO_BANANA_PRO_MODEL_ID = "gemini-3-pro-image";

const GPT_IMAGE_2_ENTRY: ImageModelCatalogEntry = {
  id: "gpt-image-2",
  label: "GPT Image 2",
  i18nKey: "models.gptImage2",
  provider: "openai",
  providerModelId: "gpt-image-2",
  protocol: "openai-images",
  supportsEdit: true,
  maxReferenceImages: 16,
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
    protocol: "gemini-generate-content",
    supportsEdit: true,
    maxReferenceImages: 14,
    connectionDefaults: {
      baseUrl: GEMINI_BASE_URL,
      generationUrl,
      editUrl: generationUrl,
    },
    parameterDefaults: {
      aspectRatio: "1:1",
      resolution: "1K",
      size: "1024x1024",
      quality: "auto",
      background: "auto",
      outputFormat: "png",
      moderation: "auto",
      inputFidelity: "high",
      imageCount: 1,
    },
    parameterCapabilities: {
      aspectRatios: [
        "1:1",
        "2:3",
        "3:2",
        "3:4",
        "4:3",
        "4:5",
        "5:4",
        "9:16",
        "16:9",
        "21:9",
      ],
      resolutions:
        model === "nano-banana"
          ? ["1K"]
          : model === "nano-banana-2"
            ? ["512", "1K", "2K", "4K"]
            : ["1K", "2K", "4K"],
      sizes: ["1024x1024", "1536x1024", "1024x1536"],
      qualities: ["auto"],
      backgrounds: ["auto"],
      outputFormats: ["png"],
      moderationLevels: ["auto"],
      inputFidelityOptions: ["high"],
      imageCounts: [1],
    },
  };
}

function createOpenAi25Entry(
  id: "gpt-image-2.5-sunburst" | "gpt-image-2.5-flare",
  label: string,
  i18nKey: string,
): ImageModelCatalogEntry {
  return {
    ...GPT_IMAGE_2_ENTRY,
    id,
    label,
    i18nKey,
    providerModelId: id,
    parameterCapabilities: {
      ...GPT_IMAGE_2_ENTRY.parameterCapabilities,
      qualities: ["auto", "low", "medium", "high", "xhigh", "max"],
    },
  };
}

function createNativeEntry(
  id: ImageModel,
  label: string,
  i18nKey: string,
  provider: string,
  baseUrl: string,
): ImageModelCatalogEntry {
  const isGrok = provider === "xai";
  const generationUrl = `${baseUrl}/images/generations`;
  return {
    id,
    label,
    i18nKey,
    provider,
    providerModelId: id,
    protocol: isGrok ? "xai-images" : "seedream-images",
    supportsEdit: true,
    maxReferenceImages: isGrok ? 5 : 10,
    connectionDefaults: {
      baseUrl,
      generationUrl,
      editUrl: isGrok ? `${baseUrl}/images/edits` : generationUrl,
    },
    parameterDefaults: {
      ...GPT_IMAGE_2_ENTRY.parameterDefaults,
      resolution: isGrok ? "1K" : "2K",
      ...(isGrok ? { aspectRatio: "1:1" } : {}),
      outputFormat: isGrok ? "jpeg" : "png",
    },
    parameterCapabilities: {
      sizes: ["auto"],
      qualities: isGrok ? ["auto", "low", "medium"] : ["auto"],
      backgrounds: ["auto"],
      outputFormats: isGrok ? ["jpeg"] : ["png", "jpeg"],
      moderationLevels: ["auto"],
      inputFidelityOptions: ["high"],
      imageCounts: isGrok ? [1, 2, 3, 4] : [1],
      resolutions: isGrok ? ["1K", "2K"] : ["auto", "1K", "1.5K", "2K"],
      ...(isGrok
        ? {
            aspectRatios: [
              "1:1",
              "2:1",
              "1:2",
              "2:3",
              "3:2",
              "3:4",
              "4:3",
              "9:16",
              "16:9",
              "19.5:9",
              "9:19.5",
              "20:9",
              "9:20",
            ],
          }
        : {}),
    },
  };
}

const NANO_BANANA_21_ENTRY = createGeminiModelEntry(
  "gemini-nano-banana-2.1",
  "Nano Banana 2.1",
  "models.nanoBanana21",
  "gemini-nano-banana-2.1",
);
NANO_BANANA_21_ENTRY.protocol = "gemini-interactions";
NANO_BANANA_21_ENTRY.connectionDefaults = {
  baseUrl: "https://generativelanguage.googleapis.com/v1beta",
  generationUrl:
    "https://generativelanguage.googleapis.com/v1beta/interactions",
  editUrl: "https://generativelanguage.googleapis.com/v1beta/interactions",
};
NANO_BANANA_21_ENTRY.parameterCapabilities.aspectRatios = [
  "1:1",
  "1:2",
  "2:1",
  "2:3",
  "3:2",
  "3:4",
  "4:3",
  "4:5",
  "5:4",
  "9:16",
  "16:9",
  "21:9",
  "1:4",
  "4:1",
  "1:8",
  "8:1",
];
NANO_BANANA_21_ENTRY.parameterCapabilities.outputFormats = ["png", "jpeg"];

export const IMAGE_MODEL_CATALOG: ImageModelCatalogEntry[] = [
  createOpenAi25Entry(
    "gpt-image-2.5-flare",
    "GPT Image 2.5 Flare",
    "models.gptImageFlare",
  ),
  createOpenAi25Entry(
    "gpt-image-2.5-sunburst",
    "GPT Image 2.5 Sunburst",
    "models.gptImageSunburst",
  ),
  GPT_IMAGE_2_ENTRY,
  NANO_BANANA_21_ENTRY,
  createNativeEntry(
    "grok-imagine-image-2.0",
    "Grok Imagine 2.0",
    "models.grokImagine",
    "xai",
    "https://api.x.ai/v1",
  ),
  createNativeEntry(
    "doubao-seedream-5-0-pro-260628",
    "Seedream 5.0 Pro",
    "models.seedreamPro",
    "volcengine",
    "https://ark.cn-beijing.volces.com/api/v3",
  ),
  createNativeEntry(
    "doubao-seedream-5-0-flash-260915",
    "Seedream 5.0 Flash",
    "models.seedreamFlash",
    "volcengine",
    "https://ark.cn-beijing.volces.com/api/v3",
  ),
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

export function getImageModelCatalogEntry(
  model: string,
): ImageModelCatalogEntry {
  const entry = IMAGE_MODEL_CATALOG_BY_ID.get(model as ImageModel);

  if (entry) {
    return entry;
  }

  reportUnknownImageModel(model);
  return GPT_IMAGE_2_ENTRY;
}
