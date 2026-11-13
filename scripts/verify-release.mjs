import { createHash, createPublicKey, verify } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readFile } from "node:fs/promises";
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

export async function verifyRelease({ directory, tag, repository, pubkey }) {
  if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag)) throw new Error("Invalid release version tag");
  if (!/^[\w.-]+\/[\w.-]+$/.test(repository)) throw new Error("Invalid repository");
  const manifest = JSON.parse(await readFile(join(directory, "latest.json"), "utf8"));
  if (manifest.version !== tag.slice(1)) throw new Error("Manifest version does not match tag");
  const platform = manifest.platforms?.["windows-x86_64"];
  if (!platform) throw new Error("Missing windows-x86_64 manifest entry");
  const url = new URL(platform.url);
  const prefix = `/${repository}/releases/download/${tag}/`;
  if (url.protocol !== "https:" || url.host !== "github.com" || url.username || url.password ||
      url.search || url.hash || !url.pathname.startsWith(prefix)) throw new Error("Unexpected updater URL");
  const name = decodeURIComponent(url.pathname.slice(prefix.length));
  if (basename(name) !== name || /[\\/\x00-\x1f]/.test(name) || !name.endsWith("-setup.exe")) {
    throw new Error("Updater URL must identify a Windows NSIS installer");
  }
  const path = join(directory, name);
  const info = await lstat(path);
  if (!info.isFile() || info.size === 0) throw new Error("Missing or invalid installer");
  const attached = (await readFile(`${path}.sig`, "utf8")).trim();
  if (attached !== platform.signature.trim()) throw new Error("Signature attachment differs from manifest");
  const material = signingMaterial(pubkey, attached);
  const sha256 = createHash("sha256"), payload = createHash("blake2b512");
  for await (const chunk of createReadStream(path)) { sha256.update(chunk); payload.update(chunk); }
  if (!verify(null, payload.digest(), material.publicKey, material.signature)) throw new Error("Invalid installer signature");
  return { version: manifest.version, installer: name, installer_sha256: sha256.digest("hex"), size: info.size };
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
