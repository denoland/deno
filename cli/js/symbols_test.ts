// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert } from "./test_util.ts";

unitTest(function symbolsExists(): void {
  assert("internal" in Deno.symbols);
  assert("customInspect" in Deno.symbols);
});
