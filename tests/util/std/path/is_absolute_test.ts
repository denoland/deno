// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
import { assertEquals } from "../assert/mod.ts";
import * as path from "./mod.ts";

Deno.test("posix.isAbsolute()", function () {
  assertEquals(path.posix.isAbsolute("/home/foo"), true);
  assertEquals(path.posix.isAbsolute("/home/foo/.."), true);
  assertEquals(path.posix.isAbsolute("bar/"), false);
  assertEquals(path.posix.isAbsolute("./baz"), false);
});

Deno.test("win32.isAbsolute()", function () {
  assertEquals(path.win32.isAbsolute("/"), true);
  assertEquals(path.win32.isAbsolute("//"), true);
  assertEquals(path.win32.isAbsolute("//server"), true);
  assertEquals(path.win32.isAbsolute("//server/file"), true);
  assertEquals(path.win32.isAbsolute("\\\\server\\file"), true);
  assertEquals(path.win32.isAbsolute("\\\\server"), true);
  assertEquals(path.win32.isAbsolute("\\\\"), true);
  assertEquals(path.win32.isAbsolute("c"), false);
  assertEquals(path.win32.isAbsolute("c:"), false);
  assertEquals(path.win32.isAbsolute("c:\\"), true);
  assertEquals(path.win32.isAbsolute("c:/"), true);
  assertEquals(path.win32.isAbsolute("c://"), true);
  assertEquals(path.win32.isAbsolute("C:/Users/"), true);
  assertEquals(path.win32.isAbsolute("C:\\Users\\"), true);
  assertEquals(path.win32.isAbsolute("C:cwd/another"), false);
  assertEquals(path.win32.isAbsolute("C:cwd\\another"), false);
  assertEquals(path.win32.isAbsolute("directory/directory"), false);
  assertEquals(path.win32.isAbsolute("directory\\directory"), false);
});
