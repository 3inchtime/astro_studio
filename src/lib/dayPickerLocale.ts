import type { DayPickerLocale } from "react-day-picker";
import { de, enUS, es, fr, ja, ko, zhCN, zhTW } from "react-day-picker/locale";
import { normalizeLanguage, type SupportedLanguage } from "./languages";

const dayPickerLocales: Record<SupportedLanguage, DayPickerLocale> = {
  en: enUS,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  ja,
  ko,
  es,
  fr,
  de,
};

export function getDayPickerLocale(language?: string | null): DayPickerLocale {
  return dayPickerLocales[normalizeLanguage(language)];
}
