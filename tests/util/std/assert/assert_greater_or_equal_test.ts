// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertGreaterOrEqual, assertThrows } from "./mod.ts";

Deno.test("assertGreaterOrEqual", () => {
  assertGreaterOrEqual(2, 1);
  assertGreaterOrEqual(1n, 1n);

  assertThrows(() => assertGreaterOrEqual(1, 2));
});
