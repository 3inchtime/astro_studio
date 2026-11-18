import manifest from "../../shared/model-catalog.v1.json";
import type {
  ImageModelCatalogEntry, ModelParameterCapabilities, ModelParameterDefaults,
} from "../types";

type ModelDefinition = {
  label: string;
  i18nKey: string;
  profile: string;
  providerModelId?: string;
  aliases?: string[];
};

export function createImageCatalog<Id extends string>(
  models: Record<Id, ModelDefinition>,
) {
  if (manifest.schemaVersion !== 1) throw new Error("Unsupported model catalog version");
  type Entry = Omit<ImageModelCatalogEntry, "id"> & { id: Id };
  const byName = new Map<string, Entry>();
  const entries = (Object.keys(models) as Id[]).map((id) => {
    const model = models[id];
    const profile = manifest.imageProfiles[model.profile as keyof typeof manifest.imageProfiles];
    if (!profile) throw new Error(`Missing capability profile for ${id}`);
    const protocol = profile.protocol as keyof typeof manifest.imageProtocols;
    const { provider, baseUrl, generationPath, editPath } = manifest.imageProtocols[protocol];
    const providerModelId = model.providerModelId ?? id;
    const endpoint = (path: string) => `${baseUrl}/${path.replace("{model}", providerModelId)}`;
    const entry: Entry = {
      id, label: model.label, i18nKey: model.i18nKey,
      providerModelId, provider, protocol,
      supportsEdit: profile.supportsEdit,
      advancedOptions: profile.advancedOptions,
      maxReferenceImages: profile.maxReferenceImages,
      connectionDefaults: { baseUrl, generationUrl: endpoint(generationPath), editUrl: endpoint(editPath) },
      parameterDefaults: profile.parameterDefaults as ModelParameterDefaults,
      parameterCapabilities: profile.parameterCapabilities as ModelParameterCapabilities,
    };
    for (const name of new Set([id, providerModelId, ...(model.aliases ?? [])])) {
      if (byName.has(name)) throw new Error(`Duplicate model catalog name: ${name}`);
      byName.set(name, entry);
    }
    return entry;
  });
  return { entries, find: (name: string) => byName.get(name) };
}

const catalog = createImageCatalog(manifest.imageModels);
export const IMAGE_MODEL_CATALOG = catalog.entries;
export const findImageModel = catalog.find;
export const DEFAULT_IMAGE_MODEL = getImageModelCatalogEntry(manifest.defaultImageModel).id;

export function supportsAdvancedImageOptions(model: string): boolean {
  return findImageModel(model)?.advancedOptions ?? false;
}

export function getImageModelCatalogEntry(model: string): ImageModelCatalogEntry {
  const entry = findImageModel(model);
  if (entry) return entry;
  console.error(new Error(`Unknown image model "${model}" was requested from the frontend model catalog.`));
  return findImageModel("gpt-image-2")!;
}
