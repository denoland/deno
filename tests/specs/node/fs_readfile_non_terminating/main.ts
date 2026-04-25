import fs from "node:fs/promises";
import assert from "node:assert";

// Test that fs.readFile on non-terminating sources like /dev/urandom
// respects AbortSignal and doesn't hang forever (issue #33237).
// The read loop must yield to the event loop between chunks so that
// abort signals can fire.

const controller = new AbortController();

// Abort after 500ms - enough time to read a few chunks
setTimeout(() => controller.abort(), 500);

try {
  await fs.readFile("/dev/urandom", {
    encoding: "utf8",
    signal: controller.signal,
  });
  assert.fail("readFile should not resolve on infinite source");
} catch (err: unknown) {
  assert.ok(err instanceof Error);
  assert.strictEqual(err.name, "AbortError");
  console.log("ok: readFile properly aborted on non-terminating source");
}
