// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { xeval } from "../xeval.ts";
import { stringsReader } from "../../io/util.ts";
import { decode, encode } from "../../strings/mod.ts";
import {
  assertEquals,
  assertStrContains,
  assert,
} from "../../testing/asserts.ts";
const { execPath, run } = Deno;

Deno.test(async function xevalSuccess(): Promise<void> {
  const chunks: string[] = [];
  await xeval(stringsReader("a\nb\nc"), ($): number => chunks.push($));
  assertEquals(chunks, ["a", "b", "c"]);
});

Deno.test(async function xevalDelimiter(): Promise<void> {
  const chunks: string[] = [];
  await xeval(stringsReader("!MADMADAMADAM!"), ($): number => chunks.push($), {
    delimiter: "MADAM",
  });
  assertEquals(chunks, ["!MAD", "ADAM!"]);
});

const xevalPath = "examples/xeval.ts";

Deno.test({
  name: "xevalCliReplvar",
  fn: async function (): Promise<void> {
    const p = run({
      cmd: [execPath(), xevalPath, "--replvar=abc", "console.log(abc)"],
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

Deno.test(async function xevalCliSyntaxError(): Promise<void> {
  const p = run({
    cmd: [execPath(), xevalPath, "("],
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  });
  assertEquals(await p.status(), { code: 1, success: false });
  assertEquals(decode(await p.output()), "");
  assertStrContains(decode(await p.stderrOutput()), "Uncaught SyntaxError");
  p.close();
});
