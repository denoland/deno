// Spawn a grandchild and print its PID, then stay alive.
import { join } from "node:path";

const grandchild = new Deno.Command(Deno.execPath(), {
  args: ["run", join(import.meta.dirname!, "grandchild.ts")],
  stdout: "inherit",
  stderr: "inherit",
}).spawn();

// Report the grandchild's PID to the parent.
console.log(grandchild.pid);

// Keep running until killed.
setInterval(() => {}, 60000);
