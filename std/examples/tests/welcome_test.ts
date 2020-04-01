// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEq } from "../../testing/asserts.ts";

Deno.test("[examples/welcome] print a welcome message", async () => {
  const decoder = new TextDecoder();
  const process = Deno.run({
    cmd: [Deno.execPath(), "welcome.ts"],
    cwd: "examples",
    stdout: "piped",
  });
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = "Welcome to Deno 🦕";
    assertStrictEq(actual, expected);
  } finally {
    process.close();
  }
});
