// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest(function locationBasic(): void {
  // location example: file:///Users/rld/src/deno/js/unit_tests.ts
  assert(window.location.toString().endsWith("unit_test_runner.ts"));
});
