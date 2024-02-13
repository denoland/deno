// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { compare } from "./compare.ts";

Deno.test("compare ignores build metadata", async (t) => {
  // v+b > v
  const steps: [string, string, number][] = [
    ["1.0.0", "1.0.0+0", 0],
    ["1.0.0+0", "1.0.0+0", 0],
    ["1.0.0+0", "1.0.0", 0],
    ["1.0.0+0", "1.0.0+0.0", 0],
    ["1.0.0+0.0", "1.0.0+0.0", 0],
    ["1.0.0+0.0", "1.0.0+0", 0],
    ["1.0.0+0", "1.0.0+1", 0],
    ["1.0.0+0", "1.0.0+0", 0],
    ["1.0.0+1", "1.0.0+0", 0],
    ["1.0.0+0001", "1.0.0+2", 0],
  ];
  for (const [v0, v1, expected] of steps) {
    await t.step(`${v0} <=> ${v1}`, () => {
      const s0 = parse(v0);
      const s1 = parse(v1);
      const actual = compare(s0, s1);
      assertEquals(actual, expected);
    });
  }
});

Deno.test({
  name: "comparePre",
  fn: async (t) => {
    const cases: [string, string, number][] = [
      ["1.2.3", "1.2.3", 0],
      ["1.2.3", "2.3.4", -1],
      ["1.2.3", "0.1.2", 1],
      ["1.2.3", "1.2.2", 1],
      ["1.2.3", "1.2.3-pre", 1],
      ["1.2.3-alpha.0.pr.1", "1.2.3-alpha.0.pr.1", 0],
      ["1.2.3-alpha.0.pr.1", "9.9.9-alpha.0.pr.1", -1],
      ["1.2.3-alpha.0.pr.1", "1.2.3", -1],
      ["1.2.3-alpha.0.pr.1", "1.2.3-alpha.0.pr.2", -1],
      ["1.2.3-alpha.0.pr.1", "1.2.3-alpha.0.2", 1],
      ["1.2.3-alpha.0.pr.1", "1.2.3-alpha.pr.0", -1],
      ["1.2.3-alpha.0.pr.1", "1.2.3-beta.0.pr.1", -1],
    ];
    for (const [v0, v1, expected] of cases) {
      const s0 = parse(v0);
      const s1 = parse(v1);
      await t.step(`${v0} <=> ${v1}`, () => {
        const actual = compare(s0, s1);
        assertEquals(actual, expected);
      });
    }
  },
});
