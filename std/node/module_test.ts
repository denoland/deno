import { test } from "../testing/mod.ts";
import { assertEquals, assert } from "../testing/asserts.ts";
import { createRequire } from "./module.ts";

// TS compiler would try to resolve if function named "require"
// Thus suffixing it with require_ to fix this...
const require_ = createRequire(import.meta.url);

test(function requireSuccess() {
  // Relative to import.meta.url
  const result = require_("./tests/cjs/cjs_a.js");
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
  const resultA = require_("./tests/cjs/cjs_cycle_a");
  const resultB = require_("./tests/cjs/cjs_cycle_b");
  assert(resultA);
  assert(resultB);
});

test(function requireBuiltin() {
  const fs = require_("fs");
  assert("readFileSync" in fs);
  const { readFileSync, isNull, extname } = require_("./tests/cjs/cjs_builtin");
  assertEquals(
    readFileSync("./node/testdata/hello.txt", { encoding: "utf8" }),
    "hello world"
  );
  assert(isNull(null));
  assertEquals(extname("index.html"), ".html");
});

test(function requireIndexJS() {
  const { isIndex } = require_("./tests/cjs");
  assert(isIndex);
});
