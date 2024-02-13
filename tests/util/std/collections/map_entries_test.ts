// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { mapEntries } from "./map_entries.ts";

function mapEntriesTest<T, O>(
  input: [Record<string, T>, (entry: [string, T]) => [string, O]],
  expected: Record<string, O>,
  message?: string,
) {
  const actual = mapEntries(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/mapEntries] no mutation",
  fn() {
    const object = { a: 5, b: true };
    mapEntries(object, ([key, value]) => [`${key}a`, value]);

    assertEquals(object, { a: 5, b: true });
  },
});

Deno.test({
  name: "[collections/mapEntries] empty input",
  fn() {
    mapEntriesTest(
      [{}, (it) => it],
      {},
    );
  },
});

Deno.test({
  name: "[collections/mapEntries] identity",
  fn() {
    mapEntriesTest(
      [
        {
          foo: true,
          bar: "lorem",
          1: -5,
        },
        (it) => it,
      ],
      {
        foo: true,
        bar: "lorem",
        1: -5,
      },
    );
  },
});

Deno.test({
  name: "[collections/mapEntries] to constant entry",
  fn() {
    mapEntriesTest(
      [
        { test: "foo", "": [] },
        () => ["b", 5],
      ],
      { b: 5 },
    );
  },
});

Deno.test({
  name: "[collections/mapEntries] overlapping keys",
  fn() {
    mapEntriesTest(
      [
        {
          "Anna": 22,
          "Kim": 24,
          "Karen": 33,
          "Claudio": 11,
          "Karl": 45,
        },
        ([name, age]) => [name.charAt(0), age - 10],
      ],
      {
        "A": 12,
        "K": 35,
        "C": 1,
      },
    );
    mapEntriesTest(
      [
        {
          "ad04": "Kim",
          "e5f1": "Marija",
          "10a8": "Kim",
        },
        ([key, value]) => [value, key],
      ],
      {
        "Kim": "10a8",
        "Marija": "e5f1",
      },
    );
  },
});

Deno.test({
  name: "[collections/mapEntries] normal mappers",
  fn() {
    mapEntriesTest(
      [
        {
          "Mine": "Aliens",
          "Umse": "Feiert das",
          "ZM": "36 Grad",
        },
        ([artist, track]) => [artist, `${artist} - ${track}`],
      ],
      {
        "Mine": "Mine - Aliens",
        "Umse": "Umse - Feiert das",
        "ZM": "ZM - 36 Grad",
      },
    );
    mapEntriesTest(
      [
        {
          "ad04": { name: "Object A", type: "Asset" },
          "e5f1": { name: "Onboarding", type: "Process" },
          "10a8": { name: "Marija", type: "Employee" },
        },
        ([_, value]) => [value.type, value],
      ],
      {
        "Asset": { name: "Object A", type: "Asset" },
        "Process": { name: "Onboarding", type: "Process" },
        "Employee": { name: "Marija", type: "Employee" },
      },
    );
  },
});
