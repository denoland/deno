// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import { test, assert } from "./mod.ts";
import { red, green, white, gray, bold } from "../colors/mod.ts";
import { assertEqual } from "./pretty.ts";

const createHeader = (): string[] => [
  "",
  "",
  `    ${gray(bold("[Diff]"))} ${red(bold("Left"))} / ${green(bold("Right"))}`,
  "",
  ""
];

const added: (s: string) => string = (s: string): string => green(bold(s));
const removed: (s: string) => string = (s: string): string => red(bold(s));

test({
  name: "pass case",
  fn() {
    assertEqual({ a: 10 }, { a: 10 });
    assertEqual(true, true);
    assertEqual(10, 10);
    assertEqual("abc", "abc");
    assertEqual({ a: 10, b: { c: "1" } }, { a: 10, b: { c: "1" } });
  }
});

test({
  name: "failed with number",
  fn() {
    assert.throws(
      () => assertEqual(1, 2),
      Error,
      [...createHeader(), removed(`-   1`), added(`+   2`), ""].join("\n")
    );
  }
});

test({
  name: "failed with number vs string",
  fn() {
    assert.throws(
      () => assertEqual(1, "1"),
      Error,
      [...createHeader(), removed(`-   1`), added(`+   "1"`)].join("\n")
    );
  }
});

test({
  name: "failed with array",
  fn() {
    assert.throws(
      () => assertEqual([1, "2", 3], ["1", "2", 3]),
      Error,
      [
        ...createHeader(),
        white("    Array ["),
        removed(`-     1,`),
        added(`+     "1",`),
        white('      "2",'),
        white("      3,"),
        white("    ]"),
        ""
      ].join("\n")
    );
  }
});

test({
  name: "failed with object",
  fn() {
    assert.throws(
      () => assertEqual({ a: 1, b: "2", c: 3 }, { a: 1, b: 2, c: [3] }),
      Error,
      [
        ...createHeader(),
        white("    Object {"),
        white(`      "a": 1,`),
        added(`+     "b": 2,`),
        added(`+     "c": Array [`),
        added(`+       3,`),
        added(`+     ],`),
        removed(`-     "b": "2",`),
        removed(`-     "c": 3,`),
        white("    }"),
        ""
      ].join("\n")
    );
  }
});
