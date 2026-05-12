import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(resolve(process.cwd(), "src/styles/globals.css"), "utf8");

describe("global Studio OS design utilities", () => {
  it("defines reusable control, panel, and focus utilities", () => {
    expect(css).toContain("@utility focus-ring");
    expect(css).toContain("@utility studio-panel");
    expect(css).toContain("@utility studio-control");
    expect(css).toContain("@utility studio-control-primary");
    expect(css).toContain("@utility studio-toolbar");
    expect(css).toContain("@utility studio-card");
  });

  it("removes decorative motion for reduced-motion users", () => {
    expect(css).toContain("@media (prefers-reduced-motion: reduce)");
    expect(css).toContain("animation-duration: 0.01ms");
  });
});
