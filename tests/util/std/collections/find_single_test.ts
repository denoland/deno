// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { findSingle } from "./find_single.ts";

function findSingleTest<I>(
  input: [Array<I>, (element: I) => boolean],
  expected: I | undefined,
  message?: string,
) {
  const actual = findSingle(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/findSingle] no mutation",
  fn() {
    const array = [1, 2, 3];
    findSingle(array, (it) => it % 2 === 0);

    assertEquals(array, [1, 2, 3]);
  },
});

Deno.test({
  name: "[collections/findSingle] empty input",
  fn() {
    findSingleTest(
      [[], (_) => true],
      undefined,
    );
  },
});

Deno.test({
  name: "[collections/findSingle] only one element",
  fn() {
    findSingleTest(
      [[42], (_it) => true],
      42,
    );
    findSingleTest(
      [["foo"], (_it) => true],
      "foo",
    );
    findSingleTest(
      [[null], (_it) => true],
      null,
    );
    findSingleTest(
      [[undefined], (_it) => true],
      undefined,
    );
  },
});

Deno.test({
  name: "[collections/findSingle] no matches",
  fn() {
    findSingleTest(
      [[9, 11, 13], (it) => it % 2 === 0],
      undefined,
    );
    findSingleTest(
      [["foo", "bar"], (it) => it.startsWith("z")],
      undefined,
    );
    findSingleTest(
      [[{ done: false }], (it) => it.done],
      undefined,
    );
  },
});

Deno.test({
  name: "[collections/findSingle] only match",
  fn() {
    findSingleTest(
      [[9, 12, 13], (it) => it % 2 === 0],
      12,
    );
    findSingleTest(
      [["zap", "foo", "bar"], (it) => it.startsWith("z")],
      "zap",
    );
    findSingleTest(
      [[{ done: false }, { done: true }], (it) => it.done],
      { done: true },
    );
  },
});

Deno.test({
  name: "[collections/findSingle] multiple matches",
  fn() {
    findSingleTest(
      [[9, 12, 13, 14], (it) => it % 2 === 0],
      undefined,
    );
    findSingleTest(
      [["zap", "foo", "bar", "zee"], (it) => it.startsWith("z")],
      undefined,
    );
  },
});
