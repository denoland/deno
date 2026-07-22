// deno-lint-ignore-file

import test from "node:test";

// An unhandled promise rejection that surfaces while a test body is still
// running must fail that test, matching Node.js (see denoland/deno#34818). The
// rejection reason becomes the test failure. Attribution is best-effort and
// bounded to the body's lifetime: a rejection whose event fires after the body
// has returned is treated as post-test asynchronous activity and is not
// attributed (the same limitation Node has for activity that outlives a test).

// A rejection that surfaces during an await is attributed to the running test
// and fails it. This is the denoland/deno#34818 repro.
test("rejection during await fails the test", async () => {
  (async () => {
    throw new Error("boom during await");
  })();
  await new Promise((resolve) => setTimeout(resolve, 50));
});

// A test that does not leak a rejection passes normally.
test("well-behaved test still passes", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1));
});
