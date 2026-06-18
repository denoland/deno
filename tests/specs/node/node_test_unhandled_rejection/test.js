// deno-lint-ignore-file

import test from "node:test";

// An unhandled promise rejection that occurs while a test body is running must
// fail that test, matching Node.js (see denoland/deno#34818). The rejection
// reason becomes the test failure.

// (a) A rejection created with no await at all before the body returns. In
// deno_core the rejection is reported a macrotask after the body resolves; the
// runner drains that window with the test still active so it is attributed.
// This is intentionally stricter than Node 26.3.0, which lets a no-await
// rejection pass the test with a diagnostic; we fail the test instead.
test("rejection with no await fails the test", () => {
  Promise.reject(new Error("boom no await"));
});

// (b) A rejection that surfaces during an await must also fail the test.
test("rejection during await fails the test", async () => {
  (async () => {
    throw new Error("boom during await");
  })();
  await new Promise((resolve) => setTimeout(resolve, 50));
});

// (c) A rejection leaked by an earlier test must not fail a subsequent
// well-behaved test: the attribution window is bounded and per-test. This test
// runs after the two failing tests above and must still pass.
test("well-behaved test still passes", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1));
});
