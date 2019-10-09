// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

const { cwd } = Deno;
import { test } from "../../testing/mod.ts";
import { assertEquals } from "../../testing/asserts.ts";
import * as path from "./mod.ts";

const pwd = cwd();

test(function joinZeroLength() {
  // join will internally ignore all the zero-length strings and it will return
  // '.' if the joined string is a zero-length string.
  assertEquals(path.posix.join(""), ".");
  assertEquals(path.posix.join("", ""), ".");
  if (path.win32) assertEquals(path.win32.join(""), ".");
  if (path.win32) assertEquals(path.win32.join("", ""), ".");
  assertEquals(path.join(pwd), pwd);
  assertEquals(path.join(pwd, ""), pwd);
});

test(function normalizeZeroLength() {
  // normalize will return '.' if the input is a zero-length string
  assertEquals(path.posix.normalize(""), ".");
  if (path.win32) assertEquals(path.win32.normalize(""), ".");
  assertEquals(path.normalize(pwd), pwd);
});

test(function isAbsoluteZeroLength() {
  // Since '' is not a valid path in any of the common environments,
  // return false
  assertEquals(path.posix.isAbsolute(""), false);
  if (path.win32) assertEquals(path.win32.isAbsolute(""), false);
});

test(function resolveZeroLength() {
  // resolve, internally ignores all the zero-length strings and returns the
  // current working directory
  assertEquals(path.resolve(""), pwd);
  assertEquals(path.resolve("", ""), pwd);
});

test(function relativeZeroLength() {
  // relative, internally calls resolve. So, '' is actually the current
  // directory
  assertEquals(path.relative("", pwd), "");
  assertEquals(path.relative(pwd, ""), "");
  assertEquals(path.relative(pwd, pwd), "");
});
