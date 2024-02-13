// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parseComparator } from "./parse_comparator.ts";
import { comparatorIntersects } from "./comparator_intersects.ts";
import { rangeIntersects } from "./range_intersects.ts";

Deno.test("intersect", async (t) => {
  const versions: [string, string, boolean][] = [
    // One is a Version
    ["1.3.0", ">=1.3.0", true],
    ["1.3.0", ">1.3.0", false],
    [">=1.3.0", "1.3.0", true],
    [">1.3.0", "1.3.0", false],

    // Same direction increasing
    [">1.3.0", ">1.2.0", true],
    [">1.2.0", ">1.3.0", true],
    [">=1.2.0", ">1.3.0", true],
    [">1.2.0", ">=1.3.0", true],

    // Same direction decreasing
    ["<1.3.0", "<1.2.0", true],
    ["<1.2.0", "<1.3.0", true],
    ["<=1.2.0", "<1.3.0", true],
    ["<1.2.0", "<=1.3.0", true],

    // Different directions, same semver and inclusive operator
    [">=1.3.0", "<=1.3.0", true],
    [">=v1.3.0", "<=1.3.0", true],
    [">=1.3.0", ">=1.3.0", true],
    ["<=1.3.0", "<=1.3.0", true],
    ["<=1.3.0", "<=v1.3.0", true],
    [">1.3.0", "<=1.3.0", false],
    [">=1.3.0", "<1.3.0", false],

    // Opposite matching directions
    [">1.0.0", "<2.0.0", true],
    [">=1.0.0", "<2.0.0", true],
    [">=1.0.0", "<=2.0.0", true],
    [">1.0.0", "<=2.0.0", true],
    ["<=2.0.0", ">1.0.0", true],
    ["<=1.0.0", ">=2.0.0", false],
  ];
  for (const v of versions) {
    const comparator1 = parseComparator(v[0]);
    const comparator2 = parseComparator(v[1]);
    const expect = v[2];
    await t.step({
      name: `${v[0]} ${expect ? "∩" : "∁"} ${v[1]}`,
      fn: () => {
        const actual1 = comparatorIntersects(comparator1, comparator2);
        const actual2 = comparatorIntersects(comparator2, comparator1);
        const actual3 = rangeIntersects(
          { ranges: [[comparator1]] },
          { ranges: [[comparator2]] },
        );
        const actual4 = rangeIntersects(
          { ranges: [[comparator2]] },
          { ranges: [[comparator1]] },
        );
        assertEquals(actual1, expect);
        assertEquals(actual2, expect);
        assertEquals(actual3, expect);
        assertEquals(actual4, expect);
      },
    });
  }
});
