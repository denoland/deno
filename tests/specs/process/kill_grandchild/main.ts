// Spawn a child which spawns a grandchild. Kill the child and verify
// that the grandchild is also terminated (descendant tree kill).

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

// Kill the child — this should also kill the grandchild via descendant walk.
child.kill("SIGKILL");
await child.status;

// Poll until the grandchild is no longer signalable (ESRCH/NotFound),
// or timeout. This avoids flakiness from fixed sleeps — a killed process
// can briefly remain signalable while in zombie state awaiting reaping.
const timeoutMs = 5000;
const pollIntervalMs = 100;
const start = Date.now();

while (true) {
  try {
    Deno.kill(grandchildPid, "SIGCONT");
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      // Expected: the grandchild is dead (ESRCH).
      console.log("OK: grandchild was killed with parent");
      break;
    }
    console.log("FAIL: unexpected error when checking grandchild");
    Deno.exit(1);
  }

  if (Date.now() - start >= timeoutMs) {
    // If we get here, the grandchild is still alive.
    console.log("FAIL: grandchild is still alive");
    try {
      Deno.kill(grandchildPid, "SIGKILL");
    } catch {
      // Ignore errors during cleanup.
    }
    Deno.exit(1);
  }

  await new Promise((r) => setTimeout(r, pollIntervalMs));
}
