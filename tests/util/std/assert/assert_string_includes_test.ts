// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertStringIncludes,
} from "./mod.ts";

Deno.test("AssertStringIncludes", function () {
  assertStringIncludes("Denosaurus", "saur");
  assertStringIncludes("Denosaurus", "Deno");
  assertStringIncludes("Denosaurus", "rus");
  let didThrow;
  try {
    assertStringIncludes("Denosaurus", "Raptor");
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});

Deno.test("AssertStringContainsThrow", function () {
  let didThrow = false;
  try {
    assertStringIncludes("Denosaurus from Jurassic", "Raptor");
  } catch (e) {
    assert(e instanceof AssertionError);
    assert(
      e.message ===
        `Expected actual: "Denosaurus from Jurassic" to contain: "Raptor".`,
    );
    didThrow = true;
  }
  assert(didThrow);
});
