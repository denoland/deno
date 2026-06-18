// `run({ watch: true })` should emit `test:watch:drained` after the initial
// (empty) run cycle, then emit `test:watch:restarted` followed by another
// `test:watch:drained` when a file inside the watched cwd changes.
import { run } from "node:test";
import { once } from "node:events";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const dir = mkdtempSync(join(tmpdir(), "node-test-run-watch-"));

const events = [];
let drainCount = 0;
const controller = new AbortController();

const stream = run({ cwd: dir, watch: true, signal: controller.signal })
  .on("data", ({ type }) => {
    events.push(type);
    if (type === "test:watch:drained") {
      drainCount++;
      if (drainCount >= 2) {
        controller.abort();
      }
    }
  });

await once(stream, "test:watch:drained");
writeFileSync(join(dir, "test.js"), "module.exports = {};");

// eslint-disable-next-line no-unused-vars
for await (const _ of stream);

rmSync(dir, { recursive: true, force: true });

// Each run cycle also emits the run summary `test:diagnostic` events (tests 0,
// pass 0, ...) before draining, so assert specifically on the ordering of the
// watch lifecycle events this test cares about.
const watchEvents = events.filter((type) => type.startsWith("test:watch:"));
const expected = [
  "test:watch:drained",
  "test:watch:restarted",
  "test:watch:drained",
];
for (let i = 0; i < expected.length; i++) {
  if (watchEvents[i] !== expected[i]) {
    console.log(
      `fail: watchEvents[${i}] = ${watchEvents[i]}, expected ${expected[i]}`,
    );
    console.log(`full events: ${JSON.stringify(events)}`);
    process.exit(1);
  }
}
console.log("ok");
