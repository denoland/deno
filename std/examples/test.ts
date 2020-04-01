// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { run } = Deno;
import { assertEquals } from "../testing/asserts.ts";

/** Example of how to do basic tests */
Deno.test(function t1(): void {
  assertEquals("hello", "hello");
});

Deno.test(function t2(): void {
  assertEquals("world", "world");
});

/** A more complicated test that runs a subprocess. */
Deno.test(async function catSmoke(): Promise<void> {
  const p = run({
    cmd: [
      Deno.execPath(),
      "run",
      "--allow-read",
      "examples/cat.ts",
      "README.md",
    ],
    stdout: "null",
    stderr: "null",
  });
  const s = await p.status();
  assertEquals(s.code, 0);
  p.close();
});
