// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError, assertNotStrictEquals, assertThrows } from "./mod.ts";

Deno.test({
  name: "strictly unequal pass case",
  fn() {
    assertNotStrictEquals(true, false);
    assertNotStrictEquals(10, 11);
    assertNotStrictEquals("abc", "xyz");
    assertNotStrictEquals<unknown>(1, "1");
    assertNotStrictEquals(-0, +0);

    const xs = [1, false, "foo"];
    const ys = [1, true, "bar"];
    assertNotStrictEquals(xs, ys);

    const x = { a: 1 };
    const y = { a: 2 };
    assertNotStrictEquals(x, y);
  },
});

Deno.test({
  name: "strictly unequal fail case",
  fn() {
    assertThrows(() => assertNotStrictEquals(1, 1), AssertionError);
    assertThrows(() => assertNotStrictEquals(NaN, NaN), AssertionError);
  },
});
