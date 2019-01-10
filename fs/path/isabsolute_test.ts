// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/

import { test, assertEqual } from "../../testing/mod.ts";
import * as path from "./mod.ts";

test(function isAbsolute() {
  assertEqual(path.posix.isAbsolute("/home/foo"), true);
  assertEqual(path.posix.isAbsolute("/home/foo/.."), true);
  assertEqual(path.posix.isAbsolute("bar/"), false);
  assertEqual(path.posix.isAbsolute("./baz"), false);
});

test(function isAbsoluteWin32() {
  assertEqual(path.win32.isAbsolute("/"), true);
  assertEqual(path.win32.isAbsolute("//"), true);
  assertEqual(path.win32.isAbsolute("//server"), true);
  assertEqual(path.win32.isAbsolute("//server/file"), true);
  assertEqual(path.win32.isAbsolute("\\\\server\\file"), true);
  assertEqual(path.win32.isAbsolute("\\\\server"), true);
  assertEqual(path.win32.isAbsolute("\\\\"), true);
  assertEqual(path.win32.isAbsolute("c"), false);
  assertEqual(path.win32.isAbsolute("c:"), false);
  assertEqual(path.win32.isAbsolute("c:\\"), true);
  assertEqual(path.win32.isAbsolute("c:/"), true);
  assertEqual(path.win32.isAbsolute("c://"), true);
  assertEqual(path.win32.isAbsolute("C:/Users/"), true);
  assertEqual(path.win32.isAbsolute("C:\\Users\\"), true);
  assertEqual(path.win32.isAbsolute("C:cwd/another"), false);
  assertEqual(path.win32.isAbsolute("C:cwd\\another"), false);
  assertEqual(path.win32.isAbsolute("directory/directory"), false);
  assertEqual(path.win32.isAbsolute("directory\\directory"), false);
});
