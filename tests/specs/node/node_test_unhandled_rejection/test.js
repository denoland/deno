// deno-lint-ignore-file

import test from "node:test";

// An unhandled promise rejection that occurs while a test body is running must
// fail that test, matching Node.js (see denoland/deno#34818). The rejection
// reason becomes the test failure.
test("fails on unhandled rejection", async () => {
  (async () => {
    throw new Error("boom from unhandled rejection");
  })();
  await new Promise((resolve) => setTimeout(resolve, 50));
});

// A subsequent, well-behaved test must still pass; the rejection above is
// attributed only to the test that was active when it fired.
test("passing test still passes", async () => {
  await new Promise((resolve) => setTimeout(resolve, 1));
});
