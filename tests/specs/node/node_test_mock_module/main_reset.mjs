// Copyright 2018-2026 the Deno authors. MIT license.
// mock.reset() and mock.restoreAll() restore module mocks, matching Node's
// MockTracker semantics, and mocks can also be restored independently.
import assert from "node:assert/strict";
import { mock } from "node:test";

// restoreAll() restores every active module mock.
mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 1;
    },
  },
});
assert.strictEqual((await import("./basic-esm.mjs")).fn(), 1);
mock.restoreAll();
assert.strictEqual(
  (await import("./basic-esm.mjs")).string,
  "original esm string",
);

// reset() also restores module mocks.
mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 2;
    },
  },
});
assert.strictEqual((await import("./basic-esm.mjs")).fn(), 2);
mock.reset();
assert.strictEqual(
  (await import("./basic-esm.mjs")).string,
  "original esm string",
);

// Mocking the same module twice without restoring is an error.
mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 3;
    },
  },
});
assert.throws(
  () =>
    mock.module("./basic-esm.mjs", {
      namedExports: {
        fn() {
          return 4;
        },
      },
    }),
  { code: "ERR_INVALID_STATE" },
);
mock.restoreAll();

// After restoreAll the module can be mocked again.
const ctx = mock.module("./basic-esm.mjs", {
  namedExports: {
    fn() {
      return 5;
    },
  },
});
assert.strictEqual((await import("./basic-esm.mjs")).fn(), 5);
ctx.restore();
assert.strictEqual(
  (await import("./basic-esm.mjs")).string,
  "original esm string",
);

console.log("reset ok");
