// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parseRange } from "./parse_range.ts";
import { rangeIntersects } from "./range_intersects.ts";

Deno.test({
  name: "rangesIntersect",
  fn: async (t) => {
    const versions: [string, string, boolean][] = [
      ["1.3.0 || <1.0.0 >2.0.0", "1.3.0 || <1.0.0 >2.0.0", true],
      [">0.0.0", "<1.0.0 >2.0.0", false],
      ["<1.0.0 >2.0.0", ">1.4.0 <1.6.0", false],
      ["<1.0.0 >2.0.0", ">1.4.0 <1.6.0 || 2.0.0", false],
      [">1.0.0 <=2.0.0", "2.0.0", true],
      ["<1.0.0 >=2.0.0", "2.1.0", false],
      ["<1.0.0 >=2.0.0", ">1.4.0 <1.6.0 || 2.0.0", false],
      ["1.5.x", "<1.5.0 || >=1.6.0", false],
      ["<1.5.0 || >=1.6.0", "1.5.x", false],
      [
        "<1.6.16 || >=1.7.0 <1.7.11 || >=1.8.0 <1.8.2",
        ">=1.6.16 <1.7.0 || >=1.7.11 <1.8.0 || >=1.8.2",
        false,
      ],
      [
        "<=1.6.16 || >=1.7.0 <1.7.11 || >=1.8.0 <1.8.2",
        ">=1.6.16 <1.7.0 || >=1.7.11 <1.8.0 || >=1.8.2",
        true,
      ],
      [">=1.0.0", "<=1.0.0", true],
      [">1.0.0 <1.0.0", "<=0.0.0", false],
      ["*", "0.0.1", true],
      ["*", ">=1.0.0", true],
      ["*", ">1.0.0", true],
      ["*", "~1.0.0", true],
      ["*", "<1.6.0", true],
      ["*", "<=1.6.0", true],
      ["1.*", "0.0.1", false],
      ["1.*", "2.0.0", false],
      ["1.*", "1.0.0", true],
      ["1.*", "<2.0.0", true],
      ["1.*", ">1.0.0", true],
      ["1.*", "<=1.0.0", true],
      ["1.*", "^1.0.0", true],
      ["1.0.*", "0.0.1", false],
      ["1.0.*", "<0.0.1", false],
      ["1.0.*", ">0.0.1", true],
      ["*", "1.3.0 || <1.0.0 >2.0.0", true],
      ["1.3.0 || <1.0.0 >2.0.0", "*", true],
      ["1.*", "1.3.0 || <1.0.0 >2.0.0", true],
      ["x", "0.0.1", true],
      ["x", ">=1.0.0", true],
      ["x", ">1.0.0", true],
      ["x", "~1.0.0", true],
      ["x", "<1.6.0", true],
      ["x", "<=1.6.0", true],
      ["1.x", "0.0.1", false],
      ["1.x", "2.0.0", false],
      ["1.x", "1.0.0", true],
      ["1.x", "<2.0.0", true],
      ["1.x", ">1.0.0", true],
      ["1.x", "<=1.0.0", true],
      ["1.x", "^1.0.0", true],
      ["1.0.x", "0.0.1", false],
      ["1.0.x", "<0.0.1", false],
      ["1.0.x", ">0.0.1", true],
      ["x", "1.3.0 || <1.0.0 >2.0.0", true],
      ["1.3.0 || <1.0.0 >2.0.0", "x", true],
      ["1.x", "1.3.0 || <1.0.0 >2.0.0", true],
      ["*", "*", true],
      ["x", "", true],
    ];

    for (const [r1, r2, expected] of versions) {
      await t.step({
        name: `${r1} ∩ ${r2}`,
        fn: () => {
          const range1 = parseRange(r1);
          const range2 = parseRange(r2);
          const actual1 = rangeIntersects(range1, range2);
          const actual2 = rangeIntersects(range2, range1);
          assertEquals(actual1, expected);
          assertEquals(actual2, expected);
        },
      });
    }
  },
});
