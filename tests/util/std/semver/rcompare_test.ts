// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { rcompare } from "./rcompare.ts";

Deno.test("rcompare", async (t) => {
  const steps: [string, string, number][] = [
    ["1.0.0", "1.0.1", 1],
    ["1.0.0", "1.0.0", 0],
    ["1.0.0+0", "1.0.0", 0],
    ["1.0.0-0", "1.0.0", 1],
    ["1.0.0-1", "1.0.0-0", -1],
    ["1.0.1", "1.0.0", -1],
  ];
  for (const [v0, v1, expected] of steps) {
    await t.step(`${v0} <=> ${v0}`, () => {
      const s0 = parse(v0);
      const s1 = parse(v1);
      const actual = rcompare(s0, s1);
      assertEquals(actual, expected);
    });
  }
});
