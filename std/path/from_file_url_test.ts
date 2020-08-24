// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { posix, win32 } from "./mod.ts";
import { assertEquals, assertThrows } from "../testing/asserts.ts";

Deno.test("[path] fromFileUrl", function () {
  assertEquals(posix.fromFileUrl(new URL("file:///home/foo")), "/home/foo");
  assertEquals(posix.fromFileUrl("file:///"), "/");
  assertEquals(posix.fromFileUrl("file:///home/foo"), "/home/foo");
  assertEquals(posix.fromFileUrl("file:///home/foo%20bar"), "/home/foo bar");
  assertEquals(posix.fromFileUrl("file:///%"), "/%");
  assertEquals(posix.fromFileUrl("file://localhost/foo"), "/foo");
  assertEquals(posix.fromFileUrl("file:///C:"), "/C:");
  assertEquals(posix.fromFileUrl("file:///C:/"), "/C:/");
  assertEquals(posix.fromFileUrl("file:///C:/Users/"), "/C:/Users/");
  assertEquals(posix.fromFileUrl("file:///C:foo/bar"), "/C:foo/bar");
  assertThrows(
    () => posix.fromFileUrl("http://localhost/foo"),
    TypeError,
    "Must be a file URL.",
  );
  assertThrows(
    () => posix.fromFileUrl("abcd://localhost/foo"),
    TypeError,
    "Must be a file URL.",
  );
});

Deno.test("[path] fromFileUrl (win32)", function () {
  assertEquals(win32.fromFileUrl(new URL("file:///home/foo")), "\\home\\foo");
  assertEquals(win32.fromFileUrl("file:///"), "\\");
  assertEquals(win32.fromFileUrl("file:///home/foo"), "\\home\\foo");
  assertEquals(win32.fromFileUrl("file:///home/foo%20bar"), "\\home\\foo bar");
  assertEquals(win32.fromFileUrl("file:///%"), "\\%");
  assertEquals(win32.fromFileUrl("file://localhost/foo"), "\\\\localhost\\foo");
  assertEquals(win32.fromFileUrl("file:///C:"), "C:\\");
  assertEquals(win32.fromFileUrl("file:///C:/"), "C:\\");
  // Drop the hostname if a drive letter is parsed.
  assertEquals(win32.fromFileUrl("file://localhost/C:/"), "C:\\");
  assertEquals(win32.fromFileUrl("file:///C:/Users/"), "C:\\Users\\");
  assertEquals(win32.fromFileUrl("file:///C:foo/bar"), "\\C:foo\\bar");
  assertThrows(
    () => win32.fromFileUrl("http://localhost/foo"),
    TypeError,
    "Must be a file URL.",
  );
  assertThrows(
    () => win32.fromFileUrl("abcd://localhost/foo"),
    TypeError,
    "Must be a file URL.",
  );
});
