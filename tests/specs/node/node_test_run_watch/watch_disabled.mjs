// `run({ watch: false })` should not emit `test:watch:restarted` even when
// files in the watched cwd change after the run starts.
import { run } from "node:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const dir = mkdtempSync(join(tmpdir(), "node-test-run-watch-"));

let restarted = false;
const stream = run({ cwd: dir, watch: false });
stream.on("test:watch:restarted", () => {
  restarted = true;
});

writeFileSync(join(dir, "test.js"), "module.exports = {};");

// eslint-disable-next-line no-unused-vars
for await (const _ of stream);

rmSync(dir, { recursive: true, force: true });

if (restarted) {
  console.log("fail: test:watch:restarted was emitted");
  process.exit(1);
}
console.log("ok");
