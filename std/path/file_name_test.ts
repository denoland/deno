// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

Deno.test("[path] fileName()", function () {
  assertEquals(path.fileName(""), null);
  assertEquals(path.fileName("/"), null);
  assertEquals(path.fileName("/dir/foo.ext"), "foo.ext");
  assertEquals(path.fileName("/foo.ext"), "foo.ext");
  assertEquals(path.fileName("foo.ext"), "foo.ext");
  assertEquals(path.fileName("foo.ext/"), "foo.ext");
  assertEquals(path.fileName("foo.ext//"), "foo.ext");
  assertEquals(path.fileName("/aaa/bbb"), "bbb");
  assertEquals(path.fileName("/aaa/"), "aaa");
  assertEquals(path.fileName("/aaa/b"), "b");
  assertEquals(path.fileName("/a/b"), "b");
  assertEquals(path.fileName("//a"), "a");
});

Deno.test("[path] fileName() posix", function () {
  // On unix a backslash is just treated as any other character.
  assertEquals(path.posix.fileName("\\"), "\\");
  assertEquals(path.posix.fileName("\\foo.ext"), "\\foo.ext");
  assertEquals(path.posix.fileName("\\dir\\foo.ext"), "\\dir\\foo.ext");
  assertEquals(path.posix.fileName("foo.ext"), "foo.ext");
  assertEquals(path.posix.fileName("foo.ext\\"), "foo.ext\\");
  assertEquals(path.posix.fileName("foo.ext\\\\"), "foo.ext\\\\");
});

Deno.test("[path] fileName() win32", function () {
  assertEquals(path.win32.fileName("\\foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("\\dir\\foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("foo.ext\\"), "foo.ext");
  assertEquals(path.win32.fileName("foo.ext\\\\"), "foo.ext");
  assertEquals(path.win32.fileName("foo"), "foo");
  assertEquals(path.win32.fileName("C:"), null);
  assertEquals(path.win32.fileName("C:\\"), null);
  assertEquals(path.win32.fileName("C:\\foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("C:\\dir\\foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("C:foo"), "foo");
  assertEquals(path.win32.fileName("C:foo.ext"), "foo.ext");
  assertEquals(path.win32.fileName("C:foo.ext\\"), "foo.ext");
  assertEquals(path.win32.fileName("C:foo.ext\\\\"), "foo.ext");
  assertEquals(path.win32.fileName("file:stream"), "file:stream");
});
