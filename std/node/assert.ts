// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assertEquals,
  assertMatch,
  assertNotEquals,
  assertNotStrictEquals,
  assertStrictEquals,
  assertThrows,
} from "../testing/asserts.ts";

export { AssertionError } from "./assertion_error.ts";

export {
  assert,
  assert as default,
  assert as ok,
  fail,
} from "../testing/asserts.ts";

export const deepStrictEqual = assertEquals;
export const notDeepStrictEqual = assertNotEquals;
export const strictEqual = assertStrictEquals;
export const notStrictEqual = assertNotStrictEquals;
export const match = assertMatch;
export const throws = assertThrows;
