// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
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
