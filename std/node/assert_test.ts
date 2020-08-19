import {
  assert as denoAssert,
  assertEquals,
  assertNotEquals,
  assertStrictEquals,
  assertNotStrictEquals,
  assertMatch,
  assertThrows,
  fail as denoFail,
} from "../testing/asserts.ts";

import assert from "./assert.ts";

import {
  ok,
  assert as assert_,
  deepStrictEqual,
  notDeepStrictEqual,
  strictEqual,
  notStrictEqual,
  match,
  throws,
  fail,
} from "./assert.ts";

Deno.test("API should be exposed", () => {
  assertStrictEquals(
    assert_,
    assert,
    "`assert()` should be the default export",
  );
  assertStrictEquals(assert_, denoAssert, "`assert()` should be exposed");
  assertStrictEquals(assert_, ok, "`assert()` should be an alias of `ok()`");
  assertStrictEquals(
    assertEquals,
    deepStrictEqual,
    "`assertEquals()` should be exposed as `deepStrictEqual()`",
  );
  assertStrictEquals(
    assertNotEquals,
    notDeepStrictEqual,
    "`assertNotEquals()` should be exposed as `notDeepStrictEqual()`",
  );
  assertStrictEquals(
    assertStrictEquals,
    strictEqual,
    "`assertStrictEquals()` should be exposed as `strictEqual()`",
  );
  assertStrictEquals(
    assertNotStrictEquals,
    notStrictEqual,
    "`assertNotStrictEquals()` should be exposed as `notStrictEqual()`",
  );
  assertStrictEquals(
    assertMatch,
    match,
    "`assertMatch()` should be exposed as `match()`",
  );
  assertStrictEquals(
    assertThrows,
    throws,
    "`assertThrows()` should be exposed as `throws()`",
  );
  assertStrictEquals(fail, denoFail, "`fail()` should be exposed");
});
