// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { chunk } from "./chunk.ts";

function chunkTest<I>(
  input: [Array<I>, number],
  expected: Array<Array<I>>,
  message?: string,
) {
  const actual = chunk(...input);
  assertEquals(actual, expected, message);
}

const testArray = [1, 2, 3, 4, 5, 6];

Deno.test({
  name: "[collections/chunk] no mutation",
  fn() {
    const array = [1, 2, 3, 4];
    chunk(array, 2);

    assertEquals(array, [1, 2, 3, 4]);
  },
});

Deno.test({
  name: "[collections/chunk] throws on non naturals",
  fn() {
    assertThrows(() => chunk([], +.5));
    assertThrows(() => chunk([], -4.7));
    assertThrows(() => chunk([], -2));
    assertThrows(() => chunk([], +0));
    assertThrows(() => chunk([], -0));
  },
});

Deno.test({
  name: "[collections/chunk] empty input",
  fn() {
    chunkTest(
      [[], 1],
      [],
    );
  },
});

Deno.test({
  name: "[collections/chunk] single element chunks",
  fn() {
    chunkTest(
      [testArray, 1],
      testArray.map((it) => [it]),
    );
    chunkTest(
      [["foo"], 1],
      [["foo"]],
    );
  },
});

Deno.test({
  name: "[collections/chunk] n chunks fitting",
  fn() {
    chunkTest(
      [testArray, 2],
      [[1, 2], [3, 4], [5, 6]],
    );
    chunkTest(
      [testArray, 3],
      [[1, 2, 3], [4, 5, 6]],
    );
  },
});

Deno.test({
  name: "[collections/chunk] n chunks not fitting",
  fn() {
    chunkTest(
      [testArray, 4],
      [[1, 2, 3, 4], [5, 6]],
    );
    chunkTest(
      [testArray, 5],
      [[1, 2, 3, 4, 5], [6]],
    );
  },
});

Deno.test({
  name: "[collections/chunk] chunks equal to length",
  fn() {
    chunkTest(
      [testArray, testArray.length],
      [testArray],
    );
    chunkTest(
      [["foo"], 1],
      [["foo"]],
    );
  },
});
