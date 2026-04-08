// Copyright 2018-2026 the Deno authors. MIT license.

// Tests that napi_wrap/napi_unwrap uses a per-isolate Private key,
// matching Node.js behavior. Objects wrapped by one native addon
// should be unwrappable by a different addon loaded in the same
// isolate.

import process from "node:process";
import { assertEquals, libPrefix, libSuffix } from "./common.js";

const targetDir = Deno.execPath().replace(/[^\/\\]+$/, "");

// Load the primary test addon (test_napi)
const mod1 = { exports: {} };
process.dlopen(
  mod1,
  `${targetDir}/${libPrefix}test_napi.${libSuffix}`,
);

// Load the second test addon (test_napi_2) — separate .node with its own env
const mod2 = { exports: {} };
process.dlopen(
  mod2,
  `${targetDir}/${libPrefix}test_napi_2.${libSuffix}`,
);

Deno.test("cross-addon wrap by addon1, unwrap by addon2", () => {
  const obj = {};
  mod1.exports.test_raw_wrap(obj, 42);
  assertEquals(mod2.exports.unwrapObject(obj), 42);
});

Deno.test("cross-addon wrap by addon2, unwrap by addon1", () => {
  const obj = {};
  mod2.exports.wrapObject(obj, 99);
  assertEquals(mod1.exports.test_raw_unwrap(obj), 99);
});

Deno.test("same-addon wrap/unwrap still works for addon1", () => {
  const obj = {};
  mod1.exports.test_raw_wrap(obj, 7);
  assertEquals(mod1.exports.test_raw_unwrap(obj), 7);
});

Deno.test("same-addon wrap/unwrap still works for addon2", () => {
  const obj = {};
  mod2.exports.wrapObject(obj, 13);
  assertEquals(mod2.exports.unwrapObject(obj), 13);
});
