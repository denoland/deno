// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import * as path from "./mod.ts";

Deno.test("[path] fileStem()", function () {
  assertEquals(path.fileStem(""), null);
  assertEquals(path.fileStem("/"), null);
  assertEquals(path.fileStem("."), ".");
  assertEquals(path.fileStem(".."), "..");
  assertEquals(path.fileStem("foo"), "foo");
  assertEquals(path.fileStem("foo.ext"), "foo");
  assertEquals(path.fileStem(".ext"), ".ext");
  assertEquals(path.fileStem("/dir/foo.ext"), "foo");
});

Deno.test("[path] fileStem() posix", function () {
  // On unix a backslash is just treated as any other character.
  assertEquals(path.posix.fileStem("\\foo.ext"), "\\foo");
  assertEquals(path.posix.fileStem("\\dir\\foo.ext"), "\\dir\\foo");
});

Deno.test("[path] fileStem() win32", function () {
  assertEquals(path.win32.fileStem("\\foo.ext"), "foo");
  assertEquals(path.win32.fileStem("\\dir\\foo.ext"), "foo");
  assertEquals(path.win32.fileStem("foo"), "foo");
  assertEquals(path.win32.fileStem("C:"), null);
  assertEquals(path.win32.fileStem("C:\\"), null);
  assertEquals(path.win32.fileStem("C:\\dir\\foo.ext"), "foo");
  assertEquals(path.win32.fileStem("C:."), ".");
  assertEquals(path.win32.fileStem("C:.."), "..");
  assertEquals(path.win32.fileStem("C:foo"), "foo");
  assertEquals(path.win32.fileStem("C:foo.ext"), "foo");
  assertEquals(path.win32.fileStem("file:stream"), "file:stream");
});
