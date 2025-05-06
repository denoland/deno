// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-deprecated-deno-api

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

Deno.test(function isattyDoesntRaiseOnBadRid() {
  // Absurdly large rid.
  // @ts-ignore `Deno.isatty()` was soft-removed in Deno 2.
  assert(!Deno.isatty(0x7fffffff));
});
