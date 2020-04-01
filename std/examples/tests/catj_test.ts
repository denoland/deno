// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEq } from "../../testing/asserts.ts";

Deno.test("[examples/catj] print an array", async () => {
  const decoder = new TextDecoder();
  const process = catj("testdata/catj/array.json");
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = [
      '.[0] = "string"',
      ".[1] = 100",
      '.[2].key = "value"',
      '.[2].array[0] = "foo"',
      '.[2].array[1] = "bar"',
    ].join("\n");

    assertStrictEq(actual, expected);
  } finally {
    process.stdin!.close();
    process.close();
  }
});

Deno.test("[examples/catj] print an object", async () => {
  const decoder = new TextDecoder();
  const process = catj("testdata/catj/object.json");
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = [
      '.string = "foobar"',
      ".number = 123",
      '.array[0].message = "hello"',
    ].join("\n");

    assertStrictEq(actual, expected);
  } finally {
    process.stdin!.close();
    process.close();
  }
});

Deno.test("[examples/catj] print multiple files", async () => {
  const decoder = new TextDecoder();
  const process = catj(
    "testdata/catj/simple-object.json",
    "testdata/catj/simple-array.json"
  );
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = ['.message = "hello"', ".[0] = 1", ".[1] = 2"].join("\n");

    assertStrictEq(actual, expected);
  } finally {
    process.stdin!.close();
    process.close();
  }
});

Deno.test("[examples/catj] read from stdin", async () => {
  const decoder = new TextDecoder();
  const process = catj("-");
  const input = `{ "foo": "bar" }`;
  try {
    await process.stdin!.write(new TextEncoder().encode(input));
    process.stdin!.close();
    const output = await process.output();
    const actual = decoder.decode(output).trim();

    assertStrictEq(actual, '.foo = "bar"');
  } finally {
    process.close();
  }
});

function catj(...files: string[]): Deno.Process {
  return Deno.run({
    cmd: [Deno.execPath(), "--allow-read", "catj.ts", ...files],
    cwd: "examples",
    stdin: "piped",
    stdout: "piped",
    env: { NO_COLOR: "true" },
  });
}
