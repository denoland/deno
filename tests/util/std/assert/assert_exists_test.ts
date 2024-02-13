// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertExists, AssertionError } from "./mod.ts";

Deno.test("AssertExists", function () {
  assertExists("Denosaurus");
  assertExists(false);
  assertExists(0);
  assertExists("");
  assertExists(-0);
  assertExists(0);
  assertExists(NaN);

  const value = new URLSearchParams({ value: "test" }).get("value");
  assertExists(value);
  assertEquals(value.length, 4);

  let didThrow;
  try {
    assertExists(undefined);
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
  didThrow = false;
  try {
    assertExists(null);
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});
