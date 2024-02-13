// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { partition } from "./partition.ts";

function partitionTest<I>(
  input: [Array<I>, (element: I) => boolean],
  expected: [Array<I>, Array<I>],
  message?: string,
) {
  const actual = partition(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/partition] no mutation",
  fn() {
    const array = [1, 2, 3];
    partition(array, (it) => it % 2 === 0);

    assertEquals(array, [1, 2, 3]);
  },
});

Deno.test({
  name: "[collections/partition] empty input",
  fn() {
    partitionTest(
      [[], () => true],
      [[], []],
    );
  },
});

Deno.test({
  name: "[collections/partition] all match",
  fn() {
    partitionTest(
      [[2, 4, 6], (it) => it % 2 === 0],
      [[2, 4, 6], []],
    );
    partitionTest(
      [["foo", "bar"], (it) => it.length > 0],
      [["foo", "bar"], []],
    );
  },
});

Deno.test({
  name: "[collections/partition] none match",
  fn() {
    partitionTest(
      [[3, 7, 5], (it) => it % 2 === 0],
      [[], [3, 7, 5]],
    );
    partitionTest(
      [["foo", "bar"], (it) => it.startsWith("z")],
      [[], ["foo", "bar"]],
    );
  },
});

Deno.test({
  name: "[collections/partition] some match",
  fn() {
    partitionTest(
      [[13, 4, 13, 8], (it) => it % 2 === 0],
      [[4, 8], [13, 13]],
    );
    partitionTest(
      [["foo", "bar", ""], (it) => it.length > 0],
      [["foo", "bar"], [""]],
    );
  },
});
