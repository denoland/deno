// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEquals } from "../../testing/asserts.ts";
import { resolve, dirname, fromFileUrl } from "../../path/mod.ts";

const moduleDir = resolve(dirname(fromFileUrl(import.meta.url)), "..");

Deno.test("[examples/welcome] print a welcome message", async () => {
  const decoder = new TextDecoder();
  const process = Deno.run({
    cmd: [Deno.execPath(), "run", "welcome.ts"],
    cwd: moduleDir,
    stdout: "piped",
  });
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = "Welcome to Deno 🦕";
    assertStrictEquals(actual, expected);
  } finally {
    process.close();
  }
});
