import { describe, expect, it } from "vitest";
import { LANGUAGE_CODES, normalizeLanguage } from "./languages";

describe("language helpers", () => {
  it("normalizes regional browser language codes to supported locales", () => {
    expect(normalizeLanguage("en-US")).toBe("en");
    expect(normalizeLanguage("es-MX")).toBe("es");
    expect(normalizeLanguage("fr_CA")).toBe("fr");
    expect(normalizeLanguage("zh-Hans-CN")).toBe("zh-CN");
    expect(normalizeLanguage("zh-Hant-TW")).toBe("zh-TW");
    expect(normalizeLanguage("zh-HK")).toBe("zh-TW");
  });

  it("falls back to English for unsupported or empty languages", () => {
    expect(normalizeLanguage("pt-BR")).toBe("en");
    expect(normalizeLanguage("")).toBe("en");
    expect(normalizeLanguage(null)).toBe("en");
  });

  it("registers every selectable language code once", () => {
    expect(new Set(LANGUAGE_CODES).size).toBe(LANGUAGE_CODES.length);
    expect(LANGUAGE_CODES).toEqual([
      "en",
      "zh-CN",
      "zh-TW",
      "ja",
      "ko",
      "es",
      "fr",
      "de",
    ]);
  });
});
