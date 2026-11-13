import i18n from "../i18n";
import en from "../locales/recovery.en.json";
import zhCN from "../locales/recovery.zh-CN.json";
import zhTW from "../locales/recovery.zh-TW.json";
import de from "../locales/recovery.de.json";
import es from "../locales/recovery.es.json";
import fr from "../locales/recovery.fr.json";
import ja from "../locales/recovery.ja.json";
import ko from "../locales/recovery.ko.json";
for (const [language, recovery] of Object.entries({
  en,
  de,
  es,
  fr,
  ja,
  ko,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
})) {
  i18n.addResourceBundle(language, "translation", { recovery }, true, true);
}
