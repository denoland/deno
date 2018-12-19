import { run } from "deno";
import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";

/** Example of how to do basic tests */
test(function t1() {
  assertEqual("hello", "hello");
});

test(function t2() {
  assertEqual("world", "world");
});

/** A more complicated test that runs a subprocess. */
test(async function catSmoke() {
  const p = run({
    args: ["deno", "examples/cat.ts", "README.md"]
  });
  const s = await p.status();
  assertEqual(s.code, 0);
});
