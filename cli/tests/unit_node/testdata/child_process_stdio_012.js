import childProcess from "node:child_process";
import process from "node:process";
import * as path from "node:path";

const script = path.join(
  path.dirname(path.fromFileUrl(import.meta.url)),
  "node_modules",
  "foo",
  "index.js",
);

const child = childProcess.spawn(process.execPath, [script], {
  stdio: [0, 1, 2],
});
child.on("close", () => console.log("close"));
