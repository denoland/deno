// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
export { AssertionError } from "./assertion_error.ts";
import {
  assertEquals as deepStrictEqual,
  AssertionError,
  assertMatch as match,
  assertNotEquals as notDeepStrictEqual,
  assertNotStrictEquals as notStrictEqual,
  assertStrictEquals as strictEqual,
  assertThrows as throws,
  fail,
} from "../testing/asserts.ts";

function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}
const ok = assert;
export default assert;

Object.assign(assert, {
  deepStrictEqual,
  fail,
  match,
  notDeepStrictEqual,
  notStrictEqual,
  ok,
  strictEqual,
  throws,
});

export {
  deepStrictEqual,
  fail,
  match,
  notDeepStrictEqual,
  notStrictEqual,
  ok,
  strictEqual,
  throws,
};
