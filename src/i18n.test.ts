import { describe, expect, it } from "vitest";
import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import fr from "./locales/fr.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

const localeResources = {
  de,
  en,
  es,
  fr,
  ja,
  ko,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
} as const;

const providerSettingsKeys = [
  "settings.noApiKey",
  "settings.useProvider",
] as const;

describe("i18n resources", () => {
  it("defines every provider settings key used by the model settings panel", () => {
    for (const [locale, resources] of Object.entries(localeResources)) {
      for (const key of providerSettingsKeys) {
        expect(resources, `${locale} should define ${key}`).toHaveProperty(key);
      }
    }
  });
});
