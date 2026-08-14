import { homedir } from "node:os";
import { join, resolve } from "node:path";

export function relayLayout(env = process.env) {
  const baseRoot = resolve(env.RELAY_NPX_HOME || join(homedir(), ".relay"));
  return {
    baseRoot,
    installRoot: resolve(env.RELAY_INSTALL_HOME || join(baseRoot, "app")),
    dataRoot: resolve(env.RELAY_DATA_HOME || join(baseRoot, "data")),
    cacheRoot: resolve(env.RELAY_DOWNLOAD_CACHE || join(baseRoot, "cache")),
  };
}
