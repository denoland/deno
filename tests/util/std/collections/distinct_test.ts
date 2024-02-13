// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { distinct } from "./distinct.ts";

function distinctTest<I>(
  input: Array<I>,
  expected: Array<I>,
  message?: string,
) {
  const actual = distinct(input);
  assertEquals(actual, expected, message);
}

Deno.test({
  name: "[collections/distinct] identities on empty array",
  fn() {
    distinctTest([], []);
  },
});

Deno.test({
  name: "[collections/distinct] removes duplicates and preserves order",
  fn() {
    distinctTest(
      [true, "asdf", 4, "asdf", true],
      [true, "asdf", 4],
    );
    distinctTest(
      [null, undefined, null, "foo", undefined],
      [null, undefined, "foo"],
    );
    distinctTest(
      [true, "asdf", 4, "asdf", true],
      [true, "asdf", 4],
    );
  },
});

Deno.test({
  name: "[collections/distinct] does not check for deep equality",
  fn() {
    const objects = [{ foo: "bar" }, { foo: "bar" }];
    distinctTest(objects, objects);

    const arrays = [[], []];
    distinctTest(arrays, arrays);

    const nans = [NaN, NaN];
    distinctTest(nans, [nans[0]]);

    const noops = [() => {}, () => {}];
    distinctTest(noops, noops);

    const sets = [new Set(), new Set()];
    distinctTest(sets, sets);
  },
});
