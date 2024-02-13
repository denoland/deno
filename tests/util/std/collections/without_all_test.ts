// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { withoutAll } from "./without_all.ts";

function withoutAllTest<I>(
  input: Array<I>,
  excluded: Array<I>,
  expected: Array<I>,
  message?: string,
) {
  const actual = withoutAll(input, excluded);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/withoutAll] no mutation",
  fn() {
    const array = [1, 2, 3, 4];
    withoutAll(array, [2, 3]);
    assertEquals(array, [1, 2, 3, 4]);
  },
});

Deno.test({
  name: "[collections/withoutAll] empty input",
  fn() {
    withoutAllTest([], [], []);
  },
});

Deno.test({
  name: "[collections/withoutAll] no matches",
  fn() {
    withoutAllTest([1, 2, 3, 4], [0, 7, 9], [1, 2, 3, 4]);
  },
});

Deno.test({
  name: "[collections/withoutAll] single matche",
  fn() {
    withoutAllTest([1, 2, 3, 4], [1], [2, 3, 4]);
    withoutAllTest([1, 2, 3, 2], [2], [1, 3]);
  },
});

Deno.test({
  name: "[collections/withoutAll] multiple matches",
  fn() {
    withoutAllTest([1, 2, 3, 4, 6, 3], [1, 2], [3, 4, 6, 3]);
    withoutAllTest([7, 2, 9, 8, 7, 6, 5, 7], [7, 9], [2, 8, 6, 5]);
  },
});

Deno.test({
  name: "[collection/withoutAll] leaves duplicate elements",
  fn() {
    withoutAllTest(
      Array.from({ length: 110 }, () => 3),
      [1],
      Array.from({ length: 110 }, () => 3),
    );
  },
});
