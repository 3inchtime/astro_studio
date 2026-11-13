import { afterEach, expect, it } from "vitest";
import i18n from "./i18n";

afterEach(async () => { await i18n.changeLanguage("en"); });
it("keeps the document language aligned with the selected interface language", async () => {
  await i18n.changeLanguage("de");
  expect(document.documentElement.lang).toBe("de");
  await i18n.changeLanguage("zh-TW");
  expect(document.documentElement.lang).toBe("zh-TW");
});
