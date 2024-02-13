// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert } from "../assert/mod.ts";
import { MAX, MIN } from "./constants.ts";
import { isSemVer } from "./is_semver.ts";

Deno.test({
  name: "invalid_semver",
  fn: async (t) => {
    let i = 0;
    const versions: [unknown][] = [
      [null],
      [undefined],
      [{}],
      [[]],
      [true],
      [false],
      [0],
      ["1.2.3"],
      [{ major: 0, minor: 0, patch: 0, prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: [] }],
      [{ major: 0, minor: 0, build: [], prerelease: [] }],
      [{ major: 0, patch: 0, build: [], prerelease: [] }],
      [{ minor: 0, patch: 0, build: [], prerelease: [] }],
      [{ major: "", minor: 0, patch: 0, build: [], prerelease: [] }],
      [{ major: 0, minor: "", patch: 0, build: [], prerelease: [] }],
      [{ major: 0, minor: 0, patch: "", build: [], prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: {}, prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: {} }],
      [{ major: 0, minor: 0, patch: 0, build: [{}], prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: [{}] }],
      [{ major: 0, minor: 0, patch: 0, build: [""], prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: [""] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: [-1] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: [Number.NaN] }],
    ];
    for (const [v] of versions) {
      await t.step(`invalid_${(i++).toString().padStart(2, "0")}`, () => {
        const actual = isSemVer(v);
        assert(!actual);
      });
    }
  },
});

Deno.test({
  name: "valid_semver",
  fn: async (t) => {
    let i = 0;
    const versions: [unknown][] = [
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: [] }],
      [{ extra: 1, major: 0, minor: 0, patch: 0, build: [], prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: ["abc"], prerelease: [] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: ["abc"] }],
      [{ major: 0, minor: 0, patch: 0, build: [], prerelease: ["abc", 0] }],
      [MIN],
      [MAX],
    ];
    for (const [v] of versions) {
      await t.step(`valid_${(i++).toString().padStart(2, "0")}`, () => {
        const actual = isSemVer(v);
        assert(actual);
      });
    }
  },
});
