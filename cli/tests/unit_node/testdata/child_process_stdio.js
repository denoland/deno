import childProcess from "node:child_process";
import process from "node:process";
import * as path from "node:path";

const script = path.join(
  path.dirname(new URL(import.meta.url).pathname),
  "node_modules",
  "foo",
  "index.js",
);

const child = childProcess.spawn(process.execPath, [script], {
  stdio: [process.stdin, process.stdout, process.stderr],
});
child.on("close", () => console.log("close"));
