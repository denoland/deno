// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, AssertionError, unimplemented } from "./mod.ts";

Deno.test("AssertsUnimplemented", function () {
  let didThrow = false;
  try {
    unimplemented();
  } catch (e) {
    assert(e instanceof AssertionError);
    assert(e.message === "Unimplemented.");
    didThrow = true;
  }
  assert(didThrow);
});
