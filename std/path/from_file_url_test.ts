// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";

Deno.test("[path] fromFileUrl", function () {
  assertEquals(
    path.posix.fromFileUrl(new URL("file:///home/foo")),
    "/home/foo"
  );
  assertEquals(path.posix.fromFileUrl("file:///home/foo"), "/home/foo");
  assertEquals(path.posix.fromFileUrl("https://example.com/foo"), "/foo");
  assertEquals(path.posix.fromFileUrl("file:///"), "/");
});

Deno.test("[path] fromFileUrl (win32)", function () {
  assertEquals(
    path.win32.fromFileUrl(new URL("file:///home/foo")),
    "\\home\\foo"
  );
  assertEquals(path.win32.fromFileUrl("file:///home/foo"), "\\home\\foo");
  assertEquals(path.win32.fromFileUrl("https://example.com/foo"), "\\foo");
  assertEquals(path.win32.fromFileUrl("file:///"), "\\");
  // FIXME(nayeemrmn): Support UNC paths. Needs support in the underlying URL
  // built-in like Chrome has.
  // assertEquals(path.win32.fromFileUrl("file:////"), "\\");
  // assertEquals(path.win32.fromFileUrl("file:////server"), "\\");
  // assertEquals(path.win32.fromFileUrl("file:////server/file"), "\\file");
  assertEquals(path.win32.fromFileUrl("file:///c"), "\\c");
  assertEquals(path.win32.fromFileUrl("file:///c:"), "c:\\");
  assertEquals(path.win32.fromFileUrl("file:///c:/"), "c:\\");
  assertEquals(path.win32.fromFileUrl("file:///C:/"), "C:\\");
  assertEquals(path.win32.fromFileUrl("file:///C:/Users/"), "C:\\Users\\");
  assertEquals(
    path.win32.fromFileUrl("file:///C:cwd/another"),
    "\\C:cwd\\another"
  );
});
