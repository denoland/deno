// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { format } from "./format.ts";
import { parse } from "./parse.ts";
import { INVALID, MAX, MIN } from "./constants.ts";
import { FormatStyle, SemVer } from "./types.ts";

Deno.test("format", async (t) => {
  const versions: [string | SemVer, FormatStyle | undefined, string][] = [
    ["1.2.3", undefined, "1.2.3"],
    ["1.2.3", "full", "1.2.3"],
    ["1.2.3", "release", "1.2.3"],
    ["1.2.3", "primary", "1.2.3"],
    ["1.2.3", "build", ""],
    ["1.2.3", "pre", ""],
    ["1.2.3", "patch", "3"],
    ["1.2.3", "minor", "2"],
    ["1.2.3", "major", "1"],

    ["1.2.3-pre", undefined, "1.2.3-pre"],
    ["1.2.3-pre", "full", "1.2.3-pre"],
    ["1.2.3-pre", "release", "1.2.3-pre"],
    ["1.2.3-pre", "primary", "1.2.3"],
    ["1.2.3-pre", "build", ""],
    ["1.2.3-pre", "pre", "pre"],
    ["1.2.3-pre", "patch", "3"],
    ["1.2.3-pre", "minor", "2"],
    ["1.2.3-pre", "major", "1"],

    ["1.2.3-pre.0", undefined, "1.2.3-pre.0"],
    ["1.2.3-pre.0", "full", "1.2.3-pre.0"],
    ["1.2.3-pre.0", "release", "1.2.3-pre.0"],
    ["1.2.3-pre.0", "primary", "1.2.3"],
    ["1.2.3-pre.0", "build", ""],
    ["1.2.3-pre.0", "pre", "pre.0"],
    ["1.2.3-pre.0", "patch", "3"],
    ["1.2.3-pre.0", "minor", "2"],
    ["1.2.3-pre.0", "major", "1"],

    ["1.2.3+b", undefined, "1.2.3+b"],
    ["1.2.3+b", "full", "1.2.3+b"],
    ["1.2.3+b", "release", "1.2.3"],
    ["1.2.3+b", "primary", "1.2.3"],
    ["1.2.3+b", "build", "b"],
    ["1.2.3+b", "pre", ""],
    ["1.2.3+b", "patch", "3"],
    ["1.2.3+b", "minor", "2"],
    ["1.2.3+b", "major", "1"],

    ["1.2.3+b.0", undefined, "1.2.3+b.0"],
    ["1.2.3+b.0", "full", "1.2.3+b.0"],
    ["1.2.3+b.0", "release", "1.2.3"],
    ["1.2.3+b.0", "primary", "1.2.3"],
    ["1.2.3+b.0", "build", "b.0"],
    ["1.2.3+b.0", "pre", ""],
    ["1.2.3+b.0", "patch", "3"],
    ["1.2.3+b.0", "minor", "2"],
    ["1.2.3+b.0", "major", "1"],

    ["1.2.3-pre.0+b.0", undefined, "1.2.3-pre.0+b.0"],
    ["1.2.3-pre.0+b.0", "full", "1.2.3-pre.0+b.0"],
    ["1.2.3-pre.0+b.0", "release", "1.2.3-pre.0"],
    ["1.2.3-pre.0+b.0", "primary", "1.2.3"],
    ["1.2.3-pre.0+b.0", "build", "b.0"],
    ["1.2.3-pre.0+b.0", "pre", "pre.0"],
    ["1.2.3-pre.0+b.0", "patch", "3"],
    ["1.2.3-pre.0+b.0", "minor", "2"],
    ["1.2.3-pre.0+b.0", "major", "1"],

    [MAX, "full", "∞.∞.∞"],
    [MIN, "full", "0.0.0"],
    [INVALID, "full", "⧞.∞.∞"],
  ];

  for (const [version, style, expected] of versions) {
    await t.step({
      name: `format(${version} ${style} ${expected})`,
      fn: () => {
        const v = parse(version)!;
        const actual = format(v, style);
        assertEquals(actual, expected);
      },
    });
  }
});
