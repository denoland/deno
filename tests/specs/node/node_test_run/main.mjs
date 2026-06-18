// Exercises the programmatic node:test run() runner end to end: real file
// discovery, child-process execution, and the full TestsStream event surface.
import { run } from "node:test";
import assert from "node:assert";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const dir = dirname(fileURLToPath(import.meta.url));
const passFile = join(dir, "pass.cjs");
const failFile = join(dir, "fail.cjs");
const neverFile = join(dir, "never.cjs");

// 1. for await iteration yields the real lifecycle events of a run.
{
  const types = new Set();
  for await (const event of run({ files: [passFile] })) {
    types.add(event.type);
  }
  assert(types.has("test:pass"), "for-await: expected a test:pass event");
  assert(types.has("test:plan"), "for-await: expected a test:plan event");
  assert(
    types.has("test:diagnostic"),
    "for-await: expected run summary test:diagnostic events",
  );
}

// 2. per-event listeners observe pass/fail/complete across multiple files.
{
  let pass = 0;
  let fail = 0;
  let complete = 0;
  const stream = run({ files: [passFile, failFile] });
  stream.on("test:pass", () => pass++);
  stream.on("test:fail", () => fail++);
  stream.on("test:complete", () => complete++);
  for await (const _ of stream);
  assert.strictEqual(pass, 2, `listeners: expected 2 passes, got ${pass}`);
  assert.strictEqual(fail, 1, `listeners: expected 1 failure, got ${fail}`);
  assert.strictEqual(
    complete,
    3,
    `listeners: expected 3 completes, got ${complete}`,
  );
}

// 3. files-scoped default discovery picks up test files under cwd.
{
  let pass = 0;
  const stream = run({ cwd: join(dir, "discover") });
  stream.on("test:pass", () => pass++);
  stream.on("test:fail", () => assert.fail("discovery: unexpected failure"));
  for await (const _ of stream);
  assert.strictEqual(pass, 1, `discovery: expected 1 pass, got ${pass}`);
}

// 4. an AbortSignal terminates a hanging file and reports it as a failure.
{
  let pass = 0;
  let fail = 0;
  const stream = run({
    signal: AbortSignal.timeout(50),
    files: [neverFile],
  });
  stream.on("test:pass", () => pass++);
  stream.on("test:fail", () => fail++);
  for await (const _ of stream);
  assert.strictEqual(pass, 0, `abort: expected 0 passes, got ${pass}`);
  assert.strictEqual(fail, 1, `abort: expected 1 failure, got ${fail}`);
}

// 5. each forwarded event carries the originating file path.
{
  const stream = run({ files: [passFile] });
  stream.on("test:pass", (data) => {
    assert.strictEqual(data.file, passFile, "event should carry its file");
  });
  for await (const _ of stream);
}

console.log("ok");
