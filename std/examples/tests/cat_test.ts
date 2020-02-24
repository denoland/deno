// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEq } from "../../testing/asserts.ts";

Deno.test("[examples/cat] print multiple files", async () => {
  const decoder = new TextDecoder();
  const process = Deno.run({
    args: [
      Deno.execPath(),
      "--allow-read",
      "cat.ts",
      "testdata/cat/hello.txt",
      "testdata/cat/world.txt"
    ],
    cwd: "examples",
    stdout: "piped"
  });

  try {
    const output = await Deno.readAll(process.stdout!);
    const actual = decoder.decode(output).trim();
    assertStrictEq(actual, "Hello\nWorld");
  } finally {
    process.close();
  }
});
