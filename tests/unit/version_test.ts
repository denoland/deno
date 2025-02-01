// Copyright 2018-2025 the Deno authors. MIT license.

import { assert, assertEquals } from "./test_util.ts";

Deno.test(function version() {
  const pattern = /^\d+\.\d+\.\d+/;
  assert(pattern.test(Deno.version.deno));
  assert(pattern.test(Deno.version.v8));
  assertEquals(Deno.version.typescript, "5.7.3");
});
