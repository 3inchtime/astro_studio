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
  "settings.sections",
  "settings.imageGenerationConfig",
  "settings.promptOptimizationConfig",
  "settings.optimizationService",
  "settings.addOptimizationService",
] as const;

const canvasEditorKeys = [
  "canvas.copySelection",
  "canvas.pasteSelection",
  "canvas.deleteSelection",
  "canvas.bringForward",
  "canvas.sendBackward",
  "canvas.bringToFront",
  "canvas.sendToBack",
  "canvas.fitFrame",
  "canvas.fitSelection",
  "canvas.selectionCount",
  "canvas.zoomStatus",
] as const;

const canvasEditorLocalizedKeys = canvasEditorKeys.filter(
  (key) => key !== "canvas.zoomStatus",
);

const canvasEditorPlaceholders = {
  "canvas.selectionCount": "{{count}}",
  "canvas.zoomStatus": "{{zoom}}",
} as const;

const noEnglishFallbackKeys = [
  "sidebar.projects",
  "sidebar.allProjects",
  "sidebar.newProject",
  "sidebar.archived",
  "sidebar.rename",
  "sidebar.renameConversation",
  "sidebar.renameProject",
  "sidebar.pin",
  "sidebar.unpin",
  "sidebar.archive",
  "sidebar.unarchive",
  "sidebar.moveToProject",
  "sidebar.delete",
  "sidebar.deleteConversationConfirm",
  "sidebar.deleteProjectConfirm",
  "sidebar.conversationActions",
  "sidebar.projectActions",
  "projects.directory",
  "projects.title",
  "projects.recentConversations",
  "projects.newConversation",
  "projects.manage",
  "projects.imagesTitle",
  "projects.imagesEmptyTitle",
  "projects.imagesEmptyHint",
  "projects.emptyConversations",
  "projects.loading",
  "projects.loadError",
  "projects.emptyTitle",
  "projects.emptyHint",
  "projects.pinned",
  "projects.deleteConfirm",
  "projects.deleteConfirmAction",
  "projects.deleteCancel",
  "projects.actionError",
  "projects.deleteError",
  "projects.pin",
  "projects.unpin",
  "projects.notFound",
  "projects.conversations",
  "projects.renameConversationError",
  "projects.deleteConversationError",
  "projectDialog.createTitle",
  "projectDialog.renameTitle",
  "projectDialog.nameLabel",
  "projectDialog.createSubmit",
  "projectDialog.renameSubmit",
  "projectDialog.cancel",
  "projectDialog.nameRequired",
  "projectDialog.createError",
  "projectDialog.renameError",
] as const;

function getNestedValue(
  resources: Record<string, unknown>,
  keyPath: string,
): unknown {
  if (keyPath in resources) {
    return resources[keyPath];
  }

  return keyPath.split(".").reduce<unknown>((value, segment) => {
    if (value && typeof value === "object" && segment in value) {
      return (value as Record<string, unknown>)[segment];
    }

    return undefined;
  }, resources);
}

function flattenKeys(
  resources: Record<string, unknown>,
  prefix = "",
): string[] {
  return Object.entries(resources).flatMap(([key, value]) => {
    const keyPath = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return flattenKeys(value as Record<string, unknown>, keyPath);
    }

    return keyPath;
  });
}

describe("i18n resources", () => {
  it("keeps every locale in parity with the English and Simplified Chinese resource keys", () => {
    const requiredKeys = new Set([
      ...flattenKeys(en),
      ...flattenKeys(zhCN),
    ]);

    for (const [locale, resources] of Object.entries(localeResources)) {
      const localeKeys = new Set(flattenKeys(resources));
      const missingKeys = [...requiredKeys].filter((key) => !localeKeys.has(key));

      expect(
        missingKeys,
        `${locale} is missing locale keys`,
      ).toEqual([]);
    }
  });

  it("defines every provider settings key used by the model settings panel", () => {
    for (const [locale, resources] of Object.entries(localeResources)) {
      for (const key of providerSettingsKeys) {
        expect(resources, `${locale} should define ${key}`).toHaveProperty(key);
      }
    }
  });

  it("defines every canvas editor key and preserves its interpolation placeholders", () => {
    for (const [locale, resources] of Object.entries(localeResources)) {
      for (const key of canvasEditorKeys) {
        const value = getNestedValue(resources, key);

        expect(value, `${locale} should define ${key}`).toBeTypeOf("string");
        expect(value, `${locale} should not leave ${key} empty`).not.toBe("");
      }

      for (const [key, placeholder] of Object.entries(canvasEditorPlaceholders)) {
        expect(
          getNestedValue(resources, key),
          `${locale} should preserve ${placeholder} in ${key}`,
        ).toContain(placeholder);
      }
    }
  });

  it("localizes canvas editor labels in every non-English locale", () => {
    const nonEnglishLocales = Object.entries(localeResources).filter(
      ([locale]) => locale !== "en",
    );

    for (const [locale, resources] of nonEnglishLocales) {
      for (const key of canvasEditorLocalizedKeys) {
        expect(
          getNestedValue(resources, key),
          `${locale} should translate ${key} instead of reusing English`,
        ).not.toBe(getNestedValue(en, key));
      }
    }
  });

  it("does not leave project and sidebar translations as English fallbacks in non-English locales", () => {
    const nonEnglishLocales = Object.entries(localeResources).filter(
      ([locale]) => locale !== "en",
    );

    for (const [locale, resources] of nonEnglishLocales) {
      for (const key of noEnglishFallbackKeys) {
        expect(
          resources,
          `${locale} should define ${key}`,
        ).toHaveProperty(key);
        const localizedValue = getNestedValue(resources, key);
        const englishValue = getNestedValue(en, key);

        expect(
          localizedValue,
          `${locale} should translate ${key} instead of reusing English`,
        ).not.toBe(englishValue);
      }
    }
  });
});
