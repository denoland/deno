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

console.log("validation ok");
