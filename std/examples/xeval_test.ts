// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { xeval } from "./xeval.ts";
import { StringReader } from "../io/readers.ts";
import { decode, encode } from "../encoding/utf8.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../testing/asserts.ts";
import { dirname, fromFileUrl } from "../path/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));

Deno.test("xevalSuccess", async function (): Promise<void> {
  const chunks: string[] = [];
  await xeval(new StringReader("a\nb\nc"), ($): number => chunks.push($));
  assertEquals(chunks, ["a", "b", "c"]);
});

Deno.test("xevalDelimiter", async function (): Promise<void> {
  const chunks: string[] = [];
  await xeval(
    new StringReader("!MADMADAMADAM!"),
    ($): number => chunks.push($),
    {
      delimiter: "MADAM",
    },
  );
  assertEquals(chunks, ["!MAD", "ADAM!"]);
});

const xevalPath = "xeval.ts";

Deno.test({
  name: "xevalCliReplvar",
  fn: async function (): Promise<void> {
    const p = Deno.run({
      cmd: [
        Deno.execPath(),
        "run",
        xevalPath,
        "--replvar=abc",
        "console.log(abc)",
      ],
      cwd: moduleDir,
      stdin: "piped",
      stdout: "piped",
      stderr: "null",
    });
    assert(p.stdin != null);
    await p.stdin.write(encode("hello"));
    p.stdin.close();
    assertEquals(await p.status(), { code: 0, success: true });
    assertEquals(decode(await p.output()).trimEnd(), "hello");
    p.close();
  },
});

Deno.test("xevalCliSyntaxError", async function (): Promise<void> {
  const p = Deno.run({
    cmd: [Deno.execPath(), "run", xevalPath, "("],
    cwd: moduleDir,
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  });
  assertEquals(await p.status(), { code: 1, success: false });
  assertEquals(decode(await p.output()), "");
  assertStringIncludes(decode(await p.stderrOutput()), "SyntaxError");
  p.close();
});
