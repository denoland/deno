// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { AssertionError, assertThrows, fail } from "./mod.ts";

Deno.test("AssertFail", function () {
  assertThrows(fail, AssertionError, "Failed assertion.");
  assertThrows(
    () => {
      fail("foo");
    },
    AssertionError,
    "Failed assertion: foo",
  );
});
