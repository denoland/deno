import {
  assertEquals,
  assertNotEquals,
  assertStrictEquals,
  assertNotStrictEquals,
  assertMatch,
  assertThrows,
} from "../testing/asserts.ts";

export {
  assert as default,
  assert as ok,
  assert,
  fail,
} from "../testing/asserts.ts";

export const deepStrictEqual = assertEquals;
export const notDeepStrictEqual = assertNotEquals;
export const strictEqual = assertStrictEquals;
export const notStrictEqual = assertNotStrictEquals;
export const match = assertMatch;
export const throws = assertThrows;
