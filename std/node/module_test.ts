// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../testing/asserts.ts";

import { relativePath, resolvePath } from "../fs/mod.ts";
import * as path from "../path/mod.ts";
import { createRequire } from "./module.ts";

const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
const testdataDir = resolvePath(moduleDir, path.join("_fs", "testdata"));

const require = createRequire(import.meta.url);

Deno.test("requireSuccess", function () {
  // Relative to import.meta.url
  const result = require("./tests/cjs/cjs_a.js");
  assert("helloA" in result);
  assert("helloB" in result);
  assert("C" in result);
  assert("leftPad" in result);
  assertEquals(result.helloA(), "A");
  assertEquals(result.helloB(), "B");
  assertEquals(result.C, "C");
  assertEquals(result.leftPad("pad", 4), " pad");
});

Deno.test("requireCycle", function () {
  const resultA = require("./tests/cjs/cjs_cycle_a");
  const resultB = require("./tests/cjs/cjs_cycle_b");
  assert(resultA);
  assert(resultB);
});

Deno.test("requireBuiltin", function () {
  const fs = require("fs");
  assert("readFileSync" in fs);
  const { readFileSync, isNull, extname } = require("./tests/cjs/cjs_builtin");

  const testData = relativePath(
    Deno.cwd(),
    path.join(testdataDir, "hello.txt"),
  );
  assertEquals(
    readFileSync(testData, { encoding: "utf8" }),
    "hello world",
  );
  assert(isNull(null));
  assertEquals(extname("index.html"), ".html");
});

Deno.test("requireIndexJS", function () {
  const { isIndex } = require("./tests/cjs");
  assert(isIndex);
});

Deno.test("requireNodeOs", function () {
  const os = require("os");
  assert(os.arch);
  assert(typeof os.arch() == "string");
});

Deno.test("requireStack", function () {
  const { hello } = require("./tests/cjs/cjs_throw");
  try {
    hello();
  } catch (e) {
    assertStringIncludes(e.stack, "/tests/cjs/cjs_throw.js");
  }
});

Deno.test("requireFileInSymlinkDir", () => {
  const { C } = require("./tests/cjs/dir");
  assertEquals(C, "C");
});
