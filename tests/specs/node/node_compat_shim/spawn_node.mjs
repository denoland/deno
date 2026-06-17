// Spawns `node` the way native tools (e.g. Next.js Turbopack) do: a raw PATH
// lookup that bypasses Deno's shell-level `node` interception. With no real
// `node` on PATH, Deno should provide itself as `node` so this succeeds.
import { spawnSync } from "node:child_process";

const result = spawnSync("node", ["-e", "process.stdout.write('NODE_OK')"], {
  encoding: "utf8",
});

if (result.error) {
  console.log("spawn error:", result.error.code);
} else {
  console.log("status:", result.status);
  console.log("stdout:", result.stdout);
}
