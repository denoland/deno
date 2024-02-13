// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { INVALID, MIN } from "./constants.ts";
import { isSemVerComparator } from "./is_semver_comparator.ts";

Deno.test({
  name: "valid_comparator",
  fn: async (t) => {
    let i = 0;
    const comparators: unknown[] = [
      {
        operator: ">=",
        semver: { major: 0, minor: 0, patch: 0, prerelease: [], build: [] },
        min: { major: 0, minor: 0, patch: 0, prerelease: [], build: [] },
        max: { major: 0, minor: 0, patch: 0, prerelease: [], build: [] },
      },
      {
        operator: "<",
        semver: MIN,
        min: INVALID,
        max: INVALID,
      },
    ];
    for (const c of comparators) {
      await t.step(
        `valid_comparator_${(i++).toString().padStart(2, "0")}`,
        () => {
          const actual = isSemVerComparator(c);
          assert(actual);
        },
      );
    }
  },
});
