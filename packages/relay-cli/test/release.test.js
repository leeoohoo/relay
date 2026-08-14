import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  checksumForAsset,
  installRelease,
  installedRelease,
  parseReleaseMarker,
} from "../src/release.js";

test("parses packaged release metadata", () => {
  assert.deepEqual(parseReleaseMarker("version=v1.0.4\nplatform=macos-apple-silicon\n"), {
    version: "v1.0.4",
    platform: "macos-apple-silicon",
  });
});

test("selects a checksum by exact asset basename", () => {
  const checksum = "a".repeat(64);
  assert.equal(
    checksumForAsset(`${checksum}  relay-macos-apple-silicon.tar.gz\n`, "relay-macos-apple-silicon.tar.gz"),
    checksum,
  );
});

test("installs a local release archive without mixing application and data roots", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "relay-cli-test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const packageRoot = join(root, "relay-v1.0.4-macos-apple-silicon");
  mkdirSync(join(packageRoot, "scripts"), { recursive: true });
  mkdirSync(join(packageRoot, "bin"), { recursive: true });
  writeFileSync(
    join(packageRoot, "RELAY_RELEASE"),
    "version=v1.0.4\nplatform=macos-apple-silicon\n",
  );
  writeFileSync(join(packageRoot, "start.sh"), "#!/usr/bin/env bash\nexit 0\n");
  writeFileSync(join(packageRoot, "scripts/start.sh"), "#!/usr/bin/env bash\nexit 0\n");
  writeFileSync(join(packageRoot, "bin/ai-chat-agent-trigger"), "#!/usr/bin/env bash\nexit 0\n");
  const archivePath = join(root, "relay.tar.gz");
  execFileSync("tar", ["-C", root, "-czf", archivePath, "relay-v1.0.4-macos-apple-silicon"]);

  const layout = {
    baseRoot: join(root, "managed"),
    installRoot: join(root, "managed", "app"),
    dataRoot: join(root, "managed", "data"),
    cacheRoot: join(root, "managed", "cache"),
  };
  await installRelease({
    layout,
    target: { asset: "relay-macos-apple-silicon.tar.gz", label: "test" },
    releaseTag: "v1.0.4",
    env: { RELAY_RELEASE_ARCHIVE: archivePath },
  });

  assert.equal(installedRelease(layout.installRoot).version, "v1.0.4");
  assert.equal(readFileSync(join(layout.installRoot, "start.sh"), "utf8").includes("exit 0"), true);
  assert.notEqual(layout.installRoot, layout.dataRoot);
});
