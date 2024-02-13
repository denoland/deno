// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../assert/mod.ts";
import * as path from "./mod.ts";

const pwd = Deno.cwd();
Deno.test(`join() returns "." if input is empty`, function () {
  assertEquals(path.posix.join(""), ".");
  assertEquals(path.posix.join("", ""), ".");
  if (path.win32) assertEquals(path.win32.join(""), ".");
  if (path.win32) assertEquals(path.win32.join("", ""), ".");
  assertEquals(path.join(pwd), pwd);
  assertEquals(path.join(pwd, ""), pwd);
});

Deno.test(`normalize() returns "." if input is empty`, function () {
  assertEquals(path.posix.normalize(""), ".");
  if (path.win32) assertEquals(path.win32.normalize(""), ".");
  assertEquals(path.normalize(pwd), pwd);
});

Deno.test("isAbsolute() retuns false if input is empty", function () {
  assertEquals(path.posix.isAbsolute(""), false);
  if (path.win32) assertEquals(path.win32.isAbsolute(""), false);
});

Deno.test("resolve() returns current working directory if input is empty", function () {
  assertEquals(path.resolve(""), pwd);
  assertEquals(path.resolve("", ""), pwd);
});

Deno.test("relative() returns current working directory if input is empty", function () {
  assertEquals(path.relative("", pwd), "");
  assertEquals(path.relative(pwd, ""), "");
  assertEquals(path.relative(pwd, pwd), "");
});
