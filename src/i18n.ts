import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { LANGUAGE_CODES, normalizeLanguage } from "./lib/languages";

import en from "./locales/en.json";
import backup from "./locales/backup.en.json";
import credentials from "./locales/credentials.en.json";
import diagnostics from "./locales/diagnostics.en.json";
import recovery from "./locales/recovery.en.json";
import exportStrings from "./locales/export.en.json";
import nodes from "./locales/nodes.en.json";
import type { BackendModule } from "i18next";

const localeFiles = import.meta.glob<{ default: Record<string, unknown> }>(["./locales/*.json", "!./locales/en.json", "!./locales/*.en.json"]);

async function loadTranslations(language: string) {
  const load = async (domain?: string) => {
    const name = domain ? `${domain}.${language}` : language;
    const file = localeFiles[`./locales/${name}.json`];
    if (!file) throw new Error(`Missing translations for ${name}`);
    return (await file()).default;
  };
  const [base, backup, credentials, diagnostics, exportStrings, recovery, nodes] = await Promise.all([
    load(), load("backup"), load("credentials"), load("diagnostics"), load("export"), load("recovery"), load("nodes"),
  ]);
  return { ...base, backup, credentials, diagnostics, recovery, export: exportStrings, nodes };
}

const bundledLocales: BackendModule = {
  type: "backend",
  init() {},
  read(language, _namespace, done) {
    loadTranslations(language).then((strings) => done(null, strings), (error) => done(error, false));
  },
};

i18n
  .use(bundledLocales)
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: { en: { translation: { ...en, backup, credentials, diagnostics, recovery, export: exportStrings, nodes } } },
    partialBundledLanguages: true,
    load: "currentOnly",
    fallbackLng: "en",
    supportedLngs: [...LANGUAGE_CODES],
    interpolation: { escapeValue: false },
    detection: {
      order: ["localStorage", "navigator"],
      lookupLocalStorage: "i18nextLng",
      caches: ["localStorage"],
      convertDetectedLanguage: normalizeLanguage,
    },
  });

const syncDocumentLanguage = (language: string) => {
  document.documentElement.lang = normalizeLanguage(language);
};
i18n.on("languageChanged", syncDocumentLanguage);
syncDocumentLanguage(i18n.resolvedLanguage ?? i18n.language ?? "en");

export default i18n;
