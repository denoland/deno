// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { associateBy } from "./associate_by.ts";

function associateByTest<T>(
  input: [Array<T>, (el: T) => string],
  expected: { [x: string]: T },
  message?: string,
) {
  const actual = associateBy(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/associateBy] no mutation",
  fn() {
    const arrayA = ["Foo", "Bar"];
    associateBy(arrayA, (it) => it.charAt(0));

    assertEquals(arrayA, ["Foo", "Bar"]);
  },
});

Deno.test({
  name: "[collections/associateBy] empty input",
  fn() {
    associateByTest(
      [[], () => "a"],
      {},
    );
  },
});

Deno.test({
  name: "[collections/associateBy] constant key",
  fn() {
    associateByTest(
      [[1, 3, 5, 6], () => "a"],
      { a: 6 },
    );
  },
});

Deno.test({
  name: "[collections/associateBy] empty key",
  fn() {
    associateByTest(
      [
        ["Foo", "b"],
        (it) => it.charAt(1),
      ],
      {
        "o": "Foo",
        "": "b",
      },
    );
  },
});

Deno.test({
  name: "[collections/associateBy] duplicate keys",
  fn() {
    associateByTest(
      [
        ["Anna", "Marija", "Karl", "Arnold", "Martha"],
        (it) => it.charAt(0),
      ],
      {
        "A": "Arnold",
        "M": "Martha",
        "K": "Karl",
      },
    );
    associateByTest(
      [
        [1.2, 2, 2.3, 6.3, 6.9, 6],
        (it) => Math.floor(it).toString(),
      ],
      {
        "1": 1.2,
        "2": 2.3,
        "6": 6,
      },
    );
  },
});

Deno.test({
  name: "[collections/associateBy] associates",
  fn() {
    associateByTest(
      [
        [
          { name: "test", done: false },
          { name: "build", done: true },
          { name: "deploy", done: false },
          { name: "audit", done: true },
        ],
        (it) => it.name,
      ],
      {
        "test": { name: "test", done: false },
        "build": { name: "build", done: true },
        "deploy": { name: "deploy", done: false },
        "audit": { name: "audit", done: true },
      },
    );
    associateByTest(
      [
        [
          "anna@example.org",
          "josh@example.org",
          "kim@example.org",
        ],
        (it) => it.substring(0, it.indexOf("@")),
      ],
      {
        "anna": "anna@example.org",
        "josh": "josh@example.org",
        "kim": "kim@example.org",
      },
    );
  },
});
