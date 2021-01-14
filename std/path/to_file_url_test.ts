// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { posix, win32 } from "./mod.ts";
import { assertEquals, assertThrows } from "../testing/asserts.ts";

Deno.test("[path] toFileUrl", function () {
  assertEquals(posix.toFileUrl("/home/foo").href, "file:///home/foo");
  assertEquals(posix.toFileUrl("/home/ ").href, "file:///home/%20");
  assertEquals(posix.toFileUrl("/home/%20").href, "file:///home/%2520");
  assertEquals(posix.toFileUrl("/home\\foo").href, "file:///home%5Cfoo");
  assertThrows(
    () => posix.toFileUrl("foo").href,
    TypeError,
    "Must be an absolute path.",
  );
  assertThrows(
    () => posix.toFileUrl("C:/"),
    TypeError,
    "Must be an absolute path.",
  );
  assertEquals(
    posix.toFileUrl("//localhost/home/foo").href,
    "file:////localhost/home/foo",
  );
  assertEquals(posix.toFileUrl("//localhost/").href, "file:////localhost/");
  assertEquals(posix.toFileUrl("//:/home/foo").href, "file:////:/home/foo");
});

Deno.test("[path] toFileUrl (win32)", function () {
  assertEquals(win32.toFileUrl("/home/foo").href, "file:///home/foo");
  assertEquals(win32.toFileUrl("/home/ ").href, "file:///home/%20");
  assertEquals(win32.toFileUrl("/home/%20").href, "file:///home/%2520");
  assertEquals(win32.toFileUrl("/home\\foo").href, "file:///home/foo");
  assertThrows(
    () => win32.toFileUrl("foo").href,
    TypeError,
    "Must be an absolute path.",
  );
  assertEquals(win32.toFileUrl("C:/").href, "file:///C:/");
  assertEquals(
    win32.toFileUrl("//localhost/home/foo").href,
    "file://localhost/home/foo",
  );
  assertEquals(win32.toFileUrl("//localhost/").href, "file:////localhost/");
  assertThrows(
    () => win32.toFileUrl("//:/home/foo").href,
    TypeError,
    "Invalid hostname.",
  );
});
