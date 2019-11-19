import { test } from "../testing/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";
import { makeRequire } from "./require.ts";

const selfPath = window.unescape(import.meta.url.substring(7));
// TS compiler would try to resolve if function named "require"
// Thus suffixing it with require_ to fix this...
const require_ = makeRequire(selfPath);

test(function requireSuccess() {
  const result = require_("./node/tests/cjs/cjs_a.js");
  assert("helloA" in result);
  assert("helloB" in result);
  assert("C" in result);
  assert("leftPad" in result);
  assertEquals(result.helloA(), "A");
  assertEquals(result.helloB(), "B");
  assertEquals(result.C, "C");
  assertEquals(result.leftPad("pad", 4), " pad");
});

test(function requireCycle() {
  const resultA = require_("./node/tests/cjs/cjs_cycle_a");
  const resultB = require_("./node/tests/cjs/cjs_cycle_b");
  assert(resultA);
  assert(resultB);
});
