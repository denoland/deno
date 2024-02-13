// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import { parse } from "./parse.ts";
import { ReleaseType } from "./types.ts";
import { difference } from "./difference.ts";

Deno.test("diff", async (t) => {
  const versions: [string, string, ReleaseType | undefined][] = [
    ["1.2.3", "0.2.3", "major"],
    ["1.4.5", "0.2.3", "major"],
    ["1.2.3", "2.0.0-pre", "premajor"],
    ["1.2.3", "1.3.3", "minor"],
    ["1.0.1", "1.1.0-pre", "preminor"],
    ["1.2.3", "1.2.4", "patch"],
    ["1.2.3", "1.2.4-pre", "prepatch"],
    ["0.0.1", "0.0.1-pre", "prerelease"],
    ["0.0.1", "0.0.1-pre-2", "prerelease"],
    ["1.1.0", "1.1.0-pre", "prerelease"],
    ["1.1.0-pre-1", "1.1.0-pre-2", "prerelease"],
    ["1.0.0", "1.0.0", undefined],
  ];

  for (const [v0, v1, expected] of versions) {
    await t.step(`${v0} ≏ ${v1} : ${expected}`, () => {
      const s0 = parse(v0);
      const s1 = parse(v1);
      const actual = difference(s0, s1);
      assertEquals(actual, expected);
    });
  }
});
