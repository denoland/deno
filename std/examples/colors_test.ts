// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEquals } from "../testing/asserts.ts";
import { dirname, fromFileUrl } from "../path/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));

Deno.test("[examples/colors] print a colored text", async () => {
  const decoder = new TextDecoder();
  const process = Deno.run({
    cmd: [Deno.execPath(), "run", "--quiet", "colors.ts"],
    cwd: moduleDir,
    stdout: "piped",
  });
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = "[44m[3m[31m[1mHello world![22m[39m[23m[49m";
    assertStrictEquals(actual, expected);
  } finally {
    process.close();
  }
});
