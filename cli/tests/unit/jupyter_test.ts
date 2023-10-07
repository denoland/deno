// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertThrows } from "./test_util.ts";

Deno.test("Deno.jupyter is not available", () => {
  assertThrows(
    () => Deno.jupyter,
    "Deno.jupyter is only available in `deno jupyter` subcommand.",
  );
});
