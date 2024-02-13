// Copyright Isaac Z. Schlueter and Contributors. All rights reserved. ISC license.
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import type { ReleaseType } from "./types.ts";
import { parse } from "./parse.ts";
import { increment } from "./increment.ts";
import { format } from "./format.ts";

Deno.test("increment", async (t) => {
  //  [version, inc, result, identifier]
  //  increment(version, inc) -> result
  const versions: [
    string,
    ReleaseType,
    string?,
    string?,
    string?,
  ][] = [
    ["1.2.3", "major", undefined, undefined, "2.0.0"],
    ["1.2.3", "minor", undefined, undefined, "1.3.0"],
    ["1.2.3", "patch", undefined, undefined, "1.2.4"],
    ["1.2.3-tag", "major", undefined, undefined, "2.0.0"],
    ["1.2.0-0", "patch", undefined, undefined, "1.2.0"],
    ["1.2.3-4", "major", undefined, undefined, "2.0.0"],
    ["1.2.3-4", "minor", undefined, undefined, "1.3.0"],
    ["1.2.3-4", "patch", undefined, undefined, "1.2.3"],
    ["1.2.3-alpha.0.beta", "major", undefined, undefined, "2.0.0"],
    ["1.2.3-alpha.0.beta", "minor", undefined, undefined, "1.3.0"],
    ["1.2.3-alpha.0.beta", "patch", undefined, undefined, "1.2.3"],
    ["1.2.4", "prerelease", undefined, undefined, "1.2.5-0"],
    ["1.2.3-0", "prerelease", undefined, undefined, "1.2.3-1"],
    ["1.2.3-alpha.0", "prerelease", undefined, undefined, "1.2.3-alpha.1"],
    ["1.2.3-alpha.1", "prerelease", undefined, undefined, "1.2.3-alpha.2"],
    ["1.2.3-alpha.2", "prerelease", undefined, undefined, "1.2.3-alpha.3"],
    [
      "1.2.3-alpha.0.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.1.beta",
    ],
    [
      "1.2.3-alpha.1.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.2.beta",
    ],
    [
      "1.2.3-alpha.2.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.3.beta",
    ],
    [
      "1.2.3-alpha.10.0.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.1.beta",
    ],
    [
      "1.2.3-alpha.10.1.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.2.beta",
    ],
    [
      "1.2.3-alpha.10.2.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.3.beta",
    ],
    [
      "1.2.3-alpha.10.beta.0",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.beta.1",
    ],
    [
      "1.2.3-alpha.10.beta.1",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.beta.2",
    ],
    [
      "1.2.3-alpha.10.beta.2",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.beta.3",
    ],
    [
      "1.2.3-alpha.9.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.10.beta",
    ],
    [
      "1.2.3-alpha.10.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.11.beta",
    ],
    [
      "1.2.3-alpha.11.beta",
      "prerelease",
      undefined,
      undefined,
      "1.2.3-alpha.12.beta",
    ],
    ["1.2.0", "prepatch", undefined, undefined, "1.2.1-0"],
    ["1.2.0-1", "prepatch", undefined, undefined, "1.2.1-2"],
    ["1.2.0", "preminor", undefined, undefined, "1.3.0-0"],
    ["1.2.3-1", "preminor", undefined, undefined, "1.3.0-2"],
    ["1.2.0", "premajor", undefined, undefined, "2.0.0-0"],
    ["1.2.3-1", "premajor", undefined, undefined, "2.0.0-2"],
    ["1.2.0-1", "minor", undefined, undefined, "1.2.0"],
    ["1.0.0-1", "major", undefined, undefined, "1.0.0"],

    ["1.2.3", "major", "dev", undefined, "2.0.0"],
    ["1.2.3", "minor", "dev", undefined, "1.3.0"],
    ["1.2.3", "patch", "dev", undefined, "1.2.4"],
    ["1.2.3-tag", "major", "dev", undefined, "2.0.0"],
    ["1.2.0-0", "patch", "dev", undefined, "1.2.0"],
    ["1.2.3-4", "major", "dev", undefined, "2.0.0"],
    ["1.2.3-4", "minor", "dev", undefined, "1.3.0"],
    ["1.2.3-4", "patch", "dev", undefined, "1.2.3"],
    ["1.2.3-alpha.0.beta", "major", "dev", undefined, "2.0.0"],
    ["1.2.3-alpha.0.beta", "minor", "dev", undefined, "1.3.0"],
    ["1.2.3-alpha.0.beta", "patch", "dev", undefined, "1.2.3"],
    ["1.2.4", "prerelease", "dev", undefined, "1.2.5-dev.0"],
    ["1.2.3-0", "prerelease", "dev", undefined, "1.2.3-dev.0"],
    ["1.2.3-alpha.0", "prerelease", "dev", undefined, "1.2.3-dev.0"],
    ["1.2.3-alpha.0", "prerelease", "alpha", undefined, "1.2.3-alpha.1"],
    ["1.2.3-alpha.0.beta", "prerelease", "dev", undefined, "1.2.3-dev.0"],
    [
      "1.2.3-alpha.0.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.1.beta",
    ],
    ["1.2.3-alpha.10.0.beta", "prerelease", "dev", undefined, "1.2.3-dev.0"],
    [
      "1.2.3-alpha.10.0.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.1.beta",
    ],
    [
      "1.2.3-alpha.10.1.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.2.beta",
    ],
    [
      "1.2.3-alpha.10.2.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.3.beta",
    ],
    [
      "1.2.3-alpha.10.beta.0",
      "prerelease",
      "dev",
      undefined,
      "1.2.3-dev.0",
    ],
    [
      "1.2.3-alpha.10.beta.0",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.beta.1",
    ],
    [
      "1.2.3-alpha.10.beta.1",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.beta.2",
    ],
    [
      "1.2.3-alpha.10.beta.2",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.beta.3",
    ],
    [
      "1.2.3-alpha.9.beta",
      "prerelease",
      "dev",
      undefined,
      "1.2.3-dev.0",
    ],
    [
      "1.2.3-alpha.9.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.10.beta",
    ],
    [
      "1.2.3-alpha.10.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.11.beta",
    ],
    [
      "1.2.3-alpha.11.beta",
      "prerelease",
      "alpha",
      undefined,
      "1.2.3-alpha.12.beta",
    ],
    [
      "1.2.0",
      "prepatch",
      "dev",
      undefined,
      "1.2.1-dev.0",
    ],
    ["1.2.0-1", "prepatch", "dev", undefined, "1.2.1-dev.0"],
    ["1.2.0", "preminor", "dev", undefined, "1.3.0-dev.0"],
    ["1.2.3-1", "preminor", "dev", undefined, "1.3.0-dev.0"],
    ["1.2.0", "premajor", "dev", undefined, "2.0.0-dev.0"],
    ["1.2.3-1", "premajor", "dev", undefined, "2.0.0-dev.0"],
    ["1.2.0-1", "minor", "dev", undefined, "1.2.0"],
    ["1.2.3-dev.bar", "prerelease", "dev", undefined, "1.2.3-dev.0"],

    // Add build metadata
    ["1.2.3", "major", undefined, "1", "2.0.0+1"],
    ["1.2.3", "minor", undefined, "1", "1.3.0+1"],
    ["1.2.3", "patch", undefined, "1", "1.2.4+1"],
    ["1.2.3", "premajor", "dev", "1", "2.0.0-dev.0+1"],
    ["1.2.3", "preminor", "dev", "1", "1.3.0-dev.0+1"],
    ["1.2.3", "prepatch", "dev", "1", "1.2.4-dev.0+1"],
    ["1.2.3", "pre", "dev", "1", "1.2.3-dev.0+1"],

    // Update build metadata
    ["1.2.3+1", "major", undefined, "2", "2.0.0+2"],
    ["1.2.3+1", "minor", undefined, "2", "1.3.0+2"],
    ["1.2.3+1", "patch", undefined, "2", "1.2.4+2"],
    ["1.2.3+1", "premajor", "dev", "2", "2.0.0-dev.0+2"],
    ["1.2.3+1", "preminor", "dev", "2", "1.3.0-dev.0+2"],
    ["1.2.3+1", "prepatch", "dev", "2", "1.2.4-dev.0+2"],

    // Retain build metadata
    ["1.2.3+1", "major", undefined, undefined, "2.0.0+1"],
    ["1.2.3+1", "minor", undefined, undefined, "1.3.0+1"],
    ["1.2.3+1", "patch", undefined, undefined, "1.2.4+1"],
    ["1.2.3+1", "premajor", "dev", undefined, "2.0.0-dev.0+1"],
    ["1.2.3+1", "preminor", "dev", undefined, "1.3.0-dev.0+1"],
    ["1.2.3+1", "prepatch", "dev", undefined, "1.2.4-dev.0+1"],
    ["1.2.3+1", "pre", "dev", undefined, "1.2.3-dev.0+1"],

    // // Remove build metadata
    ["1.2.3+1", "major", undefined, "", "2.0.0"],
    ["1.2.3+1", "minor", undefined, "", "1.3.0"],
    ["1.2.3+1", "patch", undefined, "", "1.2.4"],
    ["1.2.3+1", "premajor", "dev", "", "2.0.0-dev.0"],
    ["1.2.3+1", "preminor", "dev", "", "1.3.0-dev.0"],
    ["1.2.3+1", "prepatch", "dev", "", "1.2.4-dev.0"],
    ["1.2.3+1", "pre", "dev", "", "1.2.3-dev.0"],
  ];

  for (const [original, op, identifier, metadata, expected] of versions) {
    await t.step({
      name: `${original} ${op}+(${identifier}, ${metadata}) -> ${expected}`,
      fn: () => {
        const version = parse(original);
        const actual = increment(version, op, identifier, metadata);
        assertEquals(format(actual), expected);
      },
    });
  }
});
