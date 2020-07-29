// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { posix, win32 } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test("[path] fromFileUrl", function () {
  assertEquals(posix.fromFileUrl(new URL("file:///home/foo")), "/home/foo");
  assertEquals(posix.fromFileUrl("file:///home/foo"), "/home/foo");
  assertEquals(posix.fromFileUrl("file:///home/foo%20bar"), "/home/foo bar");
  assertEquals(posix.fromFileUrl("https://example.com/foo"), "/foo");
  assertEquals(posix.fromFileUrl("file:///"), "/");
  // Drive letters are supported platform-independently to align with the WHATWG
  // URL specification.
  assertEquals(posix.fromFileUrl("file:///c:"), "c:/");
  assertEquals(posix.fromFileUrl("file:///c:/"), "c:/");
  assertEquals(posix.fromFileUrl("file:///C:/"), "C:/");
  assertEquals(posix.fromFileUrl("file:///C:/Users/"), "C:/Users/");
  assertEquals(posix.fromFileUrl("file:///C:foo/bar"), "/C:foo/bar");
});

Deno.test("[path] fromFileUrl (win32)", function () {
  assertEquals(win32.fromFileUrl(new URL("file:///home/foo")), "\\home\\foo");
  assertEquals(win32.fromFileUrl("file:///home/foo"), "\\home\\foo");
  assertEquals(win32.fromFileUrl("file:///home/foo%20bar"), "\\home\\foo bar");
  assertEquals(win32.fromFileUrl("https://example.com/foo"), "\\foo");
  assertEquals(win32.fromFileUrl("file:///"), "\\");
  assertEquals(win32.fromFileUrl("file:///c:"), "c:\\");
  assertEquals(win32.fromFileUrl("file:///c:/"), "c:\\");
  assertEquals(win32.fromFileUrl("file:///C:/"), "C:\\");
  assertEquals(win32.fromFileUrl("file:///C:/Users/"), "C:\\Users\\");
  assertEquals(win32.fromFileUrl("file:///C:foo/bar"), "\\C:foo\\bar");
});
