// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

const pwd = Deno.cwd();

Deno.test("joinZeroLength", function () {
  // join will internally ignore all the zero-length strings and it will return
  // '.' if the joined string is a zero-length string.
  assertEquals(path.posix.join(""), ".");
  assertEquals(path.posix.join("", ""), ".");
  if (path.win32) assertEquals(path.win32.join(""), ".");
  if (path.win32) assertEquals(path.win32.join("", ""), ".");
  assertEquals(path.join(pwd), pwd);
  assertEquals(path.join(pwd, ""), pwd);
});

Deno.test("normalizeZeroLength", function () {
  // normalize will return '.' if the input is a zero-length string
  assertEquals(path.posix.normalize(""), ".");
  if (path.win32) assertEquals(path.win32.normalize(""), ".");
  assertEquals(path.normalize(pwd), pwd);
});

Deno.test("isAbsoluteZeroLength", function () {
  // Since '' is not a valid path in any of the common environments,
  // return false
  assertEquals(path.posix.isAbsolute(""), false);
  if (path.win32) assertEquals(path.win32.isAbsolute(""), false);
});
