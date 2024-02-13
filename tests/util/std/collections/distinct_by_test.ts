// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { distinctBy } from "./distinct_by.ts";

function distinctByTest<I>(
  array: Array<I>,
  selector: (element: I) => unknown,
  expected: Array<I>,
  message?: string,
) {
  const actual = distinctBy(array, selector);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/distinctBy] identities on empty array",
  fn() {
    distinctByTest(
      [],
      () => {},
      [],
    );
  },
});

Deno.test({
  name: "[collections/distinctBy] gets head on noop selector",
  fn() {
    distinctByTest(
      [25, "asdf", true],
      () => {},
      [25],
    );
  },
});

Deno.test({
  name:
    "[collections/distinctBy] removes duplicates and preserves order on identity",
  fn() {
    distinctByTest(
      [true, "asdf", 4, "asdf", true],
      (it) => it,
      [true, "asdf", 4],
    );
    distinctByTest(
      [null, undefined, null, "foo", undefined],
      (it) => it,
      [null, undefined, "foo"],
    );
  },
});

Deno.test({
  name: "[collections/distinctBy] does not check for deep equality on identity",
  fn() {
    const objects = [{ foo: "bar" }, { foo: "bar" }];
    distinctByTest(
      objects,
      (it) => it,
      objects,
    );

    const arrays = [[], []];
    distinctByTest(
      arrays,
      (it) => it,
      arrays,
    );

    const nans = [NaN, NaN];
    distinctByTest(
      nans,
      (it) => it,
      [nans[0]],
    );

    const noops = [() => {}, () => {}];
    distinctByTest(
      noops,
      (it) => it,
      noops,
    );

    const sets = [new Set(), new Set()];
    distinctByTest(
      sets,
      (it) => it,
      sets,
    );
  },
});

Deno.test({
  name:
    "[collections/distinctBy] distincts by selected value and preserves order",
  fn() {
    const kim = { name: "Kim", age: 22 };
    const arthur = { name: "Arthur", age: 22 };
    const anna = { name: "Anna", age: 48 };
    const karl = { name: "Karl", age: 48 };
    const people = [kim, arthur, anna, karl];

    distinctByTest(
      people,
      (it) => it.name.charAt(0),
      [kim, arthur],
    );
    distinctByTest(
      people,
      (it) => it.age,
      [kim, anna],
    );
    distinctByTest(
      people,
      (it) => it.name.length,
      [kim, arthur, anna],
    );
  },
});
