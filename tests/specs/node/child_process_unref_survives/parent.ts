import { spawn } from "node:child_process";
import { join } from "node:path";

const markerFile = join(Deno.cwd(), "marker.txt");
const child = spawn(Deno.execPath(), [
  "run",
  "--allow-write",
  join(import.meta.dirname!, "child.ts"),
  markerFile,
], { stdio: "ignore" });

child.unref();
// Parent exits immediately — child should continue running and create the marker file.
