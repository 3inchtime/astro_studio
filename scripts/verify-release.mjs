import { createHash, createPublicKey, verify } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readFile, readdir } from "node:fs/promises";
import { basename, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

function decode(value) {
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]+={0,2}$/.test(value.trim())) {
    throw new Error("Invalid signature encoding");
  }
  return Buffer.from(value.trim(), "base64");
}

// Tauri encodes the complete minisign text, not only its signature packet.
// Verify both the prehashed payload and authenticated comment (minisign ED).
function signingMaterial(pubkey, encodedSignature) {
  const keyLines = decode(pubkey).toString("utf8").trim().split(/\r?\n/);
  const lines = decode(encodedSignature).toString("utf8").trim().split(/\r?\n/);
  const key = decode(keyLines[1]);
  const packet = decode(lines[1]);
  if (key.length !== 42 || key.subarray(0, 2).toString() !== "Ed" ||
      packet.length !== 74 || packet.subarray(0, 2).toString() !== "ED" ||
      !key.subarray(2, 10).equals(packet.subarray(2, 10)) ||
      lines.length !== 4 || !lines[2].startsWith("trusted comment: ")) {
    throw new Error("Invalid signature key, algorithm or packet");
  }
  const publicKey = createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), key.subarray(10)]),
    format: "der", type: "spki",
  });
  const signature = packet.subarray(10);
  const comment = lines[2].slice("trusted comment: ".length);
  if (!verify(null, Buffer.concat([signature, Buffer.from(comment)]), publicKey, decode(lines[3]))) {
    throw new Error("Invalid trusted-comment signature");
  }
  return { publicKey, signature };
}

function installerName(platform, tag, repository, installerType) {
  const url = new URL(platform.url);
  const prefix = `/${repository}/releases/download/${tag}/`;
  if (url.protocol !== "https:" || url.host !== "github.com" || url.username || url.password ||
      url.search || url.hash || !url.pathname.startsWith(prefix)) throw new Error("Unexpected updater URL");
  const name = decodeURIComponent(url.pathname.slice(prefix.length));
  const suffix = installerType === "msi" ? ".msi" : "-setup.exe";
  if (basename(name) !== name || /[\\/\x00-\x1f]/.test(name) || !name.endsWith(suffix)) {
    throw new Error(`Updater URL must identify a Windows ${installerType === "msi" ? "MSI" : "NSIS"} installer`);
  }
  return name;
}

export async function verifyRelease({ directory, tag, repository, pubkey }) {
  if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) throw new Error("Invalid release version tag");
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error("Invalid repository");
  const manifest = JSON.parse(await readFile(join(directory, "latest.json"), "utf8"));
  if (manifest.version !== tag.slice(1)) throw new Error("Manifest version does not match tag");
  const generic = manifest.platforms?.["windows-x86_64"];
  if (!generic) throw new Error("Missing windows-x86_64 manifest entry");
  // Tauri selects the installer-specific key before the generic fallback. Both
  // must identify the exact NSIS file whose hash binds Windows native acceptance.
  const nsis = manifest.platforms["windows-x86_64-nsis"];
  if (nsis && (nsis.url !== generic.url || nsis.signature !== generic.signature)) {
    throw new Error("NSIS updater alias differs from the native-tested generic installer");
  }
  const primary = installerName(generic, tag, repository, "nsis");
  const verified = new Map();
  async function verifyInstaller(name, expectedSignature) {
    const path = join(directory, name);
    const attached = (await readFile(`${path}.sig`, "utf8")).trim();
    if (expectedSignature !== undefined && attached !== expectedSignature.trim()) {
      throw new Error("Signature attachment differs from manifest");
    }
    if (verified.has(name)) return;
    const info = await lstat(path);
    if (!info.isFile() || info.size === 0) throw new Error("Missing or invalid installer");
    const material = signingMaterial(pubkey, attached);
    const sha256 = createHash("sha256"), payload = createHash("blake2b512");
    for await (const chunk of createReadStream(path)) { sha256.update(chunk); payload.update(chunk); }
    if (!verify(null, payload.digest(), material.publicKey, material.signature)) throw new Error("Invalid installer signature");
    verified.set(name, { installer: name, installer_sha256: sha256.digest("hex"), size: info.size });
  }
  for (const [alias, platform] of Object.entries(manifest.platforms)) {
    if (!alias.startsWith("windows-")) continue;
    const match = /^windows-(?:x86_64|i686|aarch64|armv7)(?:-(nsis|msi))?$/.exec(alias);
    if (!match) throw new Error(`Unsupported Windows updater alias: ${alias}`);
    const name = installerName(platform, tag, repository, match[1] ?? "nsis");
    if (typeof platform.signature !== "string") throw new Error("Missing updater signature");
    await verifyInstaller(name, platform.signature);
  }
  // A downloadable installer is still public release content if its alias was
  // removed from the manifest. Verify these bytes too, including retained MSI.
  for (const name of await readdir(directory)) {
    if (name.endsWith("-setup.exe") || name.endsWith(".msi")) await verifyInstaller(name);
  }
  return { version: manifest.version, ...verified.get(primary), verified_windows_installers: [...verified.values()] };
}

export function verifyNativeEvidence(evidenceUrl, testedHash, actualHash) {
  let url;
  try { url = new URL(evidenceUrl); } catch { throw new Error("Native Windows evidence URL is required"); }
  if (url.protocol !== "https:" || url.username || url.password) throw new Error("Native evidence must use HTTPS");
  if (!/^[a-f0-9]{64}$/.test(testedHash) || testedHash !== actualHash) {
    throw new Error("Native Windows installer hash does not match verified release bytes");
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    const [directory, tag, repository, configPath, evidenceUrl, testedHash] = process.argv.slice(2);
    const config = JSON.parse(await readFile(configPath ?? "src-tauri/tauri.conf.json", "utf8"));
    if (config.version !== tag?.slice(1)) throw new Error("Application version does not match release tag");
    const result = await verifyRelease({ directory, tag, repository, pubkey: config.plugins?.updater?.pubkey });
    if (evidenceUrl !== undefined || testedHash !== undefined) verifyNativeEvidence(evidenceUrl, testedHash, result.installer_sha256);
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } catch (error) {
    process.stderr.write(`Release validation failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
