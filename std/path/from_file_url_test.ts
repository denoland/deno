// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test("[path] fromUrl", function () {
  assertEquals(path.posix.fromUrl(new URL("file:///home/foo")), "/home/foo");
  assertEquals(path.posix.fromUrl("file:///home/foo"), "/home/foo");
  assertEquals(path.posix.fromUrl("https://example.com/foo"), "/foo");
  assertEquals(path.posix.fromUrl("file:///"), "/");
});

Deno.test("[path] fromUrl (win32)", function () {
  assertEquals(path.win32.fromUrl(new URL("file:///home/foo")), "\\home\\foo");
  assertEquals(path.win32.fromUrl("file:///home/foo"), "\\home\\foo");
  assertEquals(path.win32.fromUrl("https://example.com/foo"), "\\foo");
  assertEquals(path.win32.fromUrl("file:///"), "\\");
  // FIXME(nayeemrmn): Support UNC paths. Needs support in the underlying URL
  // built-in like Chrome has.
  // assertEquals(path.win32.fromUrl("file:////"), "\\");
  // assertEquals(path.win32.fromUrl("file:////server"), "\\");
  // assertEquals(path.win32.fromUrl("file:////server/file"), "\\file");
  assertEquals(path.win32.fromUrl("file:///c"), "\\c");
  assertEquals(path.win32.fromUrl("file:///c:"), "\\c:");
  assertEquals(path.win32.fromUrl("file:///c:/"), "c:\\");
  assertEquals(path.win32.fromUrl("file:///C:/"), "C:\\");
  assertEquals(path.win32.fromUrl("file:///C:/Users/"), "C:\\Users\\");
  assertEquals(path.win32.fromUrl("file:///C:cwd/another"), "\\C:cwd\\another");
});
