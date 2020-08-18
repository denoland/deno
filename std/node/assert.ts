import {
  assertEquals,
  assertNotEquals,
  assertStrictEquals,
  assertMatch,
  assertThrows,
} from "../testing/asserts.ts";

export { assert, fail } from "../testing/asserts.ts";

export const equal = assertEquals;
export const notEqual = assertNotEquals;
export const strictEqual = assertStrictEquals;
export const match = assertMatch;
export const throws = assertThrows;
