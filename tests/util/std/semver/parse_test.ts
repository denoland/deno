// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows } from "../assert/mod.ts";
import { parse } from "./parse.ts";

Deno.test("major", async (t) => {
  // [range, version]
  // Version should be detectable despite extra characters
  const versions: [string, number][] = [
    ["1.2.3", 1],
    [" 1.2.3 ", 1],
    [" 2.2.3-4 ", 2],
    [" 3.2.3-pre ", 3],
    ["v5.2.3", 5],
    [" v8.2.3 ", 8],
    ["\t13.2.3", 13],
  ];

  for (const [v, expected] of versions) {
    await t.step(v, () => {
      const version = parse(v);
      assertEquals(version.major, expected);
    });
  }
});

Deno.test("minor", async (t) => {
  // [range, version]
  // Version should be detectable despite extra characters
  const versions: [string, number][] = [
    ["1.1.3", 1],
    [" 1.1.3 ", 1],
    [" 1.2.3-4 ", 2],
    [" 1.3.3-pre ", 3],
    ["v1.5.3", 5],
    [" v1.8.3 ", 8],
    ["\t1.13.3", 13],
  ];

  for (const [v, expected] of versions) {
    await t.step(v, () => {
      const version = parse(v);
      assertEquals(version.minor, expected);
    });
  }
});

Deno.test("patch", async (t) => {
  // [range, version]
  // Version should be detectable despite extra characters
  const versions: [string, number][] = [
    ["1.2.1", 1],
    [" 1.2.1 ", 1],
    [" 1.2.2-4 ", 2],
    [" 1.2.3-pre ", 3],
    ["v1.2.5", 5],
    [" v1.2.8 ", 8],
    ["\t1.2.13", 13],
  ];
  for (const [v, expected] of versions) {
    await t.step(``, () => {
      const semver = parse(v);
      const actual = semver.patch;
      assertEquals(actual, expected);
    });
  }
});

Deno.test("prerelease", async (t) => {
  // [prereleaseParts, version]
  const versions: [string, (string | number)[]][] = [
    ["1.2.2-alpha.1", ["alpha", 1]],
    ["0.6.1-1", [1]],
    ["1.0.0-beta.2", ["beta", 2]],
    ["v0.5.4-pre", ["pre"]],
    ["1.2.2-alpha.1", ["alpha", 1]],
    ["1.2.0-1b3-4", ["1b3-4"]],
    ["1.2.0-3.6-pre2", [3, "6-pre2"]],
    ["2.0.0", []],
  ];

  for (const [v, expected] of versions) {
    await t.step(`${v} : ${JSON.stringify(expected)}`, () => {
      const semver = parse(v);
      assertEquals(
        semver.prerelease,
        expected,
      );
    });
  }
});

Deno.test({
  name: "badVersions",
  fn: async (t) => {
    const versions: [unknown][] = [
      ["1.2." + new Array(256).join("1")], // too long
      ["1.2." + new Array(100).join("1")], // too big
      [null],
      [undefined],
      [{}],
      [[]],
      [false],
      [true],
      [0],
      [""],
      ["not a version"],
      ["∞.∞.∞"],
      ["NaN.NaN.NaN"],
    ];
    for (const [v] of versions) {
      await t.step(`${JSON.stringify(v)}`, () => {
        assertThrows(() => parse(v as string));
      });
    }
  },
});

Deno.test("invalidVersion", async (t) => {
  const versions = ["1.2.3.4", "NOT VALID", 1.2, null, "Infinity.NaN.Infinity"];

  for (const v of versions) {
    await t.step(`invalid ${v}`, () => {
      assertThrows(
        function () {
          parse(v as string);
        },
        TypeError,
      );
    });
  }
});

Deno.test("bigNumericPrerelease", function () {
  const r = parse("1.2.3-beta." + Number.MAX_SAFE_INTEGER + "0");
  assertEquals(r.prerelease, ["beta", "90071992547409910"]);
});
