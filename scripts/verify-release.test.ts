import { afterEach, describe, expect, it } from "vitest";
import { createHash, generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { verifyRelease, verifyNativeEvidence } from "./verify-release.mjs";

const directories: string[] = [];
afterEach(() => directories.splice(0).forEach((dir) => rmSync(dir, { recursive: true, force: true })));

function fixture() {
  const dir = mkdtempSync(join(tmpdir(), "astro-release-"));
  directories.push(dir);
  const { publicKey, privateKey } = generateKeyPairSync("ed25519");
  const keyId = Buffer.from("0102030405060708", "hex");
  const key = Buffer.concat([Buffer.from("Ed"), keyId, publicKey.export({ format: "der", type: "spki" }).subarray(-32)]);
  const pubkey = Buffer.from(`untrusted comment: test key\n${key.toString("base64")}\n`).toString("base64");
  function addInstaller(name: string, bytes = Buffer.from(`signed installer ${name}`)) {
    const sig = sign(null, createHash("blake2b512").update(bytes).digest(), privateKey);
    const comment = `timestamp:123\tfile:${name}\tprehashed`;
    const global = sign(null, Buffer.concat([sig, Buffer.from(comment)]), privateKey);
    const signature = Buffer.from(`untrusted comment: test\n${Buffer.concat([Buffer.from("ED"), keyId, sig]).toString("base64")}\ntrusted comment: ${comment}\n${global.toString("base64")}\n`).toString("base64");
    writeFileSync(join(dir, name), bytes);
    writeFileSync(join(dir, `${name}.sig`), signature);
    return { signature, url: `https://github.com/3inchtime/astro_studio/releases/download/v0.0.35/${name}` };
  }
  const name = "Astro_0.0.35_x64-setup.exe";
  const manifest: { version: string; platforms: Record<string, {signature: string; url: string}> } = {
    version: "0.0.35", platforms: { "windows-x86_64": addInstaller(name) },
  };
  writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
  const options = { directory: dir, tag: "v0.0.35", repository: "3inchtime/astro_studio", pubkey };
  return { dir, name, manifest, options, addInstaller };
}

describe("signed release validation", () => {
  it("verifies actual installer bytes against the updater key and matching manifest", async () => {
    const { options } = fixture();
    const result = await verifyRelease(options);
    expect(result.version).toBe("0.0.35");
    expect(result.installer_sha256).toMatch(/^[a-f0-9]{64}$/);
  });
  it("rejects a modified installer", async () => {
    const { dir, name, options } = fixture();
    writeFileSync(join(dir, name), "MZ tampered");
    await expect(verifyRelease(options)).rejects.toThrow(/signature/i);
  });
  it("rejects a different signing key", async () => {
    const a = fixture(), b = fixture();
    await expect(verifyRelease({ ...a.options, pubkey: b.options.pubkey })).rejects.toThrow(/signature/i);
  });
  it("rejects altered trusted comments even with intact installer signature", async () => {
    const { dir, name, manifest, options } = fixture();
    const signature = Buffer.from(Buffer.from(manifest.platforms["windows-x86_64"].signature, "base64").toString().replace("timestamp:123", "timestamp:999")).toString("base64");
    manifest.platforms["windows-x86_64"].signature = signature;
    writeFileSync(join(dir, `${name}.sig`), signature);
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow(/signature/i);
  });
  it("requires a matching signature attachment", async () => {
    const { dir, name, options } = fixture();
    writeFileSync(join(dir, `${name}.sig`), "different");
    await expect(verifyRelease(options)).rejects.toThrow(/attachment/i);
  });
  it.each(["https://evil.example/installer.exe", "https://github.com/other/repo/releases/download/v0.0.35/a.exe", "https://github.com/3inchtime/astro_studio/releases/download/v0.0.34/a.exe"])("rejects unbound updater URL %s", async (url) => {
    const { dir, manifest, options } = fixture();
    manifest.platforms["windows-x86_64"].url = url;
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow(/URL/i);
  });
  it("rejects a mismatched manifest version and missing installer", async () => {
    const { dir, name, manifest, options } = fixture();
    manifest.version = "0.0.34";
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow(/version/i);
    manifest.version = "0.0.35";
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    rmSync(join(dir, name));
    await expect(verifyRelease(options)).rejects.toThrow();
  });
  it.each(["windows-x86_64-nsis", "windows-x86_64-msi", "windows-aarch64-nsis"])("rejects an unverified installer-specific entry %s", async (alias) => {
    const { dir, manifest, options } = fixture();
    manifest.platforms[alias] = { url: "https://evil.example/untested.exe", signature: "invalid" };
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow();
  });
  it("rejects a signed NSIS alias that differs from the native-tested generic installer", async () => {
    const { dir, manifest, options, addInstaller } = fixture();
    manifest.platforms["windows-x86_64-nsis"] = addInstaller("Other_0.0.35_x64-setup.exe");
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow(/NSIS.*alias/i);
  });
  it("verifies MSI bytes as well as NSIS while keeping native acceptance bound to NSIS", async () => {
    const { dir, name, manifest, options, addInstaller } = fixture();
    manifest.platforms["windows-x86_64-nsis"] = {...manifest.platforms["windows-x86_64"]};
    const msi="Astro_0.0.35_x64_en-US.msi";
    manifest.platforms["windows-x86_64-msi"] = addInstaller(msi);
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    const result=await verifyRelease(options);
    expect(result.installer).toBe(name);
    expect(result.verified_windows_installers.map((file: {installer:string})=>file.installer).sort()).toEqual([name,msi].sort());
    const hash=createHash("sha256").update(readFileSync(join(dir,name))).digest("hex");
    expect(result.installer_sha256).toBe(hash);
    writeFileSync(join(dir,msi),"tampered MSI");
    await expect(verifyRelease(options)).rejects.toThrow(/signature/i);
  });
  it("verifies downloadable MSI assets even if a manifest alias was removed", async () => {
    const { dir, options, addInstaller } = fixture();
    const msi="Astro_0.0.35_x64_en-US.msi";
    addInstaller(msi);
    writeFileSync(join(dir,msi),"tampered unlisted MSI");
    await expect(verifyRelease(options)).rejects.toThrow(/signature/i);
  });
  it("rejects a Windows installer suffix that does not match its alias", async () => {
    const { dir, manifest, options } = fixture();
    manifest.platforms["windows-x86_64-msi"] = {...manifest.platforms["windows-x86_64"]};
    writeFileSync(join(dir, "latest.json"), JSON.stringify(manifest));
    await expect(verifyRelease(options)).rejects.toThrow(/MSI/i);
  });
  it("binds native acceptance to the exact tested installer", () => {
    const hash = "a".repeat(64);
    expect(() => verifyNativeEvidence("", hash, hash)).toThrow(/evidence/i);
    expect(() => verifyNativeEvidence("https://github.com/3inchtime/astro_studio/issues/1", "b".repeat(64), hash)).toThrow(/hash/i);
    expect(() => verifyNativeEvidence("https://github.com/3inchtime/astro_studio/issues/1", hash, hash)).not.toThrow();
  });
  it("gates release uploads behind quality checks and keeps them draft until native acceptance", () => {
    const workflow = readFileSync(join(__dirname, "../.github/workflows/build-desktop.yml"), "utf8");
    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("needs: [resolve, quality]");
    expect(workflow).toContain("createUpdaterArtifacts");
    expect(workflow).toContain("releaseDraft: true");
    expect(workflow).toContain("verify-release.mjs");
  });
});
