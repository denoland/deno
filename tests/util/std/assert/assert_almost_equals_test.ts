// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertAlmostEquals, AssertionError, assertThrows } from "./mod.ts";

Deno.test("assert almost equals number", () => {
  //Default precision
  assertAlmostEquals(-0, +0);
  assertAlmostEquals(Math.PI, Math.PI);
  assertAlmostEquals(0.1 + 0.2, 0.3);
  assertAlmostEquals(NaN, NaN);
  assertAlmostEquals(Number.NaN, Number.NaN);
  assertThrows(() => assertAlmostEquals(1, 2));
  assertThrows(() => assertAlmostEquals(1, 1.1));

  //Higher precision
  assertAlmostEquals(0.1 + 0.2, 0.3, 1e-16);
  assertThrows(
    () => assertAlmostEquals(0.1 + 0.2, 0.3, 1e-17),
    AssertionError,
    `Expected actual: "${
      (
        0.1 + 0.2
      ).toExponential()
    }" to be close to "${(0.3).toExponential()}"`,
  );

  //Special cases
  assertAlmostEquals(Infinity, Infinity);
  assertThrows(
    () => assertAlmostEquals(0, Infinity),
    AssertionError,
    'Expected actual: "0" to be close to "Infinity"',
  );
  assertThrows(
    () => assertAlmostEquals(-Infinity, +Infinity),
    AssertionError,
    'Expected actual: "-Infinity" to be close to "Infinity"',
  );
  assertThrows(
    () => assertAlmostEquals(Infinity, NaN),
    AssertionError,
    'Expected actual: "Infinity" to be close to "NaN"',
  );
});
