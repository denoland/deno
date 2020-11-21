// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEquals } from "../testing/asserts.ts";
import { dirname, fromFileUrl } from "../path/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));

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

    assertStrictEquals(actual, expected);
  } finally {
    process.stdin.close();
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

    assertStrictEquals(actual, expected);
  } finally {
    process.stdin.close();
    process.close();
  }
});

Deno.test("[examples/catj] print multiple files", async () => {
  const decoder = new TextDecoder();
  const process = catj(
    "testdata/catj/simple-object.json",
    "testdata/catj/simple-array.json",
  );
  try {
    const output = await process.output();
    const actual = decoder.decode(output).trim();
    const expected = ['.message = "hello"', ".[0] = 1", ".[1] = 2"].join("\n");

    assertStrictEquals(actual, expected);
  } finally {
    process.stdin.close();
    process.close();
  }
});

Deno.test("[examples/catj] read from stdin", async () => {
  const decoder = new TextDecoder();
  const process = catj("-");
  const input = `{ "foo": "bar" }`;
  try {
    await process.stdin.write(new TextEncoder().encode(input));
    process.stdin.close();
    const output = await process.output();
    const actual = decoder.decode(output).trim();

    assertStrictEquals(actual, '.foo = "bar"');
  } finally {
    process.close();
  }
});

function catj(
  ...files: string[]
): Deno.Process<Deno.RunOptions & { stdin: "piped"; stdout: "piped" }> {
  return Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--allow-read",
      "catj.ts",
      ...files,
    ],
    cwd: moduleDir,
    stdin: "piped",
    stdout: "piped",
    env: { NO_COLOR: "true" },
  });
}
