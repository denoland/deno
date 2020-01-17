// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, assert } from "./test_util.ts";

test(function symbolsExists(): void {
  assert("internal" in Deno.symbols);
  assert("customInspect" in Deno.symbols);
});
