// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { gte } from "./gte.ts";

Deno.test({
  name: "comparison",
  fn: async (t) => {
    // [version1, version2]
    // version1 should be greater than version2
    const versions: [string, string][] = [
      ["0.0.0", "0.0.0"],
      ["0.0.0", "0.0.0-foo"],
      ["0.0.1", "0.0.0"],
      ["1.0.0", "0.9.9"],
      ["0.10.0", "0.9.0"],
      ["0.99.0", "0.10.0"],
      ["2.0.0", "1.2.3"],
      ["1.2.3", "1.2.3-asdf"],
      ["1.2.3", "1.2.3-4"],
      ["1.2.3", "1.2.3-4-foo"],
      ["1.2.3-5-foo", "1.2.3-5"], // numbers < strings, `5-foo` is a string not a number
      ["1.2.3-5", "1.2.3-4"],
      ["1.2.3-5-foo", "1.2.3-5-Foo"],
      ["3.0.0", "2.7.2+asdf"],
      ["1.2.3-a.10", "1.2.3-a.5"],
      ["1.2.3-a.b", "1.2.3-a.5"],
      ["1.2.3-a.b", "1.2.3-a"],
      ["1.2.3-a.b.c.10.d.5", "1.2.3-a.b.c.5.d.100"],
      ["1.2.3-r2", "1.2.3-r100"],
      ["1.2.3-r100", "1.2.3-R2"],
    ];

    for (const [v0, v1] of versions) {
      await t.step(`${v0} <=> ${v1}`, () => {
        const s0 = parse(v0);
        const s1 = parse(v1);
        const actual = gte(s0, s1);
        assertEquals(actual, true, `${v0} >= ${v1} : ${actual}`);
      });
    }
  },
});
