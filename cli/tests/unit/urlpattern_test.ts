// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

Deno.test(function urlPatternFromString() {
  const pattern = new URLPattern("https://deno.land/foo/:bar");
  assertEquals(pattern.protocol, "https");
  assertEquals(pattern.hostname, "deno.land");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  assert(match);
  assertEquals(match.pathname.input, "/foo/x");
  assertEquals(match.pathname.groups, { bar: "x" });
});

Deno.test(function urlPatternFromStringWithBase() {
  const pattern = new URLPattern("/foo/:bar", "https://deno.land");
  assertEquals(pattern.protocol, "https");
  assertEquals(pattern.hostname, "deno.land");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(!pattern.test("https://deno.com/foo/x"));
  const match = pattern.exec("https://deno.land/foo/x");
  assert(match);
  assertEquals(match.pathname.input, "/foo/x");
  assertEquals(match.pathname.groups, { bar: "x" });
});

Deno.test(function urlPatternFromInit() {
  const pattern = new URLPattern({
    pathname: "/foo/:bar",
  });
  assertEquals(pattern.protocol, "*");
  assertEquals(pattern.hostname, "*");
  assertEquals(pattern.pathname, "/foo/:bar");

  assert(pattern.test("https://deno.land/foo/x"));
  assert(pattern.test("https://deno.com/foo/x"));
  assert(!pattern.test("https://deno.com/bar/x"));

  assert(pattern.test({ pathname: "/foo/x" }));
});
