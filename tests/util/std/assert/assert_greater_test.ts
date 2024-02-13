// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertGreater, assertThrows } from "./mod.ts";

Deno.test("assertGreater", () => {
  assertGreater(2, 1);
  assertGreater(2n, 1n);
  assertGreater(1.1, 1);

  assertThrows(() => assertGreater(1, 2));
});
