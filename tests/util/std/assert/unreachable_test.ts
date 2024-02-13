// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, AssertionError, unreachable } from "./mod.ts";

Deno.test("AssertsUnreachable", function () {
  let didThrow = false;
  try {
    unreachable();
  } catch (e) {
    assert(e instanceof AssertionError);
    assert(e.message === "unreachable");
    didThrow = true;
  }
  assert(didThrow);
});
