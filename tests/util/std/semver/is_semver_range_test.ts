// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { ALL } from "./constants.ts";
import { isSemVerRange } from "./is_semver_range.ts";

Deno.test({
  name: "valid_range",
  fn: async (t) => {
    let i = 0;
    const ranges: unknown[] = [
      {
        ranges: [
          [ALL],
        ],
      },
    ];
    for (const r of ranges) {
      await t.step(`valid_range_${(i++).toString().padStart(2, "0")}`, () => {
        const actual = isSemVerRange(r);
        assert(actual);
      });
    }
  },
});
