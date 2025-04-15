// Copyright 2018-2025 the Deno authors. MIT license.
import * as assert from "node:assert";

Deno.test("[node/assert] .throws() compares Error instance", () => {
  assert.throws(
    () => {
      throw new Error("FAIL");
    },
    Error,
  );

  assert.throws(
    () => {
      throw new TypeError("FAIL");
    },
    TypeError,
  );
});
