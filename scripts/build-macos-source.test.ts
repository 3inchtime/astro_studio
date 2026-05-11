import { describe, expect, it } from "vitest";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(__dirname, "..");

describe("macOS source build script", () => {
  it("is executable and builds a runnable macOS bundle from source", () => {
    const scriptPath = resolve(repoRoot, "scripts/build-macos-source.sh");
    const script = readFileSync(scriptPath, "utf8");
    const mode = statSync(scriptPath).mode;

    expect(mode & 0o111).toBeGreaterThan(0);
    expect(script).toContain('$(uname -s)');
    expect(script).toContain('"Darwin"');
    expect(script).toContain("install_with_brew_if_missing npm node");
    expect(script).toContain("install_with_brew_if_missing cargo rust");
    expect(script).toContain("brew install \"$formula\"");
    expect(script).toContain("Homebrew is required");
    expect(script).toContain("npm install");
    expect(script).toContain("cargo fetch --manifest-path src-tauri/Cargo.toml");
    expect(script).toContain("npm run tauri build");
    expect(script).toContain("src-tauri/target/release/bundle/macos");
    expect(script).toContain("src-tauri/target/release/bundle/dmg");
  });
});
