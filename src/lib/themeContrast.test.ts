import { describe, expect, it } from "vitest";
import { THEME_CATALOG } from "./themes";
import { contrastRatio } from "./themeContrast";

describe("workspace theme readability", () => {
  for (const theme of THEME_CATALOG) {
    it(`${theme.id} keeps text and controls readable`, () => {
      const v = theme.variables;
      for (const surface of ["--color-background", "--color-surface", "--color-subtle", "--color-rail", "--color-canvas"]) {
        expect(contrastRatio(v["--color-muted"], v[surface])).toBeGreaterThanOrEqual(4.5);
        expect(contrastRatio(v["--color-primary"], v[surface])).toBeGreaterThanOrEqual(4.5);
        expect(contrastRatio(v["--color-control-border"], v[surface])).toBeGreaterThanOrEqual(3);
      }
      expect(contrastRatio(v["--color-primary"], v["--color-primary-foreground"])).toBeGreaterThanOrEqual(4.5);
    });
  }
});
