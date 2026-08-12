// Copyright 2018-2026 the Deno authors. MIT license.
// Input validation for mock.module().
import assert from "node:assert/strict";
import { mock } from "node:test";

assert.throws(() => mock.module(5), { code: "ERR_INVALID_ARG_TYPE" });

assert.throws(
  () => mock.module("./basic-esm.mjs", null),
  { code: "ERR_INVALID_ARG_TYPE" },
);

assert.throws(
  () => mock.module("./basic-esm.mjs", { cache: 5 }),
  { code: "ERR_INVALID_ARG_TYPE" },
);

assert.throws(
  () => mock.module("./basic-esm.mjs", { namedExports: null }),
  { code: "ERR_INVALID_ARG_TYPE" },
);

// The legacy `exports` option must be an object.
assert.throws(
  () => mock.module("./basic-esm.mjs", { exports: null }),
  { code: "ERR_INVALID_ARG_TYPE" },
);

// `exports` cannot be combined with `namedExports` or `defaultExport`.
assert.throws(
  () => mock.module("./basic-esm.mjs", { exports: {}, namedExports: {} }),
  { code: "ERR_INVALID_ARG_VALUE" },
);

assert.throws(
  () => mock.module("./basic-esm.mjs", { exports: {}, defaultExport: {} }),
  { code: "ERR_INVALID_ARG_VALUE" },
);

assert.throws(
  () =>
    mock.module("./basic-esm.mjs", {
      exports: {},
      namedExports: {},
      defaultExport: {},
    }),
  { code: "ERR_INVALID_ARG_VALUE" },
);

console.log("validation ok");
