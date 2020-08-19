import {
  assert as denoAssert,
  assertEquals,
  assertNotEquals,
  assertStrictEquals,
  assertMatch,
  assertThrows,
  fail as denoFail
} from "../testing/asserts.ts";

import {  
  ok,
  assert, 
  equal,
  notEqual,
  strictEqual,
  match,
  throws,
  fail
} from './assert.ts';

assertStrictEquals(assert, denoAssert, '`assert()` should be exposed');
// ref: https://nodejs.org/dist/latest-v14.x/docs/api/assert.html#assert_assert_value_message
assertStrictEquals(assert, ok, '`assert()` should be an alias of `ok()`');
assertStrictEquals(assertEquals, equal, '`assertEquals()` should be exposed as `equal()`');
assertStrictEquals(assertNotEquals, notEqual, '`assertNotEquals()` should be exposed as `notEqual()`');
assertStrictEquals(assertStrictEquals, strictEqual, '`assertStrictEquals()` should be exposed as `strictEqual()`');
assertStrictEquals(assertMatch, match, '`assertMatch()` should be exposed as `match()`');
assertStrictEquals(assertThrows, throws, '`assertThrows()` should be exposed as `throws()`');
assertStrictEquals(fail, denoFail, '`fail()` should be exposed');
