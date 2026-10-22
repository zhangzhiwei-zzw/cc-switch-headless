import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";
const root = fileURLToPath(new URL("../..", import.meta.url));
const dev = process.argv.includes("--dev");
const result = spawnSync(process.execPath, [
  path.join(root, "node_modules/vite/bin/vite.js"),
  ...(dev ? [] : ["build", "--outDir", "../dist-web"]),
], { cwd: root, env: { ...process.env, CC_SWITCH_WEB: "1" }, stdio: "inherit" });
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
