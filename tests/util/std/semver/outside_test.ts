// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { outside } from "./outside.ts";
import { parse } from "./parse.ts";
import { parseRange } from "./parse_range.ts";

Deno.test({
  name: "outside",
  fn: async (t) => {
    const steps: [string, string, boolean][] = [
      ["1.2.3", "1.0.0 - 1.2.2", true],
      ["1.2.3", "1.0.0 - 1.2.3", false],
      ["0.0.0", "1.0.0 - 1.2.2", true],
      ["1.0.0", "1.0.0 - 1.2.3", false],
    ];
    for (const [version, range, expected] of steps) {
      await t.step({
        name: `${range} ${expected ? "∋" : "∌"} ${version}`,
        fn: () => {
          const v = parse(version);
          const r = parseRange(range);
          const actual = outside(v, r);
          assertEquals(actual, expected);
        },
      });
    }
  },
});
