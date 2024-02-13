// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, AssertionError, assertNotMatch } from "./mod.ts";

Deno.test("AssertStringNotMatching", function () {
  assertNotMatch("foobar.deno.com", RegExp(/[a-zA-Z]+@[a-zA-Z]+.com/));
});

Deno.test("AssertStringNotMatchingThrows", function () {
  let didThrow = false;
  try {
    assertNotMatch("Denosaurus from Jurassic", RegExp(/from/));
  } catch (e) {
    assert(e instanceof AssertionError);
    assert(
      e.message ===
        `Expected actual: "Denosaurus from Jurassic" to not match: "/from/".`,
    );
    didThrow = true;
  }
  assert(didThrow);
});
