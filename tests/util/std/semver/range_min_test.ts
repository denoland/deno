// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { format } from "./format.ts";
import { eq } from "./eq.ts";
import { parse } from "./parse.ts";
import { parseRange } from "./parse_range.ts";
import { rangeMin } from "./range_min.ts";
import { INVALID } from "./constants.ts";
import type { SemVer } from "./types.ts";

Deno.test({
  name: "rangeMin",
  fn: async (t) => {
    const versions: [string, string | SemVer][] = [
      // Stars
      ["*", "0.0.0"],
      ["* || >=2", "0.0.0"],
      [">=2 || *", "0.0.0"],
      [">2 || *", "0.0.0"],

      // equal
      ["1.0.0", "1.0.0"],
      ["1.0", "1.0.0"],
      ["1.0.x", "1.0.0"],
      ["1.0.*", "1.0.0"],
      ["1", "1.0.0"],
      ["1.x.x", "1.0.0"],
      ["1.x.x", "1.0.0"],
      ["1.*.x", "1.0.0"],
      ["1.x.*", "1.0.0"],
      ["1.x", "1.0.0"],
      ["1.*", "1.0.0"],
      ["=1.0.0", "1.0.0"],

      // Tilde
      ["~1.1.1", "1.1.1"],
      ["~1.1.1-beta", "1.1.1-beta"],
      ["~1.1.1 || >=2", "1.1.1"],

      // Caret
      ["^1.1.1", "1.1.1"],
      ["^1.1.1-beta", "1.1.1-beta"],
      ["^1.1.1 || >=2", "1.1.1"],

      // '-' operator
      ["1.1.1 - 1.8.0", "1.1.1"],
      ["1.1 - 1.8.0", "1.1.0"],

      // Less / less or equal
      ["<2", "0.0.0"],
      ["<0.0.0-beta", INVALID],
      ["<0.0.1-beta", "0.0.0"],
      ["<2 || >4", "0.0.0"],
      [">4 || <2", "0.0.0"],
      ["<=2 || >=4", "0.0.0"],
      [">=4 || <=2", "0.0.0"],
      ["<0.0.0-beta >0.0.0-alpha", INVALID],
      [">0.0.0-alpha <0.0.0-beta", INVALID],

      // Greater than or equal
      [">=1.1.1 <2 || >=2.2.2 <2", "1.1.1"],
      [">=2.2.2 <2 || >=1.1.1 <2", "1.1.1"],

      // Greater than but not equal
      [">1.0.0", "1.0.1"],
      [">1.0.0-0", "1.0.0-1"],
      [">1.0.0-beta", "1.0.0-beta.0"],
      [">2 || >1.0.0", "1.0.1"],
      [">2 || >1.0.0-0", "1.0.0-1"],
      [">2 || >1.0.0-beta", "1.0.0-beta.0"],

      // Impossible range
      [">4 <3", INVALID],
    ];

    for (const [a, b] of versions) {
      await t.step(a, () => {
        const range = parseRange(a);
        const version = parse(b);
        const min = rangeMin(range);
        assert(eq(min, version), `${format(min)} != ${format(version)}`);
      });
    }
  },
});
