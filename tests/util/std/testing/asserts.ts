// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/**
 * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/mod.ts} instead.
 *
 * A library of assertion functions.
 * If the assertion is false an `AssertionError` will be thrown which will
 * result in pretty-printed diff of failing assertion.
 *
 * This module is browser compatible, but do not rely on good formatting of
 * values for AssertionError messages in browsers.
 *
 * @module
 */

export {
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert.ts} instead.
   *
   * Make an assertion, error will be thrown if `expr` does not have truthy value.
   */
  assert,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_equals.ts} instead.
   *
   * Make an assertion that `actual` and `expected` are almost equal numbers through
   * a given tolerance. It can be used to take into account IEEE-754 double-precision
   * floating-point representation limitations.
   * If the values are not almost equal then throw.
   *
   * @example
   * ```ts
   * import { assertAlmostEquals, assertThrows } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * assertAlmostEquals(0.1, 0.2);
   *
   * // Using a custom tolerance value
   * assertAlmostEquals(0.1 + 0.2, 0.3, 1e-16);
   * assertThrows(() => assertAlmostEquals(0.1 + 0.2, 0.3, 1e-17));
   * ```
   */
  assertAlmostEquals,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_array_includes.ts} instead.
   *
   * Make an assertion that `actual` includes the `expected` values.
   * If not then an error will be thrown.
   *
   * Type parameter can be specified to ensure values under comparison have the same type.
   *
   * @example
   * ```ts
   * import { assertArrayIncludes } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * assertArrayIncludes<number>([1, 2], [2])
   * ```
   */
  assertArrayIncludes,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_equals.ts} instead.
   *
   * Make an assertion that `actual` and `expected` are equal, deeply. If not
   * deeply equal, then throw.
   *
   * Type parameter can be specified to ensure values under comparison have the same type.
   *
   * @example
   * ```ts
   * import { assertEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("example", function (): void {
   *   assertEquals("world", "world");
   *   assertEquals({ hello: "world" }, { hello: "world" });
   * });
   * ```
   */
  assertEquals,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_exists.ts} instead.
   *
   * Make an assertion that actual is not null or undefined.
   * If not then throw.
   */
  assertExists,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_false.ts} instead.
   *
   * Make an assertion, error will be thrown if `expr` have truthy value.
   */
  assertFalse,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_instance_of.ts} instead.
   *
   * Make an assertion that `obj` is an instance of `type`.
   * If not then throw.
   */
  assertInstanceOf,
  /** @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assertion_error.ts} instead. */
  AssertionError,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_is_error.ts} instead.
   *
   * Make an assertion that `error` is an `Error`.
   * If not then an error will be thrown.
   * An error class and a string that should be included in the
   * error message can also be asserted.
   */
  assertIsError,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_match.ts} instead.
   *
   * Make an assertion that `actual` match RegExp `expected`. If not
   * then throw.
   */
  assertMatch,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_not_equals.ts} instead.
   *
   * Make an assertion that `actual` and `expected` are not equal, deeply.
   * If not then throw.
   *
   * Type parameter can be specified to ensure values under comparison have the same type.
   *
   * @example
   * ```ts
   * import { assertNotEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * assertNotEquals<number>(1, 2)
   * ```
   */
  assertNotEquals,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_not_instance_of.ts} instead.
   *
   * Make an assertion that `obj` is not an instance of `type`.
   * If so, then throw.
   */
  assertNotInstanceOf,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_not_match.ts} instead.
   *
   * Make an assertion that `actual` object is a subset of `expected` object, deeply.
   * If not, then throw.
   */
  assertNotMatch,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_not_strict_equals.ts} instead.
   *
   * Make an assertion that `actual` and `expected` are not strictly equal.
   * If the values are strictly equal then throw.
   *
   * ```ts
   * import { assertNotStrictEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * assertNotStrictEquals(1, 1)
   * ```
   */
  assertNotStrictEquals,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_object_match.ts} instead.
   *
   * Make an assertion that `actual` object is a subset of `expected` object, deeply.
   * If not, then throw.
   */
  assertObjectMatch,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_rejects.ts} instead.
   *
   * Executes a function which returns a promise, expecting it to reject.
   * If it does not, then it throws. An error class and a string that should be
   * included in the error message can also be asserted.
   *
   * @example
   * ```ts
   * import { assertRejects } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("doesThrow", async function () {
   *   await assertRejects(async () => {
   *     throw new TypeError("hello world!");
   *   }, TypeError);
   *   await assertRejects(
   *     async () => {
   *       throw new TypeError("hello world!");
   *     },
   *     TypeError,
   *     "hello",
   *   );
   * });
   *
   * // This test will not pass.
   * Deno.test("fails", async function () {
   *   await assertRejects(
   *     async () => {
   *       console.log("Hello world");
   *     },
   *   );
   * });
   * ```
   *
   * @example
   * ```ts
   * import { assertRejects } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("doesThrow", async function () {
   *   await assertRejects(
   *     async () => {
   *       throw new TypeError("hello world!");
   *     },
   *   );
   *   await assertRejects(
   *     async () => {
   *       return Promise.reject(new Error());
   *     },
   *   );
   * });
   *
   * // This test will not pass.
   * Deno.test("fails", async function () {
   *   await assertRejects(
   *     async () => {
   *       console.log("Hello world");
   *     },
   *   );
   * });
   * ```
   */
  assertRejects,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_strict_equals.ts} instead.
   *
   * Make an assertion that `actual` and `expected` are strictly equal. If
   * not then throw.
   *
   * @example
   * ```ts
   * import { assertStrictEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("isStrictlyEqual", function (): void {
   *   const a = {};
   *   const b = a;
   *   assertStrictEquals(a, b);
   * });
   *
   * // This test fails
   * Deno.test("isNotStrictlyEqual", function (): void {
   *   const a = {};
   *   const b = {};
   *   assertStrictEquals(a, b);
   * });
   * ```
   */
  assertStrictEquals,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_string_includes.ts} instead.
   *
   * Make an assertion that actual includes expected. If not
   * then throw.
   */
  assertStringIncludes,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/assert_throws.ts} instead.
   *
   * Executes a function, expecting it to throw. If it does not, then it
   * throws. An error class and a string that should be included in the
   * error message can also be asserted.
   *
   * @example
   * ```ts
   * import { assertThrows } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("doesThrow", function (): void {
   *   assertThrows((): void => {
   *     throw new TypeError("hello world!");
   *   }, TypeError);
   *   assertThrows(
   *     (): void => {
   *       throw new TypeError("hello world!");
   *     },
   *     TypeError,
   *     "hello",
   *   );
   * });
   *
   * // This test will not pass.
   * Deno.test("fails", function (): void {
   *   assertThrows((): void => {
   *     console.log("Hello world");
   *   });
   * });
   * ```
   *
   * @example
   * ```ts
   * import { assertThrows } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
   *
   * Deno.test("doesThrow", function (): void {
   *   assertThrows((): void => {
   *     throw new TypeError("hello world!");
   *   });
   * });
   *
   * // This test will not pass.
   * Deno.test("fails", function (): void {
   *   assertThrows((): void => {
   *     console.log("Hello world");
   *   });
   * });
   * ```
   */
  assertThrows,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/equal.ts} instead.
   *
   * Deep equality comparison used in assertions
   * @param c actual value
   * @param d expected value
   */
  equal,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/fail.ts} instead.
   *
   * Forcefully throws a failed assertion
   */
  fail,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/unimplemented.ts} instead.
   *
   * Use this to stub out methods that will throw when invoked.
   */
  unimplemented,
  /**
   * @deprecated (will be removed after 1.0.0) Import from {@link https://deno.land/std/assert/unreachable.ts} instead.
   *
   * Use this to assert unreachable code.
   */
  unreachable,
} from "../assert/mod.ts";
