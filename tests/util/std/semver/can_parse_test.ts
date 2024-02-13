// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { canParse } from "./can_parse.ts";

Deno.test("[semver] canParse", async (t) => {
  // deno-lint-ignore no-explicit-any
  const versions: [any, boolean][] = [
    ["1.2.3", true],
    [" 1.2.3 ", true],
    [" 2.2.3-4 ", true],
    [" 3.2.3-pre ", true],
    ["v5.2.3", true],
    [" v8.2.3 ", true],
    ["\t13.2.3", true],
    ["1.2." + new Array(256).join("1"), false], // too long
    ["1.2." + new Array(100).join("1"), false], // too big
    [null, false],
    [undefined, false],
    [{}, false],
    [[], false],
    [false, false],
    [true, false],
    [0, false],
    ["", false],
    ["not a version", false],
    ["∞.∞.∞", false],
    ["NaN.NaN.NaN", false],
    ["1.2.3.4", false],
    ["NOT VALID", false],
    [1.2, false],
    [null, false],
    ["Infinity.NaN.Infinity", false],
  ];

  for (const [v, expected] of versions) {
    await t.step(`${v}`, () => {
      assertEquals(canParse(v), expected);
    });
  }
});
