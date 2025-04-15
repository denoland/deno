// Copyright 2018-2025 the Deno authors. MIT license.
import { assert } from "./test_util.ts";

// Test that `Symbol.metadata` is defined. This file can be removed when V8
// supports `Symbol.metadata` natively.

Deno.test(
  function symbolMetadataIsDefined() {
    assert(typeof Symbol.metadata === "symbol");
  },
);
