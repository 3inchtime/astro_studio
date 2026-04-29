export const LANGUAGE_OPTIONS = [
  { code: "en", label: "English" },
  { code: "zh-CN", label: "简体中文" },
  { code: "zh-TW", label: "繁體中文" },
  { code: "ja", label: "日本語" },
  { code: "ko", label: "한국어" },
  { code: "es", label: "Español" },
  { code: "fr", label: "Français" },
  { code: "de", label: "Deutsch" },
] as const;

export type SupportedLanguage = typeof LANGUAGE_OPTIONS[number]["code"];

export const LANGUAGE_CODES = LANGUAGE_OPTIONS.map(({ code }) => code);

const SUPPORTED_LANGUAGE_SET = new Set<string>(LANGUAGE_CODES);

export function normalizeLanguage(language?: string | null): SupportedLanguage {
  const normalized = language?.trim().replace("_", "-").toLowerCase() ?? "";

  if (!normalized) {
    return "en";
  }

  if (
    normalized === "zh-tw" ||
    normalized === "zh-hk" ||
    normalized === "zh-mo" ||
    normalized.includes("hant")
  ) {
    return "zh-TW";
  }

  if (normalized.startsWith("zh")) {
    return "zh-CN";
  }

  const primaryLanguage = normalized.split("-")[0];

  if (SUPPORTED_LANGUAGE_SET.has(primaryLanguage)) {
    return primaryLanguage as SupportedLanguage;
  }

  return "en";
}
