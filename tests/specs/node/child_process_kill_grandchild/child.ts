// Spawn a grandchild and report its PID, then stay alive.
import { join } from "node:path";

const grandchild = new Deno.Command(Deno.execPath(), {
  args: ["run", join(import.meta.dirname!, "grandchild.ts")],
  stdout: "inherit",
  stderr: "inherit",
}).spawn();

console.log(grandchild.pid);

// Keep running until killed.
setInterval(() => {}, 60000);
