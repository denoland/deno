// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { firstNotNullishOf } from "./first_not_nullish_of.ts";

function firstNotNullishOfTest<T, O>(
  input: [Array<T>, (el: T) => O | undefined | null],
  expected: NonNullable<O> | undefined,
  message?: string,
) {
  const actual = firstNotNullishOf(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/firstNotNullishOf] no mutation",
  fn() {
    const array = [1, 2, 3, 4];
    firstNotNullishOf(array, (it) => it * 2);

    assertEquals(array, [1, 2, 3, 4]);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] empty input",
  fn() {
    firstNotNullishOfTest([[], (it) => it], undefined);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] all items nullish",
  fn() {
    firstNotNullishOfTest([[undefined, null], (it) => it], undefined);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] identity",
  fn() {
    firstNotNullishOfTest([[[], 1, 3], (it) => it], []);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] array of array",
  fn() {
    firstNotNullishOfTest([[[, 0], [null], ["Kim"]], (it) => it[0]], "Kim");
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] falsy values",
  fn() {
    firstNotNullishOfTest([[undefined, false, null], (it) => it], false);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] mappers without nullish values",
  fn() {
    firstNotNullishOfTest(
      [["Anna", "Kim", "Hans"], (it) => it.charAt(0)],
      "A",
    );
    firstNotNullishOfTest([[3, 4, 5, 6], (it) => it * 2], 6);
  },
});

Deno.test({
  name: "[collections/firstNotNullishOf] mappers with nullish values",
  fn() {
    firstNotNullishOfTest(
      [
        [
          { first: "Kim", middle: undefined, last: "Example" },
          { first: "Arthur", middle: "Hans", last: "Somename" },
          { first: "Laura", middle: "Marija", last: "Anothername" },
          { first: "Sam", middle: null, last: "Smith" },
        ],
        (it) => it.middle,
      ],
      "Hans",
    );
  },
});
