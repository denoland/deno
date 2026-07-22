import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const result = spawnSync(process.execPath, [fileURLToPath(import.meta.resolve("./spawned.js"))], {
  stdio: "inherit",
});
if (result.error) {
  console.error("Failed:", result.error);
  process.exit(1);
}