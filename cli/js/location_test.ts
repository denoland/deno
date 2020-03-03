// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function locationBasic(): void {
  // location example: file:///Users/rld/src/deno/js/unit_tests.ts
  assert(window.location.toString().endsWith("unit_tests.ts"));
});
