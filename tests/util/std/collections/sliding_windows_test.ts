// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals, assertThrows } from "../assert/mod.ts";
import { slidingWindows } from "./sliding_windows.ts";

function slidingWindowsTest<T>(
  input: [
    collection: T[],
    size: number,
    config?: { step?: number; partial?: boolean },
  ],
  expected: T[][],
  message?: string,
) {
  const actual = slidingWindows(...input);
  assertEquals(actual, expected, message);
}

function slidingWindowsThrowsTest<T>(
  input: [
    collection: T[],
    size: number,
    config?: { step?: number; partial?: boolean },
  ],
  ErrorClass: ErrorConstructor,
  msgIncludes?: string,
  msg?: string | undefined,
) {
  assertThrows(
    () => {
      slidingWindows(...input);
    },
    ErrorClass,
    msgIncludes,
    msg,
  );
}

Deno.test({
  name: "[collections/slidingWindows] no mutation",
  fn() {
    const numbers = [1, 2, 3, 4, 5];
    slidingWindows(numbers, 3);
    assertEquals(numbers, [1, 2, 3, 4, 5]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] empty input",
  fn() {
    slidingWindowsTest([[], 3], []);
    slidingWindowsTest([[], 3, {}], []);
    slidingWindowsTest([[], 3, { step: 2 }], []);
    slidingWindowsTest([[], 3, { partial: true }], []);
    slidingWindowsTest([[], 3, { step: 2, partial: true }], []);
  },
});

Deno.test({
  name: "[collections/slidingWindows] default option",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 5], [
      [1, 2, 3, 4, 5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 3], [
      [1, 2, 3],
      [2, 3, 4],
      [3, 4, 5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 1], [
      [1],
      [2],
      [3],
      [4],
      [5],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] step option",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 5, { step: 2 }], [
      [1, 2, 3, 4, 5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 3, { step: 2 }], [
      [1, 2, 3],
      [3, 4, 5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 1, { step: 2 }], [
      [1],
      [3],
      [5],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] partial option",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 5, { partial: true }], [
      [1, 2, 3, 4, 5],
      [2, 3, 4, 5],
      [3, 4, 5],
      [4, 5],
      [5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 3, { partial: true }], [
      [1, 2, 3],
      [2, 3, 4],
      [3, 4, 5],
      [4, 5],
      [5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 1, { partial: true }], [
      [1],
      [2],
      [3],
      [4],
      [5],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] step and partial option",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 5, { step: 2, partial: true }], [
      [1, 2, 3, 4, 5],
      [3, 4, 5],
      [5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 3, { step: 2, partial: true }], [
      [1, 2, 3],
      [3, 4, 5],
      [5],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 1, { step: 2, partial: true }], [
      [1],
      [3],
      [5],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] invalid size or step: other than number",
  fn() {
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], NaN],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: NaN }],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      // @ts-ignore: for test
      [[1, 2, 3, 4, 5], "invalid"],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      // @ts-ignore: for test
      [[1, 2, 3, 4, 5], 3, { step: "invalid" }],
      RangeError,
      "Both size and step must be positive integer.",
    );
  },
});

Deno.test({
  name: "[collections/slidingWindows] invalid size or step: not integer number",
  fn() {
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 0.5],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: 0.5 }],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 1.5],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: 1.5 }],
      RangeError,
      "Both size and step must be positive integer.",
    );
  },
});

Deno.test({
  name:
    "[collections/slidingWindows] invalid size or step: not positive number",
  fn() {
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 0],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: 0 }],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], -1],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: -1 }],
      RangeError,
      "Both size and step must be positive integer.",
    );
  },
});

Deno.test({
  name: "[collections/slidingWindows] invalid size or step: infinity",
  fn() {
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], Number.NEGATIVE_INFINITY],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: Number.NEGATIVE_INFINITY }],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], Number.POSITIVE_INFINITY],
      RangeError,
      "Both size and step must be positive integer.",
    );
    slidingWindowsThrowsTest(
      [[1, 2, 3, 4, 5], 3, { step: Number.POSITIVE_INFINITY }],
      RangeError,
      "Both size and step must be positive integer.",
    );
  },
});

Deno.test({
  name: "[collections/slidingWindows] large size",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 100], []);
    slidingWindowsTest([[1, 2, 3, 4, 5], 100, { step: 2 }], []);
    slidingWindowsTest([[1, 2, 3, 4, 5], 100, { step: 2, partial: true }], [
      [1, 2, 3, 4, 5],
      [3, 4, 5],
      [5],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] large step",
  fn() {
    slidingWindowsTest([[1, 2, 3, 4, 5], 3, { step: 100 }], [
      [1, 2, 3],
    ]);
    slidingWindowsTest([[1, 2, 3, 4, 5], 3, { step: 100, partial: true }], [
      [1, 2, 3],
    ]);
  },
});

Deno.test({
  name: "[collections/slidingWindows] empty Array",
  fn() {
    slidingWindowsTest([Array(5), 5], [
      Array(5),
    ]);
    slidingWindowsTest([Array(5), 3], [
      Array(3),
      Array(3),
      Array(3),
    ]);
    slidingWindowsTest([Array(5), 1], [
      Array(1),
      Array(1),
      Array(1),
      Array(1),
      Array(1),
    ]);
  },
});
