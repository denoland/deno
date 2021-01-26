// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { dirname, fromFileUrl, relative } from "../path/mod.ts";
import { resolvePath } from "../fs/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));

/** Example of how to do basic tests */
Deno.test("t1", function (): void {
  assertEquals("hello", "hello");
});

Deno.test("t2", function (): void {
  assertEquals("world", "world");
});

/** A more complicated test that runs a subprocess. */
Deno.test("catSmoke", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--allow-read",
      relative(Deno.cwd(), resolvePath(moduleDir, "cat.ts")),
      relative(Deno.cwd(), resolvePath(moduleDir, "..", "README.md")),
    ],
    stdout: "null",
    stderr: "null",
  });
  const s = await p.status();
  assertEquals(s.code, 0);
  p.close();
});
