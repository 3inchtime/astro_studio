import exportzhTW from "./locales/export.zh-TW.json";
import exportzhCN from "./locales/export.zh-CN.json";
import exportko from "./locales/export.ko.json";
import exportja from "./locales/export.ja.json";
import exportfr from "./locales/export.fr.json";
import exportes from "./locales/export.es.json";
import exporten from "./locales/export.en.json";
import exportde from "./locales/export.de.json";
import backupko from "./locales/backup.ko.json";
import backupja from "./locales/backup.ja.json";
import backupfr from "./locales/backup.fr.json";
import backupes from "./locales/backup.es.json";
import backupde from "./locales/backup.de.json";
import backupzhTW from "./locales/backup.zh-TW.json";
import backupzhCN from "./locales/backup.zh-CN.json";
import backupen from "./locales/backup.en.json";
import diagnosticsde from "./locales/diagnostics.de.json";
import diagnosticsen from "./locales/diagnostics.en.json";
import diagnosticses from "./locales/diagnostics.es.json";
import diagnosticsfr from "./locales/diagnostics.fr.json";
import diagnosticsja from "./locales/diagnostics.ja.json";
import diagnosticsko from "./locales/diagnostics.ko.json";
import diagnosticszhCN from "./locales/diagnostics.zh-CN.json";
import diagnosticszhTW from "./locales/diagnostics.zh-TW.json";
import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import de from "./locales/de.json";
import en from "./locales/en.json";
import es from "./locales/es.json";
import fr from "./locales/fr.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import { LANGUAGE_CODES, normalizeLanguage } from "./lib/languages";

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      de: { translation: { ...de, diagnostics: diagnosticsde, backup: backupde, export: exportde } },
      en: { translation: { ...en, diagnostics: diagnosticsen, backup: backupen, export: exporten } },
      es: { translation: { ...es, diagnostics: diagnosticses, backup: backupes, export: exportes } },
      fr: { translation: { ...fr, diagnostics: diagnosticsfr, backup: backupfr, export: exportfr } },
      ja: { translation: { ...ja, diagnostics: diagnosticsja, backup: backupja, export: exportja } },
      ko: { translation: { ...ko, diagnostics: diagnosticsko, backup: backupko, export: exportko } },
      "zh-CN": { translation: { ...zhCN, diagnostics: diagnosticszhCN, backup: backupzhCN, export: exportzhCN } },
      "zh-TW": { translation: { ...zhTW, diagnostics: diagnosticszhTW, backup: backupzhTW, export: exportzhTW } },
    },
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
