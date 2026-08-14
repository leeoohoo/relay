import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { relayLayout } from "./layout.js";
import { detectRelayPlatform } from "./platform.js";
import { installRelease, installedRelease } from "./release.js";
import { openRelay, relayUrl, runRelay } from "./runtime.js";

const packageRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));

const HELP = `Relay npm launcher

Usage:
  npx --yes @relay-ai/relay web       Install, start, and open Relay
  npx --yes @relay-ai/relay install   Download Relay without starting it
  npx --yes @relay-ai/relay status    Show Relay services
  npx --yes @relay-ai/relay logs      Follow Relay logs
  npx --yes @relay-ai/relay restart   Restart Relay
  npx --yes @relay-ai/relay stop      Stop Relay without deleting data
  npx --yes @relay-ai/relay update    Reinstall this Relay version and start it

Environment:
  RELAY_NPX_HOME       Installation root (default: ~/.relay)
  RELAY_DATA_HOME      Persistent data root (default: ~/.relay/data)
  RELAY_HOST_IMPORT_ROOT  Local folders Relay may import (default: user home)
`;

function releaseTag(env = process.env) {
  return env.RELAY_RELEASE_TAG || `v${packageJson.version}`;
}

function needsInstall(layout, tag) {
  const installed = installedRelease(layout.installRoot);
  return !installed || installed.version !== tag;
}

async function ensureInstalled({ layout, target, force = false, env = process.env }) {
  const tag = releaseTag(env);
  if (!force && !needsInstall(layout, tag)) {
    return installedRelease(layout.installRoot);
  }
  if (installedRelease(layout.installRoot)) {
    try {
      runRelay(layout, "down", { quiet: true, env });
    } catch {
      // A stale or partially installed runtime must not prevent repair/update.
    }
  }
  const marker = await installRelease({ layout, target, releaseTag: tag, env });
  console.log(`Relay ${marker.version} installed at ${layout.installRoot}`);
  console.log(`Persistent data: ${layout.dataRoot}`);
  return marker;
}

export async function main(args, env = process.env) {
  const command = args[0] || "web";
  if (["help", "--help", "-h"].includes(command)) {
    console.log(HELP);
    return;
  }
  if (["version", "--version", "-v"].includes(command)) {
    console.log(packageJson.version);
    return;
  }

  const layout = relayLayout(env);
  const target = detectRelayPlatform({ env });

  if (command === "install") {
    await ensureInstalled({ layout, target, env });
    return;
  }

  if (["web", "start", "up", "restart", "update"].includes(command)) {
    const force = command === "update";
    const before = installedRelease(layout.installRoot);
    await ensureInstalled({ layout, target, force, env });
    const mode = command === "restart" && before ? "restart" : "up";
    runRelay(layout, mode, { env });
    const url = relayUrl(env);
    console.log(`Relay is ready: ${url}`);
    if (["web", "start", "up", "update"].includes(command)) {
      openRelay(url, { env });
    }
    return;
  }

  const mode = { stop: "down", down: "down", status: "status", logs: "logs" }[command];
  if (!mode) {
    throw new Error(`unknown command ${JSON.stringify(command)}\n\n${HELP}`);
  }
  runRelay(layout, mode, { env });
}
