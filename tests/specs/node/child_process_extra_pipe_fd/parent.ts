// Regression test for https://github.com/denoland/deno/issues/33368
// Spawn a child with an extra pipe at fd 3 and verify the child can
// write to it via fs.createWriteStream and the parent can read from it.

import { spawn } from "node:child_process";

const child = spawn(process.execPath, [import.meta.dirname + "/child.ts"], {
  stdio: ["inherit", "inherit", "inherit", "pipe"],
});

child.stdio[3]!.on("data", (d: Buffer) => {
  console.log("parent got: " + d.toString().trim());
});

child.on("exit", (code: number | null) => {
  console.log("child exit: " + code);
});
