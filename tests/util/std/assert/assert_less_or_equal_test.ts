// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertLessOrEqual, assertThrows } from "./mod.ts";

Deno.test("assertLessOrEqual", () => {
  assertLessOrEqual(1, 2);
  assertLessOrEqual(1n, 1n);

  assertThrows(() => assertLessOrEqual(2, 1));
});
