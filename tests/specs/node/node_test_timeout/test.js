// deno-lint-ignore-file

import test from "node:test";

// A test with { timeout: N } must fail with a timeout error after N ms instead
// of hanging forever. The timeout also aborts t.signal so the body can react.
test("times out", { timeout: 50 }, async (t) => {
  await new Promise((resolve) => {
    const id = setTimeout(resolve, 60_000);
    t.signal.addEventListener("abort", () => {
      clearTimeout(id);
      resolve();
    });
  });
});

// A caller-supplied { signal } that aborts must fail the test with the abort
// reason and abort t.signal.
const ac = new AbortController();
test("aborted by caller signal", { signal: ac.signal }, async (t) => {
  setTimeout(() => ac.abort(new Error("caller aborted the test")), 30);
  await new Promise((resolve) => {
    t.signal.addEventListener("abort", resolve);
  });
});

// A test that finishes before its timeout must pass normally.
test("completes before timeout", { timeout: 5000 }, async () => {
  await new Promise((resolve) => setTimeout(resolve, 10));
});
