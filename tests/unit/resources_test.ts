// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-deprecated-deno-api

import { assertThrows } from "./test_util.ts";

Deno.test(function resourcesCloseBadArgs() {
  assertThrows(() => {
    Deno.close((null as unknown) as number);
  }, TypeError);
});
