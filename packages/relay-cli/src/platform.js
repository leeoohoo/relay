import { readFileSync } from "node:fs";

function isWsl(env = process.env) {
  if (env.WSL_DISTRO_NAME || env.WSL_INTEROP) {
    return true;
  }
  try {
    return /microsoft|wsl/i.test(readFileSync("/proc/version", "utf8"));
  } catch {
    return false;
  }
}

export function detectRelayPlatform({
  platform = process.platform,
  arch = process.arch,
  env = process.env,
} = {}) {
  if (platform === "darwin" && arch === "arm64") {
    return {
      key: "macos-apple-silicon",
      asset: "relay-macos-apple-silicon.tar.gz",
      label: "Apple Silicon macOS",
    };
  }
  if (platform === "linux" && arch === "x64" && isWsl(env)) {
    return {
      key: "windows-wsl2-x86_64",
      asset: "relay-windows-wsl2-x86_64.tar.gz",
      label: "Windows WSL2 x86_64",
    };
  }
  if (platform === "win32") {
    throw new Error(
      "Windows currently runs Relay inside WSL2. Open Ubuntu/WSL and run the same npx command there.",
    );
  }
  if (platform === "darwin") {
    throw new Error("Relay npm releases currently support Apple Silicon Macs only.");
  }
  throw new Error(
    "Unsupported platform. Relay npm releases currently support Apple Silicon macOS and Windows through WSL2.",
  );
}
