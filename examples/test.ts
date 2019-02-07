// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { run } from "deno";
import { test, assertEqual } from "../testing/mod.ts";

/** Example of how to do basic tests */
test(function t1() {
  assertEqual("hello", "hello");
});

test(function t2() {
  assertEqual("world", "world");
});

/** A more complicated test that runs a subprocess. */
/* TODO re-enable this test. Seems to be broken on Windows.
test(async function catSmoke() {
  const p = run({
    args: ["deno", "examples/cat.ts", "README.md"],
    stdout: "piped"
  });
  const s = await p.status();
  assertEqual(s.code, 0);
});
*/
