// Copyright 2018-2026 the Deno authors. MIT license.

// Tests that napi_wrap/napi_unwrap uses a per-isolate Private key,
// matching Node.js behavior. Objects wrapped by one native addon
// should be unwrappable by a different addon loaded in the same
// isolate — even if it's the same .node file loaded via a different
// path (which gives it a separate napi env).

import { copyFileSync } from "node:fs";
import process from "node:process";
import { assertEquals, libPrefix, libSuffix } from "./common.js";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");
const original = `${targetDir}/${libPrefix}test_napi.${libSuffix}`;
const copy = `${targetDir}/${libPrefix}test_napi_cross_env.${libSuffix}`;
copyFileSync(original, copy);

// Load the same addon twice via different paths — each gets its own napi env.
const addon1 = { exports: {} };
process.dlopen(addon1, original);

const addon2 = { exports: {} };
process.dlopen(addon2, copy);

Deno.test("cross-env wrap by addon1, unwrap by addon2", () => {
  const obj = {};
  addon1.exports.test_raw_wrap(obj, 42);
  assertEquals(addon2.exports.test_raw_unwrap(obj), 42);
});

Deno.test("cross-env wrap by addon2, unwrap by addon1", () => {
  const obj = {};
  addon2.exports.test_raw_wrap(obj, 99);
  assertEquals(addon1.exports.test_raw_unwrap(obj), 99);
});

Deno.test("same-env wrap/unwrap still works for addon1", () => {
  const obj = {};
  addon1.exports.test_raw_wrap(obj, 7);
  assertEquals(addon1.exports.test_raw_unwrap(obj), 7);
});

Deno.test("same-env wrap/unwrap still works for addon2", () => {
  const obj = {};
  addon2.exports.test_raw_wrap(obj, 13);
  assertEquals(addon2.exports.test_raw_unwrap(obj), 13);
});
