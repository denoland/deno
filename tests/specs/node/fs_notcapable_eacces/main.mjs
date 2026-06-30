// Regression test for https://github.com/denoland/deno/issues/32583
// node:fs operations denied by Deno's permission sandbox must surface a
// Node-compatible error (code/errno/syscall) rather than a raw NotCapable error.
//
// Run with --allow-write=./allowed; the paths below sit outside that scope, so
// each operation is denied by the sandbox (a NotCapable error).
import { mkdirSync, openSync, writeFileSync } from "node:fs";

function check(label, fn) {
  try {
    fn();
    console.log(`${label}: NO ERROR`);
  } catch (e) {
    // errno value is platform-specific (-13 on unix, -4092 on windows), so we
    // only assert it is present rather than its exact value.
    console.log(
      `${label}: code=${e.code} errno_set=${
        typeof e.errno === "number"
      } syscall=${e.syscall}`,
    );
  }
}

check("writeFileSync", () => writeFileSync("blocked.txt", "test"));
check("openSync", () => openSync("blocked.txt", "w"));
check("mkdirSync", () => mkdirSync("blocked_dir"));
