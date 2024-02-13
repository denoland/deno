// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { parseRange } from "./parse_range.ts";
import { maxSatisfying } from "./max_satisfying.ts";
import { MAX, MIN } from "./constants.ts";

Deno.test({
  name: "maxSatisfying",
  fn: async (t) => {
    const versions: [string[], string, string][] = [
      [["1.2.3", "1.2.4"], "1.2", "1.2.4"],
      [["1.2.4", "1.2.3"], "1.2", "1.2.4"],
      [["1.2.3", "1.2.4", "1.2.5", "1.2.6"], "~1.2.3", "1.2.6"],
    ];

    for (const [v, r, e] of versions) {
      await t.step(`[${v}] ${r} : ${e}`, () => {
        const versions = v.map((v) => parse(v));
        const range = parseRange(r);
        const expect = parse(e);
        const actual = maxSatisfying(versions, range);
        assertEquals(actual, expect);
      });
    }
  },
});

Deno.test("badRangesInMaxOrMinSatisfying", function () {
  const r = parseRange("some frogs and sneks-v2.5.6");
  assertEquals(maxSatisfying([MIN, MAX], r), undefined);
});
