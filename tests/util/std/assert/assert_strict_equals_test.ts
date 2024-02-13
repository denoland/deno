// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError, assertStrictEquals, assertThrows } from "./mod.ts";

Deno.test({
  name: "strict types test",
  fn() {
    const x = { number: 2 };

    const y = x as Record<never, never>;
    const z = x as unknown;

    // y.number;
    //   ~~~~~~
    // Property 'number' does not exist on type 'Record<never, never>'.deno-ts(2339)

    assertStrictEquals(y, x);
    y.number; // ok

    // z.number;
    // ~
    // Object is of type 'unknown'.deno-ts(2571)

    assertStrictEquals(z, x);
    z.number; // ok
  },
});

Deno.test({
  name: "strict pass case",
  fn() {
    assertStrictEquals(true, true);
    assertStrictEquals(10, 10);
    assertStrictEquals("abc", "abc");
    assertStrictEquals(NaN, NaN);

    const xs = [1, false, "foo"];
    const ys = xs;
    assertStrictEquals(xs, ys);

    const x = { a: 1 };
    const y = x;
    assertStrictEquals(x, y);
  },
});

Deno.test({
  name: "strict failed with structure diff",
  fn() {
    assertThrows(
      () => assertStrictEquals({ a: 1, b: 2 }, { a: 1, c: [3] }),
      AssertionError,
      `
    {
      a: 1,
+     c: [
+       3,
+     ],
-     b: 2,
    }`,
    );
  },
});

Deno.test({
  name: "strict failed with reference diff",
  fn() {
    assertThrows(
      () => assertStrictEquals({ a: 1, b: 2 }, { a: 1, b: 2 }),
      AssertionError,
      `Values have the same structure but are not reference-equal.

    {
      a: 1,
      b: 2,
    }`,
    );
  },
});

Deno.test({
  name: "strict failed with custom msg",
  fn() {
    assertThrows(
      () => assertStrictEquals({ a: 1 }, { a: 1 }, "CUSTOM MESSAGE"),
      AssertionError,
      `Values have the same structure but are not reference-equal: CUSTOM MESSAGE

    {
      a: 1,
    }`,
    );
  },
});
