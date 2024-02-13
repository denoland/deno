// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { compareBuild } from "./compare_build.ts";

Deno.test("compareBuild", async (t) => {
  // v+b > v
  const steps: [string, string, number][] = [
    ["1.0.0", "1.0.0+0", -1],
    ["1.0.0+0", "1.0.0+0", 0],
    ["1.0.0+0", "1.0.0", 1],
    ["1.0.0+0", "1.0.0+0.0", -1],
    ["1.0.0+0.0", "1.0.0+0.0", 0],
    ["1.0.0+0.0", "1.0.0+0", 1],
    ["1.0.0+0", "1.0.0+1", -1],
    ["1.0.0+0", "1.0.0+0", 0],
    ["1.0.0+1", "1.0.0+0", 1],

    // Builds are sorted alphabetically, not numerically
    ["1.0.0+0001", "1.0.0+2", -1],
  ];
  for (const [v0, v1, expected] of steps) {
    await t.step(`${v0} <=> ${v1}`, () => {
      const s0 = parse(v0);
      const s1 = parse(v1);
      const actual = compareBuild(s0, s1);
      assertEquals(actual, expected);
    });
  }
});
