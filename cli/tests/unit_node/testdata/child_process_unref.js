import cp from "node:child_process";
import * as path from "node:path";

const script = path.join(
  path.dirname(new URL(import.meta.url).pathname),
  "infinite_loop.js",
);
const childProcess = cp.spawn(Deno.execPath(), ["run", script]);
childProcess.unref();
