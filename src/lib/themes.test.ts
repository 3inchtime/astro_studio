import { describe, expect, it } from "vitest";
import {
  DEFAULT_THEME_ID,
  THEME_CATALOG,
  getThemeCatalogEntry,
  getThemeDescription,
  getThemeName,
  resolveThemeId,
} from "./themes";

describe("theme localization", () => {
  it("resolves localized theme labels through translation keys", () => {
    const theme = getThemeCatalogEntry("ocean-depths");
    const translations: Record<string, string> = {
      "themes.ocean-depths.name": "深海之境",
      "themes.ocean-depths.description": "冷静、可信的海洋商务风格。",
    };
    const t = (key: string) => translations[key] ?? key;

    expect(getThemeName(theme, t)).toBe("深海之境");
    expect(getThemeDescription(theme, t)).toBe("冷静、可信的海洋商务风格。");
  });

  it("includes the new pure light and pure black presets and defaults to pure light", () => {
    expect(THEME_CATALOG.map((theme) => theme.id)).toContain("pure-light");
    expect(THEME_CATALOG.map((theme) => theme.id)).toContain("pure-black");
    expect(THEME_CATALOG).toHaveLength(12);
    expect(DEFAULT_THEME_ID).toBe("pure-light");
    expect(resolveThemeId(null)).toBe("pure-light");
  });
});
