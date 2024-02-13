// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, AssertionError, assertMatch } from "./mod.ts";

Deno.test("AssertStringMatching", function () {
  assertMatch("foobar@deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

Deno.test("AssertStringMatchingThrows", function () {
  let didThrow = false;
  try {
    assertMatch("Denosaurus from Jurassic", RegExp(/Raptor/));
  } catch (e) {
    assert(e instanceof AssertionError);
    assert(
      e.message ===
        `Expected actual: "Denosaurus from Jurassic" to match: "/Raptor/".`,
    );
    didThrow = true;
  }
  assert(didThrow);
});
