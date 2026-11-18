import { afterEach, expect, it } from "vitest";
import i18n from "./i18n";

afterEach(async () => { await i18n.changeLanguage("en"); });
it("keeps the document language aligned with the selected interface language", async () => {
  await i18n.changeLanguage("de");
  expect(document.documentElement.lang).toBe("de");
  await i18n.changeLanguage("zh-TW");
  expect(document.documentElement.lang).toBe("zh-TW");
});

it("loads the full selected language when the recovery notice is available", async () => {
  await import("./components/generate/RecoveryNotice");
  await i18n.changeLanguage("fr");
  expect(i18n.t("nav.gallery")).toBe("Galerie");
  expect(i18n.t("recovery.title")).toBe("Examiner les résultats inachevés");
});
