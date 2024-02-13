// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { eq } from "./eq.ts";

Deno.test({
  name: "comparison",
  fn: async (t) => {
    // [version1, version2]
    // version1 should be greater than version2
    const versions: [string, string, boolean][] = [
      ["0.0.0", "0.0.0", true],
      ["1.2.3", "1.2.3", true],
      ["1.2.3-pre.0", "1.2.3-pre.0", true],
      ["1.2.3-pre.0+abc", "1.2.3-pre.0+abc", true],
      ["0.0.0", "0.0.0-foo", false],
      ["0.0.1", "0.0.0", false],
      ["1.0.0", "0.9.9", false],
      ["0.10.0", "0.9.0", false],
      ["0.99.0", "0.10.0", false],
      ["2.0.0", "1.2.3", false],
      ["1.2.3", "1.2.3-asdf", false],
      ["1.2.3", "1.2.3-4", false],
      ["1.2.3", "1.2.3-4-foo", false],
      ["1.2.3-5", "1.2.3-5-foo", false], // numbers > strings, `5-foo` is a string not a number
      ["1.2.3-5", "1.2.3-4", false],
      ["1.2.3-5-foo", "1.2.3-5-Foo", false],
      ["3.0.0", "2.7.2+asdf", false],
      ["1.2.3-a.10", "1.2.3-a.5", false],
      ["1.2.3-a.5", "1.2.3-a.b", false],
      ["1.2.3-a.b", "1.2.3-a", false],
      ["1.2.3-a.b.c.10.d.5", "1.2.3-a.b.c.5.d.100", false],
      ["1.2.3-r2", "1.2.3-r100", false],
      ["1.2.3-r100", "1.2.3-R2", false],
    ];

    for (const [v0, v1, expected] of versions) {
      await t.step(`${v0} == ${v1}`, () => {
        const s0 = parse(v0);
        const s1 = parse(v1);

        const eq0 = eq(s0, s0);
        const eq1 = eq(s1, s1);
        const eq2 = eq(s0, s1);
        const eq3 = eq(s1, s0);
        const op = expected ? "==" : "!=";

        assert(eq0, `${s0} == ${s0}`);
        assert(eq1, `${s1} == ${s1}`);
        assertEquals(eq2, expected, `${s0} ${op} ${s1}`);
        assertEquals(eq3, expected, `${s0} ${op} ${s1}`);
      });
    }
  },
});
