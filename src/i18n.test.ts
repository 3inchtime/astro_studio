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

describe("i18n resources", () => {
  it("defines every provider settings key used by the model settings panel", () => {
    for (const [locale, resources] of Object.entries(localeResources)) {
      for (const key of providerSettingsKeys) {
        expect(resources, `${locale} should define ${key}`).toHaveProperty(key);
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
