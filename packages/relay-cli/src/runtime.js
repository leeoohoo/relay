import { execFileSync, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

export function relayEnvironment(layout, env = process.env) {
  return {
    ...env,
    RELAY_DATA_HOME: layout.dataRoot,
    RELAY_HOST_IMPORT_ROOT: env.RELAY_HOST_IMPORT_ROOT || homedir(),
    COMPOSE_PROJECT_NAME: env.COMPOSE_PROJECT_NAME || "relay",
  };
}

export function runRelay(layout, mode, { quiet = false, env = process.env } = {}) {
  const startScript = join(layout.installRoot, "start.sh");
  if (!existsSync(startScript)) {
    throw new Error("Relay is not installed. Run `npx @relay-ai/relay web` first.");
  }
  const result = spawnSync("bash", [startScript, mode], {
    cwd: layout.installRoot,
    env: relayEnvironment(layout, env),
    stdio: quiet ? "ignore" : "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`Relay command failed: ${mode}`);
  }
}

export function relayUrl(env = process.env) {
  try {
    const port = execFileSync(
      "docker",
      [
        "inspect",
        "-f",
        '{{with index .HostConfig.PortBindings "8080/tcp"}}{{(index . 0).HostPort}}{{end}}',
        "ai-chat-server",
      ],
      { encoding: "utf8", env },
    ).trim();
    if (/^\d+$/.test(port)) {
      return `http://127.0.0.1:${port}`;
    }
  } catch {
    // The startup script already prints the URL; keep the documented fallback.
  }
  return "http://127.0.0.1:45274";
}

export function openRelay(url, { platform = process.platform, env = process.env } = {}) {
  const command =
    platform === "darwin"
      ? ["open", [url]]
      : ["powershell.exe", ["-NoProfile", "-Command", `Start-Process '${url}'`]];
  const result = spawnSync(command[0], command[1], {
    env,
    stdio: "ignore",
    timeout: 10_000,
  });
  if (result.error || result.status !== 0) {
    console.log(`Open Relay in your browser: ${url}`);
  }
}
