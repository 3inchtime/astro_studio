import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(__dirname, "..");

type PackageJson = {
  scripts?: Record<string, string>;
};

type TauriConfig = {
  bundle?: {
    createUpdaterArtifacts?: boolean;
  };
};

const packageJson = JSON.parse(
  readFileSync(resolve(repoRoot, "package.json"), "utf8"),
) as PackageJson;

const tauriConfig = JSON.parse(
  readFileSync(resolve(repoRoot, "src-tauri/tauri.conf.json"), "utf8"),
) as TauriConfig;

describe("release build signing configuration", () => {
  it("keeps regular local Tauri builds from requiring updater signing keys", () => {
    expect(tauriConfig.bundle?.createUpdaterArtifacts).toBe(false);
  });

  it("keeps updater artifact generation behind an explicit signed Windows release script", () => {
    const releaseScript = packageJson.scripts?.["tauri:build:release:windows-updater"];

    expect(releaseScript).toBeDefined();
    expect(releaseScript).toContain("tauri build");
    expect(releaseScript).toContain("--bundles nsis");
    expect(releaseScript).toContain("createUpdaterArtifacts");
    expect(releaseScript).toContain("true");
    expect(releaseScript).not.toContain("'");
  });
});
