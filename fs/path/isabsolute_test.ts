// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test } from "../../testing/mod.ts";
import { assertEq } from "../../testing/asserts.ts";
import * as path from "./mod.ts";

test(function isAbsolute() {
  assertEq(path.posix.isAbsolute("/home/foo"), true);
  assertEq(path.posix.isAbsolute("/home/foo/.."), true);
  assertEq(path.posix.isAbsolute("bar/"), false);
  assertEq(path.posix.isAbsolute("./baz"), false);
});

test(function isAbsoluteWin32() {
  assertEq(path.win32.isAbsolute("/"), true);
  assertEq(path.win32.isAbsolute("//"), true);
  assertEq(path.win32.isAbsolute("//server"), true);
  assertEq(path.win32.isAbsolute("//server/file"), true);
  assertEq(path.win32.isAbsolute("\\\\server\\file"), true);
  assertEq(path.win32.isAbsolute("\\\\server"), true);
  assertEq(path.win32.isAbsolute("\\\\"), true);
  assertEq(path.win32.isAbsolute("c"), false);
  assertEq(path.win32.isAbsolute("c:"), false);
  assertEq(path.win32.isAbsolute("c:\\"), true);
  assertEq(path.win32.isAbsolute("c:/"), true);
  assertEq(path.win32.isAbsolute("c://"), true);
  assertEq(path.win32.isAbsolute("C:/Users/"), true);
  assertEq(path.win32.isAbsolute("C:\\Users\\"), true);
  assertEq(path.win32.isAbsolute("C:cwd/another"), false);
  assertEq(path.win32.isAbsolute("C:cwd\\another"), false);
  assertEq(path.win32.isAbsolute("directory/directory"), false);
  assertEq(path.win32.isAbsolute("directory\\directory"), false);
});
