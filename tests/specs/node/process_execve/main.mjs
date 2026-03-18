import process from "node:process";
import { strict as assert } from "node:assert";

if (process.argv[2] === "replaced") {
  assert.deepStrictEqual(process.argv.slice(-1), ["replaced"]);
  assert.strictEqual(process.env.EXECVE_A, "FIRST");
  assert.strictEqual(process.env.EXECVE_B, "SECOND");
  assert.strictEqual(process.env.CWD, process.cwd());
  console.log("OK");
} else {
  process.execve(
    process.execPath,
    [process.execPath, "run", "-A", new URL(import.meta.url).pathname, "replaced"],
    { ...process.env, EXECVE_A: "FIRST", EXECVE_B: "SECOND", CWD: process.cwd() },
  );

  // If process.execve succeeds, this should never be executed.
  assert.fail("process.execve failed");
}
