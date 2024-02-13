// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { sortBy } from "./sort_by.ts";

Deno.test({
  name: "[collections/sortBy] no mutation",
  fn() {
    const array = ["a", "abc", "ba"];
    sortBy(array, (it) => it.length);

    assertEquals(array, ["a", "abc", "ba"]);
  },
});

Deno.test({
  name: "[collections/sortBy] calls the selector function once",
  fn() {
    let callCount = 0;
    const array = [0, 1, 2];
    sortBy(array, (it) => {
      callCount++;
      return it;
    });

    assertEquals(callCount, array.length);
  },
});

Deno.test({
  name: "[collections/sortBy] empty input",
  fn() {
    assertEquals(sortBy([], () => 5), []);
  },
});

Deno.test({
  name: "[collections/sortBy] identity selector",
  fn() {
    assertEquals(sortBy([2, 3, 1], (it) => it), [1, 2, 3]);
  },
});

Deno.test({
  name: "[collections/sortBy] stable sort",
  fn() {
    assertEquals(
      sortBy([
        { id: 1, date: "February 1, 2022" },
        { id: 2, date: "December 17, 1995" },
        { id: 3, date: "June 12, 2012" },
        { id: 4, date: "December 17, 1995" },
        { id: 5, date: "June 12, 2012" },
      ], (it) => new Date(it.date)),
      [
        { id: 2, date: "December 17, 1995" },
        { id: 4, date: "December 17, 1995" },
        { id: 3, date: "June 12, 2012" },
        { id: 5, date: "June 12, 2012" },
        { id: 1, date: "February 1, 2022" },
      ],
    );

    assertEquals(
      sortBy([
        { id: 1, str: "c" },
        { id: 2, str: "a" },
        { id: 3, str: "b" },
        { id: 4, str: "a" },
        { id: 5, str: "b" },
      ], (it) => it.str),
      [
        { id: 2, str: "a" },
        { id: 4, str: "a" },
        { id: 3, str: "b" },
        { id: 5, str: "b" },
        { id: 1, str: "c" },
      ],
    );
  },
});

Deno.test({
  name: "[collections/sortBy] special number values",
  fn() {
    assertEquals(
      sortBy([
        1,
        Number.POSITIVE_INFINITY,
        2,
        Number.NEGATIVE_INFINITY,
        3,
        Number.NaN,
        4,
        Number.NaN,
      ], (it) => it),
      [
        Number.NEGATIVE_INFINITY,
        1,
        2,
        3,
        4,
        Number.POSITIVE_INFINITY,
        Number.NaN,
        Number.NaN,
      ],
    );

    assertEquals(
      sortBy([
        Number.NaN,
        1,
        Number.POSITIVE_INFINITY,
        Number.NaN,
        7,
        Number.NEGATIVE_INFINITY,
        Number.NaN,
        2,
        6,
        5,
        9,
      ], (it) => it),
      [
        Number.NEGATIVE_INFINITY,
        1,
        2,
        5,
        6,
        7,
        9,
        Number.POSITIVE_INFINITY,
        Number.NaN,
        Number.NaN,
        Number.NaN,
      ],
    );

    // Test that NaN sort is stable.
    const nanArray = [
      { id: 1, nan: Number.NaN },
      { id: 2, nan: Number.NaN },
      { id: 3, nan: Number.NaN },
      { id: 4, nan: Number.NaN },
    ];
    assertEquals(sortBy(nanArray, ({ nan }) => nan), nanArray);
  },
});

Deno.test({
  name: "[collections/sortBy] sortings",
  fn() {
    const testArray = [
      { name: "benchmark", stage: 3 },
      { name: "test", stage: 2 },
      { name: "build", stage: 1 },
      { name: "deploy", stage: 4 },
    ];

    assertEquals(sortBy(testArray, (it) => it.stage), [
      { name: "build", stage: 1 },
      { name: "test", stage: 2 },
      { name: "benchmark", stage: 3 },
      { name: "deploy", stage: 4 },
    ]);

    assertEquals(sortBy(testArray, (it) => it.name), [
      { name: "benchmark", stage: 3 },
      { name: "build", stage: 1 },
      { name: "deploy", stage: 4 },
      { name: "test", stage: 2 },
    ]);

    assertEquals(
      sortBy([
        "9007199254740999",
        "9007199254740991",
        "9007199254740995",
      ], (it) => BigInt(it)),
      [
        "9007199254740991",
        "9007199254740995",
        "9007199254740999",
      ],
    );

    assertEquals(
      sortBy([
        "February 1, 2022",
        "December 17, 1995",
        "June 12, 2012",
      ], (it) => new Date(it)),
      [
        "December 17, 1995",
        "June 12, 2012",
        "February 1, 2022",
      ],
    );
  },
});

Deno.test({
  name: "[collections/sortBy] desc ordering",
  fn() {
    assertEquals(
      sortBy(
        [
          "January 27, 1995",
          "November 26, 2020",
          "June 17, 1952",
          "July 15, 1993",
        ],
        (it) => new Date(it),
        { order: "desc" },
      ),
      [
        "November 26, 2020",
        "January 27, 1995",
        "July 15, 1993",
        "June 17, 1952",
      ],
    );
  },
});
