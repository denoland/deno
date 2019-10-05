// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function locationBasic(): void {
  // location example: file:///Users/rld/src/deno/js/unit_tests.ts
  console.log("location", window.location.toString());
  assert(window.location.toString().endsWith("unit_tests.ts"));
});
