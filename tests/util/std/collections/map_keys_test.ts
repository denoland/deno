// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { mapKeys } from "./map_keys.ts";

function mapKeysTest<T>(
  input: [Record<string, T>, (key: string) => string],
  expected: Record<string, T>,
  message?: string,
) {
  const actual = mapKeys(...input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/mapKeys] no mutation",
  fn() {
    const object = { a: 5, b: true };
    mapKeys(object, (it) => `${it}a`);

    assertEquals(object, { a: 5, b: true });
  },
});

Deno.test({
  name: "[collections/mapKeys] empty input",
  fn() {
    mapKeysTest(
      [{}, (it) => it],
      {},
    );
  },
});

Deno.test({
  name: "[collections/mapKeys] identity",
  fn() {
    mapKeysTest(
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
  name: "[collections/mapKeys] to constant key",
  fn() {
    mapKeysTest(
      [
        { test: "foo", "": [] },
        () => "a",
      ],
      { a: [] },
    );
  },
});

Deno.test({
  name: "[collections/mapKeys] overlapping keys",
  fn() {
    mapKeysTest(
      [
        {
          "Anna": 22,
          "Kim": 24,
          "Karen": 33,
          "Claudio": 11,
          "Karl": 45,
        },
        (name) => name.charAt(0),
      ],
      {
        "A": 22,
        "K": 45,
        "C": 11,
      },
    );
    mapKeysTest(
      [
        {
          "ad04": "foo",
          "ad28": "bar",
          "100f": "dino",
        },
        (it) => it.slice(0, 2),
      ],
      {
        "ad": "bar",
        "10": "dino",
      },
    );
  },
});

Deno.test({
  name: "[collections/mapKeys] empty key",
  fn() {
    mapKeysTest(
      [
        {
          "ab": 22,
          "a": 24,
          "bcd": 33,
          "d": 11,
        },
        (key) => key.slice(1),
      ],
      {
        "b": 22,
        "": 11,
        "cd": 33,
      },
    );
  },
});

Deno.test({
  name: "[collections/mapKeys] normal mappers",
  fn() {
    mapKeysTest(
      [
        {
          "/home/deno/food.txt": "Plants, preferably fruit",
          "/home/deno/other-dinos.txt": "Noderaptor, Pythonoctorus",
        },
        (path) => path.split("/").slice(-1)[0],
      ],
      {
        "food.txt": "Plants, preferably fruit",
        "other-dinos.txt": "Noderaptor, Pythonoctorus",
      },
    );
    mapKeysTest(
      [
        {
          "EUR": 1200,
          "USD": 1417,
          "JPY": 1563,
        },
        (currencyCode) =>
          ({ EUR: "Euro", USD: "US Dollar", JPY: "Japanese Yen" })[
            currencyCode
          ] ?? "_",
      ],
      {
        "Euro": 1200,
        "US Dollar": 1417,
        "Japanese Yen": 1563,
      },
    );
  },
});
