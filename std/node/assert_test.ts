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
  deepStrictEqual,
  notDeepStrictEqual,
  strictEqual,
  match,
  throws,
  fail
} from './assert.ts';

Deno.test('API should be exposed', () => {
  assertStrictEquals(assert, denoAssert, '`assert()` should be exposed');
  assertStrictEquals(assert, ok, '`assert()` should be an alias of `ok()`');
  assertStrictEquals(assertEquals, deepStrictEqual, '`assertEquals()` should be exposed as `deepStrictEqual()`');
  assertStrictEquals(assertNotEquals, notDeepStrictEqual, '`assertNotEquals()` should be exposed as `notDeepStrictEqual()`');
  assertStrictEquals(assertStrictEquals, strictEqual, '`assertStrictEquals()` should be exposed as `strictEqual()`');
  assertStrictEquals(assertMatch, match, '`assertMatch()` should be exposed as `match()`');
  assertStrictEquals(assertThrows, throws, '`assertThrows()` should be exposed as `throws()`');
  assertStrictEquals(fail, denoFail, '`fail()` should be exposed');
});
