// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { maxWith } from "./max_with.ts";

function maxWithTest<T>(
  input: [T[], (a: T, b: T) => number],
  expected: T | undefined,
  message?: string,
) {
  const actual = maxWith(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/maxWith] no mutation",
  fn() {
    const input = [[1, 3], [6, 1, 3], [4]];
    maxWith(input, (a, b) => a.length - b.length);

    assertEquals(input, [[1, 3], [6, 1, 3], [4]]);
  },
});

Deno.test({
  name: "[collections/maxWith] empty input",
  fn() {
    maxWithTest<string>([[], (a, b) => a.length - b.length], undefined);
  },
});

Deno.test({
  name: "[collections/maxWith] array of arrays",
  fn() {
    maxWithTest([[[1, 3], [6, 1, 3], [4]], (a, b) => a.length - b.length], [
      6,
      1,
      3,
    ]);
  },
});

Deno.test({
  name: "[collections/maxWith] array of strings",
  fn() {
    maxWithTest(
      [["Kim", "Anna", "Arthur"], (a, b) => a.length - b.length],
      "Arthur",
    );
  },
});

Deno.test({
  name: "[collections/maxWith] array of objects",
  fn() {
    maxWithTest(
      [
        [
          { name: "Kim", age: 24 },
          { name: "Anna", age: 20 },
          { name: "John", age: 43 },
        ],
        (a, b) => a.age - b.age,
      ],
      { name: "John", age: 43 },
    );
  },
});

Deno.test({
  name: "[collections/maxWith] duplicates",
  fn() {
    maxWithTest(
      [["John", "Arthur", "Arthur"], (a, b) => a.length - b.length],
      "Arthur",
    );
  },
});

Deno.test({
  name: "[collections/maxWith] array containing undefined",
  fn() {
    maxWithTest(
      [
        [undefined, undefined, 1],
        (a, b) => {
          if (a === undefined) {
            return 1;
          }
          if (b === undefined) {
            return -1;
          }
          return 0;
        },
      ],
      undefined,
    );
  },
});
