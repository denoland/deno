// Test that node:child_process.kill("SIGKILL") does NOT kill grandchild
// processes, matching Node.js behavior.
import { spawn } from "node:child_process";
import { join } from "node:path";

const child = spawn(Deno.execPath(), [
  "run",
  "--allow-all",
  join(import.meta.dirname!, "child.ts"),
], { stdio: ["ignore", "pipe", "ignore"] });

// Read the grandchild PID from child's stdout.
let buf = "";
for await (const chunk of child.stdout!) {
  buf += chunk.toString();
  if (buf.includes("\n")) break;
}

const grandchildPid = parseInt(buf.trim(), 10);

// Give the grandchild a moment to start.
await new Promise((r) => setTimeout(r, 200));

// Kill the child with SIGKILL via node:child_process API.
child.kill("SIGKILL");

// Wait for child to exit.
await new Promise<void>((resolve) => child.on("exit", resolve));

// Wait a moment then check if grandchild is still alive.
await new Promise((r) => setTimeout(r, 200));

try {
  // Signal 0 checks if process exists without sending a signal.
  process.kill(grandchildPid, 0);
  console.log("OK: grandchild survived (matches Node.js behavior)");
  // Clean up the orphaned grandchild.
  process.kill(grandchildPid, "SIGKILL");
} catch {
  console.log("FAIL: grandchild was killed (diverges from Node.js behavior)");
  Deno.exit(1);
}
