// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test, assertEqual } from "../../testing/mod.ts";
import * as path from "./mod.ts";
import { cwd } from "deno";

const pwd = cwd();

test(function joinZeroLength() {
  // join will internally ignore all the zero-length strings and it will return
  // '.' if the joined string is a zero-length string.
  assertEqual(path.posix.join(""), ".");
  assertEqual(path.posix.join("", ""), ".");
  if (path.win32) assertEqual(path.win32.join(""), ".");
  if (path.win32) assertEqual(path.win32.join("", ""), ".");
  assertEqual(path.join(pwd), pwd);
  assertEqual(path.join(pwd, ""), pwd);
});

test(function normalizeZeroLength() {
  // normalize will return '.' if the input is a zero-length string
  assertEqual(path.posix.normalize(""), ".");
  if (path.win32) assertEqual(path.win32.normalize(""), ".");
  assertEqual(path.normalize(pwd), pwd);
});

test(function isAbsoluteZeroLength() {
  // Since '' is not a valid path in any of the common environments, return false
  assertEqual(path.posix.isAbsolute(""), false);
  if (path.win32) assertEqual(path.win32.isAbsolute(""), false);
});

test(function resolveZeroLength() {
  // resolve, internally ignores all the zero-length strings and returns the
  // current working directory
  assertEqual(path.resolve(""), pwd);
  assertEqual(path.resolve("", ""), pwd);
});

test(function relativeZeroLength() {
  // relative, internally calls resolve. So, '' is actually the current directory
  assertEqual(path.relative("", pwd), "");
  assertEqual(path.relative(pwd, ""), "");
  assertEqual(path.relative(pwd, pwd), "");
});
