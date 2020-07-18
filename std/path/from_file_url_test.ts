// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { posix, win32 } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test("[path] fromFileUrl", function () {
  assertEquals(posix.fromFileUrl(new URL("file:///home/foo")), "/home/foo");
  assertEquals(posix.fromFileUrl("file:///home/foo"), "/home/foo");
  assertEquals(posix.fromFileUrl("https://example.com/foo"), "/foo");
  assertEquals(posix.fromFileUrl("file:///"), "/");
  // FIXME(nayeemrmn): Remove the condition. UNC paths are supported here when
  // run on Windows (matching the underlying URL class), but
  // `posix.fromFileUrl()` should not support them under any circumstance.
  if (Deno.build.os != "windows") {
    assertEquals(posix.fromFileUrl("file:////"), "//");
    assertEquals(posix.fromFileUrl("file:////server"), "//server");
    assertEquals(posix.fromFileUrl("file:////server/file"), "//server/file");
  }
});

Deno.test("[path] fromFileUrl (win32)", function () {
  assertEquals(win32.fromFileUrl(new URL("file:///home/foo")), "\\home\\foo");
  assertEquals(win32.fromFileUrl("file:///home/foo"), "\\home\\foo");
  assertEquals(win32.fromFileUrl("https://example.com/foo"), "\\foo");
  assertEquals(win32.fromFileUrl("file:///"), "\\");
  // FIXME(nayeemrmn): Remove the condition. UNC paths are only supported here
  // when run on Windows (matching the underlying URL class), but
  // `win32.fromFileUrl()` should support them under every circumstance.
  if (Deno.build.os == "windows") {
    assertEquals(win32.fromFileUrl("file:////"), "\\");
    assertEquals(win32.fromFileUrl("file:////server"), "\\");
    assertEquals(win32.fromFileUrl("file:////server/file"), "\\file");
  }
  assertEquals(win32.fromFileUrl("file:///c"), "\\c");
  assertEquals(win32.fromFileUrl("file:///c:"), "c:\\");
  assertEquals(win32.fromFileUrl("file:///c:/"), "c:\\");
  assertEquals(win32.fromFileUrl("file:///C:/"), "C:\\");
  assertEquals(win32.fromFileUrl("file:///C:/Users/"), "C:\\Users\\");
  assertEquals(win32.fromFileUrl("file:///C:cwd/another"), "\\C:cwd\\another");
});
