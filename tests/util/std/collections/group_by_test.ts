// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { groupBy } from "./group_by.ts";

function groupByTest<T>(
  input: [Array<T>, (el: T) => PropertyKey],
  expected: { [x: string]: Array<T> },
  message?: string,
) {
  const actual = groupBy(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/groupBy] no mutation",
  fn() {
    const arrayA = [1.1, 4.2, 4.5];
    groupBy(arrayA, () => "test");

    assertEquals(arrayA, [1.1, 4.2, 4.5]);
  },
});

Deno.test({
  name: "[collections/groupBy] empty input",
  fn() {
    groupByTest(
      [[], () => "a"],
      {},
    );
  },
});

Deno.test({
  name: "[collections/groupBy] constant key",
  fn() {
    groupByTest(
      [[1, 3, 5, 6], () => "a"],
      { a: [1, 3, 5, 6] },
    );
  },
});
Deno.test({
  name: "[collections/groupBy] non-string key",
  fn() {
    groupByTest(
      [
        [
          { number: 1, name: "a" },
          { number: 1, name: "b" },
          { number: 2, name: "c" },
          { number: 3, name: "d" },
        ],
        ({ number }) => number,
      ],
      {
        1: [{ number: 1, name: "a" }, { number: 1, name: "b" }],
        2: [{ number: 2, name: "c" }],
        3: [{ number: 3, name: "d" }],
      },
    );
  },
});

Deno.test({
  name: "[collections/groupBy] empty key",
  fn() {
    groupByTest(
      [
        ["Foo", "b"],
        (it) => it.charAt(1),
      ],
      {
        "o": ["Foo"],
        "": ["b"],
      },
    );
  },
});

Deno.test({
  name: "[collections/groupBy] groups",
  fn() {
    groupByTest(
      [
        ["Anna", "Marija", "Karl", "Arnold", "Martha"],
        (it) => it.charAt(0),
      ],
      {
        "A": ["Anna", "Arnold"],
        "M": ["Marija", "Martha"],
        "K": ["Karl"],
      },
    );
    groupByTest(
      [
        [1.2, 2, 2.3, 6.3, 6.9, 6],
        (it) => Math.floor(it).toString(),
      ],
      {
        "1": [1.2],
        "2": [2, 2.3],
        "6": [6.3, 6.9, 6],
      },
    );
  },
});

Deno.test({
  name: "[collections/groupBy] callback index",
  fn() {
    const actual = groupBy(
      ["a", "b", "c", "d"],
      (_, i) => i % 2 === 0 ? "even" : "odd",
    );

    const expected = { even: ["a", "c"], odd: ["b", "d"] };

    assertEquals(actual, expected);
  },
});

Deno.test({
  name: "[collections/groupBy] iterable input",
  fn() {
    function* count(): Generator<number, void> {
      for (let i = 0; i < 5; i += 1) yield i;
    }

    const actual = groupBy(count(), (n) => n % 2 === 0 ? "even" : "odd");
    const expected = { even: [0, 2, 4], odd: [1, 3] };

    assertEquals(actual, expected);
  },
});
