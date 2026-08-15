import test from "node:test";
import assert from "node:assert/strict";
import { detectRelayPlatform } from "../src/platform.js";

test("selects the Apple Silicon release", () => {
  assert.equal(
    detectRelayPlatform({ platform: "darwin", arch: "arm64", env: {} }).asset,
    "relay-macos-apple-silicon.tar.gz",
  );
});

test("selects the WSL2 release", () => {
  assert.equal(
    detectRelayPlatform({
      platform: "linux",
      arch: "x64",
      env: { WSL_DISTRO_NAME: "Ubuntu" },
    }).asset,
    "relay-windows-wsl2-x86_64.tar.gz",
  );
});

test("native Windows points users to WSL2", () => {
  assert.throws(
    () => detectRelayPlatform({ platform: "win32", arch: "x64", env: {} }),
    /inside WSL2/,
  );
});
