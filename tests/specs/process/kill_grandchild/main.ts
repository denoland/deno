// Spawn a child which spawns a grandchild. Kill the child and verify
// that the grandchild is also terminated (process group kill).

import { join } from "node:path";

const child = new Deno.Command(Deno.execPath(), {
  args: ["run", "--allow-all", join(import.meta.dirname!, "child.ts")],
  stdout: "piped",
  stderr: "null",
}).spawn();

// Read stdout to get the grandchild PID.
const reader = child.stdout.getReader();
let buf = "";
while (true) {
  const { value, done } = await reader.read();
  if (done) break;
  buf += new TextDecoder().decode(value);
  if (buf.includes("\n")) break;
}
reader.releaseLock();

const grandchildPid = parseInt(buf.trim(), 10);

// Give the grandchild a moment to start running.
await new Promise((r) => setTimeout(r, 200));

// Kill the child — this should also kill the grandchild via process group.
child.kill("SIGKILL");
await child.status;

// Wait a moment for the grandchild to be killed.
await new Promise((r) => setTimeout(r, 200));

// Check if the grandchild is still alive.
try {
  Deno.kill(grandchildPid, "SIGCONT");
  // If we get here, the grandchild is still alive — that's a bug.
  console.log("FAIL: grandchild is still alive");
  // Clean up the orphan.
  Deno.kill(grandchildPid, "SIGKILL");
  Deno.exit(1);
} catch {
  // Expected: the grandchild is dead (ESRCH).
  console.log("OK: grandchild was killed with parent");
}
