// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  AssertionError,
  assertNotEquals,
} from "./mod.ts";

Deno.test("NotEquals", function () {
  const a = { foo: "bar" };
  const b = { bar: "foo" };
  assertNotEquals<unknown>(a, b);
  assertNotEquals("Denosaurus", "Tyrannosaurus");
  assertNotEquals(
    new Date(2019, 0, 3, 4, 20, 1, 10),
    new Date(2019, 0, 3, 4, 20, 1, 20),
  );
  assertNotEquals(new Date("invalid"), new Date(2019, 0, 3, 4, 20, 1, 20));
  let didThrow;
  try {
    assertNotEquals("Raptor", "Raptor");
    didThrow = false;
  } catch (e) {
    assert(e instanceof AssertionError);
    didThrow = true;
  }
  assertEquals(didThrow, true);
});
