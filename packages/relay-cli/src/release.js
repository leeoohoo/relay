import { createHash } from "node:crypto";
import {
  chmodSync,
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
} from "node:fs";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { basename, dirname, join } from "node:path";
import { spawnSync } from "node:child_process";

export function parseReleaseMarker(text) {
  return Object.fromEntries(
    text
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const separator = line.indexOf("=");
        return separator < 0
          ? [line, ""]
          : [line.slice(0, separator), line.slice(separator + 1)];
      }),
  );
}

export function installedRelease(installRoot) {
  const markerPath = join(installRoot, "RELAY_RELEASE");
  if (!existsSync(markerPath)) {
    return null;
  }
  return parseReleaseMarker(readFileSync(markerPath, "utf8"));
}

export function checksumForAsset(text, assetName) {
  for (const line of text.split(/\r?\n/)) {
    const match = line.trim().match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (match && basename(match[2]) === assetName) {
      return match[1].toLowerCase();
    }
  }
  throw new Error(`SHA256SUMS does not contain ${assetName}`);
}

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

async function download(url, destination) {
  const response = await fetch(url, {
    redirect: "follow",
    headers: { "user-agent": "@relay-ai/relay" },
    signal: AbortSignal.timeout(120_000),
  });
  if (!response.ok || !response.body) {
    throw new Error(`download failed (${response.status}) for ${url}`);
  }
  mkdirSync(dirname(destination), { recursive: true });
  await pipeline(Readable.fromWeb(response.body), createWriteStream(destination));
}

async function readRemoteText(url) {
  const response = await fetch(url, {
    redirect: "follow",
    headers: { "user-agent": "@relay-ai/relay" },
    signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok) {
    throw new Error(`download failed (${response.status}) for ${url}`);
  }
  return response.text();
}

function extractArchive(archivePath, destination) {
  mkdirSync(destination, { recursive: true });
  const result = spawnSync(
    "tar",
    ["-xzf", archivePath, "--strip-components=1", "-C", destination],
    { stdio: "inherit" },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`could not extract ${archivePath}`);
  }
}

function validateExtractedRelease(stagingRoot) {
  for (const relativePath of [
    "RELAY_RELEASE",
    "start.sh",
    "scripts/start.sh",
    "bin/ai-chat-agent-trigger",
  ]) {
    if (!existsSync(join(stagingRoot, relativePath))) {
      throw new Error(`release archive is missing ${relativePath}`);
    }
  }
  chmodSync(join(stagingRoot, "start.sh"), 0o755);
  chmodSync(join(stagingRoot, "scripts/start.sh"), 0o755);
  chmodSync(join(stagingRoot, "bin/ai-chat-agent-trigger"), 0o755);
}

function replaceInstall(stagingRoot, installRoot) {
  const backupRoot = `${installRoot}.previous`;
  rmSync(backupRoot, { recursive: true, force: true });
  mkdirSync(dirname(installRoot), { recursive: true });
  if (existsSync(installRoot)) {
    renameSync(installRoot, backupRoot);
  }
  try {
    renameSync(stagingRoot, installRoot);
    rmSync(backupRoot, { recursive: true, force: true });
  } catch (error) {
    if (!existsSync(installRoot) && existsSync(backupRoot)) {
      renameSync(backupRoot, installRoot);
    }
    throw error;
  }
}

export async function installRelease({
  layout,
  target,
  releaseTag,
  env = process.env,
}) {
  mkdirSync(layout.baseRoot, { recursive: true });
  mkdirSync(layout.dataRoot, { recursive: true });
  mkdirSync(layout.cacheRoot, { recursive: true });

  const localArchive = env.RELAY_RELEASE_ARCHIVE;
  const repositoryUrl = (env.RELAY_RELEASE_REPOSITORY || "https://github.com/leeoohoo/relay").replace(
    /\/$/,
    "",
  );
  const releaseBase = `${repositoryUrl}/releases/download/${releaseTag}`;
  const archivePath = localArchive
    ? localArchive
    : join(layout.cacheRoot, `${releaseTag}-${target.asset}`);

  if (!localArchive) {
    console.log(`Downloading Relay ${releaseTag} for ${target.label}…`);
    await download(`${releaseBase}/${target.asset}`, archivePath);
    if (env.RELAY_SKIP_CHECKSUM !== "1") {
      const sums = await readRemoteText(`${releaseBase}/SHA256SUMS`);
      const expected = checksumForAsset(sums, target.asset);
      const actual = await sha256(archivePath);
      if (actual !== expected) {
        rmSync(archivePath, { force: true });
        throw new Error(`checksum mismatch for ${target.asset}`);
      }
    }
  }

  const stagingRoot = join(layout.baseRoot, `.install-${process.pid}-${Date.now()}`);
  rmSync(stagingRoot, { recursive: true, force: true });
  try {
    extractArchive(archivePath, stagingRoot);
    validateExtractedRelease(stagingRoot);
    const marker = installedRelease(stagingRoot);
    if (!marker?.version) {
      throw new Error("release archive contains an invalid RELAY_RELEASE marker");
    }
    replaceInstall(stagingRoot, layout.installRoot);
    return marker;
  } finally {
    rmSync(stagingRoot, { recursive: true, force: true });
  }
}
