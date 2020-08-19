import {
  assertEquals,
  assertNotEquals,
  assertStrictEquals,
  assertMatch,
  assertThrows,
} from "../testing/asserts.ts";

export { assert as ok, assert, fail } from "../testing/asserts.ts";

export const deepStrictEqual = assertEquals;
export const notDeepStrictEqual = assertNotEquals;
export const strictEqual = assertStrictEquals;
export const match = assertMatch;
export const throws = assertThrows;
