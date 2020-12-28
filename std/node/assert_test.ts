// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert as denoAssert,
  assertEquals,
  assertMatch,
  assertNotEquals,
  assertNotStrictEquals,
  assertStrictEquals,
  assertThrows,
  fail as denoFail,
} from "../testing/asserts.ts";

import AssertionError from "./assertion_error.ts";

import assert, {
  AssertionError as AssertionError_,
  deepStrictEqual,
  fail,
  match,
  notDeepStrictEqual,
  notStrictEqual,
  ok,
  strictEqual,
  throws,
} from "./assert.ts";

Deno.test("API should be exposed", () => {
  assertStrictEquals(assert, ok, "`assert()` should be an alias of `ok()`");
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
  assertStrictEquals(
    AssertionError,
    AssertionError_,
    "`AssertionError()` constructor should be exposed",
  );
});
