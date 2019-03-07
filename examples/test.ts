// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { run } = Deno;
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";

/** Example of how to do basic tests */
test(function t1() {
  assertEquals("hello", "hello");
});

test(function t2() {
  assertEquals("world", "world");
});

/** A more complicated test that runs a subprocess. */
test(async function catSmoke() {
  const p = run({
    args: ["deno", "--allow-read", "examples/cat.ts", "README.md"],
    stdout: "piped"
  });
  const s = await p.status();
  assertEquals(s.code, 0);
});
