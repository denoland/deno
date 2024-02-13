// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { mapNotNullish } from "./map_not_nullish.ts";

function mapNotNullishTest<T, O>(
  input: [Array<T>, (el: T) => O | undefined | null],
  expected: Array<O>,
  message?: string,
) {
  const actual = mapNotNullish(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/mapNotNullish] no mutation",
  fn() {
    const array = [1, 2, 3, 4];
    mapNotNullish(array, (it) => it * 2);

    assertEquals(array, [1, 2, 3, 4]);
  },
});

Deno.test({
  name: "[collections/mapNotNullish] empty input",
  fn() {
    mapNotNullishTest(
      [[], (it) => it],
      [],
    );
  },
});

Deno.test({
  name: "[collections/mapNotNullish] identity",
  fn() {
    mapNotNullishTest(
      [
        [[], 1, 3],
        (it) => it,
      ],
      [[], 1, 3],
    );
  },
});

Deno.test({
  name: "[collections/mapNotNullish] mappers without nullish values",
  fn() {
    mapNotNullishTest(
      [
        ["Anna", "Kim", "Hans"],
        (it) => it.charAt(0),
      ],
      ["A", "K", "H"],
    );
    mapNotNullishTest(
      [
        [3, 4, 5, 6],
        (it) => it * 2,
      ],
      [6, 8, 10, 12],
    );
  },
});

Deno.test({
  name: "[collections/mapNotNullish] mappers with nullish values",
  fn() {
    mapNotNullishTest(
      [
        ["Errors: 5", "Success", "Warnings: 10", "..."],
        (it) =>
          it.match(/\w+: (?<numberOfProblems>\d+)/u)?.groups?.numberOfProblems,
      ],
      ["5", "10"],
    );
    mapNotNullishTest(
      [
        [
          { first: "Kim", middle: undefined, last: "Example" },
          { first: "Arthur", middle: "Hans", last: "Somename" },
          { first: "Laura", middle: "Marija", last: "Anothername" },
          { first: "Sam", middle: null, last: "Smith" },
        ],
        (it) => it.middle,
      ],
      ["Hans", "Marija"],
    );
  },
});
