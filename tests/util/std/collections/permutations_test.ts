// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { permutations } from "./permutations.ts";

function permutationsTest<T>(
  input: [Array<T>],
  expected: Array<Array<T>>,
  message?: string,
) {
  const actual = permutations(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/permutations] no mutation",
  fn() {
    const array = [1, 2, 3];
    permutations(array);

    assertEquals(array, [1, 2, 3]);
  },
});

Deno.test({
  name: "[collections/permutations] empty input",
  fn() {
    permutationsTest(
      [[]],
      [],
    );
  },
});

Deno.test({
  name: "[collections/permutations] one element",
  fn() {
    permutationsTest(
      [
        [true],
      ],
      [[true]],
    );
    permutationsTest(
      [
        [undefined],
      ],
      [[undefined]],
    );
  },
});

Deno.test({
  name: "[collections/permutations] equality is ignored",
  fn() {
    permutationsTest(
      [[1, 1]],
      [[1, 1], [1, 1]],
    );
  },
});

Deno.test({
  name: "[collections/permutations] examples",
  fn() {
    permutationsTest(
      [["a", "b", "c"]],
      [
        ["a", "b", "c"],
        ["b", "a", "c"],
        ["c", "a", "b"],
        ["a", "c", "b"],
        ["b", "c", "a"],
        ["c", "b", "a"],
      ],
    );
    permutationsTest(
      [[true, false, true]],
      [
        [true, false, true],
        [false, true, true],
        [true, true, false],
        [true, true, false],
        [false, true, true],
        [true, false, true],
      ],
    );
  },
});
