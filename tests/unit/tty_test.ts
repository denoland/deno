// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert } from "./test_util.ts";

// Note tests for Deno.stdin.setRaw is in integration tests.

Deno.test(function consoleSize() {
  if (!Deno.stdout.isTerminal()) {
    return;
  }
  const result = Deno.consoleSize();
  assert(typeof result.columns !== "undefined");
  assert(typeof result.rows !== "undefined");
});
