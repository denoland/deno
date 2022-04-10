// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible. Do not rely on good formatting of values
// for AssertionError messages in browsers.

import {
  bgGreen,
  bgRed,
  bold,
  gray,
  green,
  red,
  stripColor,
  white,
} from "../fmt/colors.ts";
import { diff, DiffResult, diffstr, DiffType } from "./_diff.ts";

const CAN_NOT_DISPLAY = "[Cannot display]";

export interface Constructor {
  // deno-lint-ignore no-explicit-any
  new (...args: any[]): any;
}

export class AssertionError extends Error {
  name = "AssertionError";
  constructor(message: string) {
    super(message);
  }
}

/**
 * Converts the input into a string. Objects, Sets and Maps are sorted so as to
 * make tests less flaky
 * @param v Value to be formatted
 */
export function _format(v: unknown): string {
  // deno-lint-ignore no-explicit-any
  const { Deno } = globalThis as any;
  return typeof Deno?.inspect === "function"
    ? Deno.inspect(v, {
      depth: Infinity,
      sorted: true,
      trailingComma: true,
      compact: false,
      iterableLimit: Infinity,
    })
    : `"${String(v).replace(/(?=["\\])/g, "\\")}"`;
}

/**
 * Colors the output of assertion diffs
 * @param diffType Difference type, either added or removed
 */
function createColor(
  diffType: DiffType,
  { background = false } = {},
): (s: string) => string {
  switch (diffType) {
    case DiffType.added:
      return (s: string): string =>
        background ? bgGreen(white(s)) : green(bold(s));
    case DiffType.removed:
      return (s: string): string => background ? bgRed(white(s)) : red(bold(s));
    default:
      return white;
  }
}

/**
 * Prefixes `+` or `-` in diff output
 * @param diffType Difference type, either added or removed
 */
function createSign(diffType: DiffType): string {
  switch (diffType) {
    case DiffType.added:
      return "+   ";
    case DiffType.removed:
      return "-   ";
    default:
      return "    ";
  }
}

function buildMessage(
  diffResult: ReadonlyArray<DiffResult<string>>,
  { stringDiff = false } = {},
): string[] {
  const messages: string[] = [], diffMessages: string[] = [];
  messages.push("");
  messages.push("");
  messages.push(
    `    ${gray(bold("[Diff]"))} ${red(bold("Actual"))} / ${
      green(bold("Expected"))
    }`,
  );
  messages.push("");
  messages.push("");
  diffResult.forEach((result: DiffResult<string>): void => {
    const c = createColor(result.type);
    const line = result.details?.map((detail) =>
      detail.type !== DiffType.common
        ? createColor(detail.type, { background: true })(detail.value)
        : detail.value
    ).join("") ?? result.value;
    diffMessages.push(c(`${createSign(result.type)}${line}`));
  });
  messages.push(...(stringDiff ? [diffMessages.join("")] : diffMessages));
  messages.push("");

  return messages;
}

function isKeyedCollection(x: unknown): x is Set<unknown> {
  return [Symbol.iterator, "size"].every((k) => k in (x as Set<unknown>));
}

/**
 * Deep equality comparison used in assertions
 * @param c actual value
 * @param d expected value
 */
export function equal(c: unknown, d: unknown): boolean {
  const seen = new Map();
  return (function compare(a: unknown, b: unknown): boolean {
    // Have to render RegExp & Date for string comparison
    // unless it's mistreated as object
    if (
      a &&
      b &&
      ((a instanceof RegExp && b instanceof RegExp) ||
        (a instanceof URL && b instanceof URL))
    ) {
      return String(a) === String(b);
    }
    if (a instanceof Date && b instanceof Date) {
      const aTime = a.getTime();
      const bTime = b.getTime();
      // Check for NaN equality manually since NaN is not
      // equal to itself.
      if (Number.isNaN(aTime) && Number.isNaN(bTime)) {
        return true;
      }
      return a.getTime() === b.getTime();
    }
    if (Object.is(a, b)) {
      return true;
    }
    if (a && typeof a === "object" && b && typeof b === "object") {
      if (a && b && !constructorsEqual(a, b)) {
        return false;
      }
      if (a instanceof WeakMap || b instanceof WeakMap) {
        if (!(a instanceof WeakMap && b instanceof WeakMap)) return false;
        throw new TypeError("cannot compare WeakMap instances");
      }
      if (a instanceof WeakSet || b instanceof WeakSet) {
        if (!(a instanceof WeakSet && b instanceof WeakSet)) return false;
        throw new TypeError("cannot compare WeakSet instances");
      }
      if (seen.get(a) === b) {
        return true;
      }
      if (Object.keys(a || {}).length !== Object.keys(b || {}).length) {
        return false;
      }
      if (isKeyedCollection(a) && isKeyedCollection(b)) {
        if (a.size !== b.size) {
          return false;
        }

        let unmatchedEntries = a.size;

        for (const [aKey, aValue] of a.entries()) {
          for (const [bKey, bValue] of b.entries()) {
            /* Given that Map keys can be references, we need
             * to ensure that they are also deeply equal */
            if (
              (aKey === aValue && bKey === bValue && compare(aKey, bKey)) ||
              (compare(aKey, bKey) && compare(aValue, bValue))
            ) {
              unmatchedEntries--;
            }
          }
        }

        return unmatchedEntries === 0;
      }
      const merged = { ...a, ...b };
      for (
        const key of [
          ...Object.getOwnPropertyNames(merged),
          ...Object.getOwnPropertySymbols(merged),
        ]
      ) {
        type Key = keyof typeof merged;
        if (!compare(a && a[key as Key], b && b[key as Key])) {
          return false;
        }
        if (((key in a) && (!(key in b))) || ((key in b) && (!(key in a)))) {
          return false;
        }
      }
      seen.set(a, b);
      if (a instanceof WeakRef || b instanceof WeakRef) {
        if (!(a instanceof WeakRef && b instanceof WeakRef)) return false;
        return compare(a.deref(), b.deref());
      }
      return true;
    }
    return false;
  })(c, d);
}

// deno-lint-ignore ban-types
function constructorsEqual(a: object, b: object) {
  return a.constructor === b.constructor ||
    a.constructor === Object && !b.constructor ||
    !a.constructor && b.constructor === Object;
}

/** Make an assertion, error will be thrown if `expr` does not have truthy value. */
export function assert(expr: unknown, msg = ""): asserts expr {
  if (!expr) {
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` and `expected` are equal, deeply. If not
 * deeply equal, then throw.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 * For example:
 * ```ts
 * import { assertEquals } from "./asserts.ts";
 *
 * assertEquals<number>(1, 2)
 * ```
 */
export function assertEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void;
export function assertEquals<T>(actual: T, expected: T, msg?: string): void;
export function assertEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void {
  if (equal(actual, expected)) {
    return;
  }
  let message = "";
  const actualString = _format(actual);
  const expectedString = _format(expected);
  try {
    const stringDiff = (typeof actual === "string") &&
      (typeof expected === "string");
    const diffResult = stringDiff
      ? diffstr(actual as string, expected as string)
      : diff(actualString.split("\n"), expectedString.split("\n"));
    const diffMsg = buildMessage(diffResult, { stringDiff }).join("\n");
    message = `Values are not equal:\n${diffMsg}`;
  } catch {
    message = `\n${red(CAN_NOT_DISPLAY)} + \n\n`;
  }
  if (msg) {
    message = msg;
  }
  throw new AssertionError(message);
}

/**
 * Make an assertion that `actual` and `expected` are not equal, deeply.
 * If not then throw.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 * For example:
 * ```ts
 * import { assertNotEquals } from "./asserts.ts";
 *
 * assertNotEquals<number>(1, 2)
 * ```
 */
export function assertNotEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void;
export function assertNotEquals<T>(actual: T, expected: T, msg?: string): void;
export function assertNotEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void {
  if (!equal(actual, expected)) {
    return;
  }
  let actualString: string;
  let expectedString: string;
  try {
    actualString = String(actual);
  } catch {
    actualString = "[Cannot display]";
  }
  try {
    expectedString = String(expected);
  } catch {
    expectedString = "[Cannot display]";
  }
  if (!msg) {
    msg = `actual: ${actualString} expected: ${expectedString}`;
  }
  throw new AssertionError(msg);
}

/**
 * Make an assertion that `actual` and `expected` are strictly equal. If
 * not then throw.
 *
 * ```ts
 * import { assertStrictEquals } from "./asserts.ts";
 *
 * assertStrictEquals(1, 2)
 * ```
 */
export function assertStrictEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void;
export function assertStrictEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
): void;
export function assertStrictEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void {
  if (actual === expected) {
    return;
  }

  let message: string;

  if (msg) {
    message = msg;
  } else {
    const actualString = _format(actual);
    const expectedString = _format(expected);

    if (actualString === expectedString) {
      const withOffset = actualString
        .split("\n")
        .map((l) => `    ${l}`)
        .join("\n");
      message =
        `Values have the same structure but are not reference-equal:\n\n${
          red(withOffset)
        }\n`;
    } else {
      try {
        const stringDiff = (typeof actual === "string") &&
          (typeof expected === "string");
        const diffResult = stringDiff
          ? diffstr(actual as string, expected as string)
          : diff(actualString.split("\n"), expectedString.split("\n"));
        const diffMsg = buildMessage(diffResult, { stringDiff }).join("\n");
        message = `Values are not strictly equal:\n${diffMsg}`;
      } catch {
        message = `\n${red(CAN_NOT_DISPLAY)} + \n\n`;
      }
    }
  }

  throw new AssertionError(message);
}

/**
 * Make an assertion that `actual` and `expected` are not strictly equal.
 * If the values are strictly equal then throw.
 *
 * ```ts
 * import { assertNotStrictEquals } from "./asserts.ts";
 *
 * assertNotStrictEquals(1, 1)
 * ```
 */
export function assertNotStrictEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void;
export function assertNotStrictEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
): void;
export function assertNotStrictEquals(
  actual: unknown,
  expected: unknown,
  msg?: string,
): void {
  if (actual !== expected) {
    return;
  }

  throw new AssertionError(
    msg ?? `Expected "actual" to be strictly unequal to: ${_format(actual)}\n`,
  );
}

/**
 * Make an assertion that actual is not null or undefined.
 * If not then throw.
 */
export function assertExists<T>(
  actual: T,
  msg?: string,
): asserts actual is NonNullable<T> {
  if (actual === undefined || actual === null) {
    if (!msg) {
      msg = `actual: "${actual}" expected to not be null or undefined`;
    }
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that actual includes expected. If not
 * then throw.
 */
export function assertStringIncludes(
  actual: string,
  expected: string,
  msg?: string,
): void {
  if (!actual.includes(expected)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to contain: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` includes the `expected` values.
 * If not then an error will be thrown.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 * For example:
 *
 * ```ts
 * import { assertArrayIncludes } from "./asserts.ts";
 *
 * assertArrayIncludes<number>([1, 2], [2])
 * ```
 */
export function assertArrayIncludes(
  actual: ArrayLike<unknown>,
  expected: ArrayLike<unknown>,
  msg?: string,
): void;
export function assertArrayIncludes<T>(
  actual: ArrayLike<T>,
  expected: ArrayLike<T>,
  msg?: string,
): void;
export function assertArrayIncludes(
  actual: ArrayLike<unknown>,
  expected: ArrayLike<unknown>,
  msg?: string,
): void {
  const missing: unknown[] = [];
  for (let i = 0; i < expected.length; i++) {
    let found = false;
    for (let j = 0; j < actual.length; j++) {
      if (equal(expected[i], actual[j])) {
        found = true;
        break;
      }
    }
    if (!found) {
      missing.push(expected[i]);
    }
  }
  if (missing.length === 0) {
    return;
  }
  if (!msg) {
    msg = `actual: "${_format(actual)}" expected to include: "${
      _format(expected)
    }"\nmissing: ${_format(missing)}`;
  }
  throw new AssertionError(msg);
}

/**
 * Make an assertion that `actual` match RegExp `expected`. If not
 * then throw.
 */
export function assertMatch(
  actual: string,
  expected: RegExp,
  msg?: string,
): void {
  if (!expected.test(actual)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to match: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` not match RegExp `expected`. If match
 * then throw.
 */
export function assertNotMatch(
  actual: string,
  expected: RegExp,
  msg?: string,
): void {
  if (expected.test(actual)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to not match: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}

/**
 * Make an assertion that `actual` object is a subset of `expected` object, deeply.
 * If not, then throw.
 */
export function assertObjectMatch(
  // deno-lint-ignore no-explicit-any
  actual: Record<PropertyKey, any>,
  expected: Record<PropertyKey, unknown>,
): void {
  type loose = Record<PropertyKey, unknown>;
  const seen = new WeakMap();
  return assertEquals(
    (function filter(a: loose, b: loose): loose {
      // Prevent infinite loop with circular references with same filter
      if ((seen.has(a)) && (seen.get(a) === b)) {
        return a;
      }
      seen.set(a, b);
      // Filter keys and symbols which are present in both actual and expected
      const filtered = {} as loose;
      const entries = [
        ...Object.getOwnPropertyNames(a),
        ...Object.getOwnPropertySymbols(a),
      ]
        .filter((key) => key in b)
        .map((key) => [key, a[key as string]]) as Array<[string, unknown]>;
      for (const [key, value] of entries) {
        // On array references, build a filtered array and filter nested objects inside
        if (Array.isArray(value)) {
          const subset = (b as loose)[key];
          if (Array.isArray(subset)) {
            filtered[key] = value
              .slice(0, subset.length)
              .map((element, index) => {
                const subsetElement = subset[index];
                if ((typeof subsetElement === "object") && (subsetElement)) {
                  return filter(element, subsetElement);
                }
                return element;
              });
            continue;
          }
        } // On nested objects references, build a filtered object recursively
        else if (typeof value === "object") {
          const subset = (b as loose)[key];
          if ((typeof subset === "object") && (subset)) {
            filtered[key] = filter(value as loose, subset as loose);
            continue;
          }
        }
        filtered[key] = value;
      }
      return filtered;
    })(actual, expected),
    expected,
  );
}

/**
 * Forcefully throws a failed assertion
 */
export function fail(msg?: string): never {
  assert(false, `Failed assertion${msg ? `: ${msg}` : "."}`);
}

/**
 * Executes a function, expecting it to throw.  If it does not, then it
 * throws. An error class and a string that should be included in the
 * error message can also be asserted. Or you can pass a
 * callback which will be passed the error, usually to apply some custom
 * assertions on it.
 */
export function assertThrows(
  fn: () => unknown,
  ErrorClass?: Constructor,
  msgIncludes?: string,
  msg?: string,
): void;
export function assertThrows(
  fn: () => unknown,
  errorCallback: (e: Error) => unknown,
  msg?: string,
): void;
export function assertThrows(
  fn: () => unknown,
  errorClassOrCallback?: Constructor | ((e: Error) => unknown),
  msgIncludesOrMsg?: string,
  msg?: string,
): void {
  let ErrorClass;
  let msgIncludes;
  let errorCallback;
  if (
    errorClassOrCallback == null ||
    errorClassOrCallback.prototype instanceof Error ||
    errorClassOrCallback.prototype === Error.prototype
  ) {
    ErrorClass = errorClassOrCallback;
    msgIncludes = msgIncludesOrMsg;
    errorCallback = null;
  } else {
    ErrorClass = null;
    msgIncludes = null;
    errorCallback = errorClassOrCallback as (e: Error) => unknown;
    msg = msgIncludesOrMsg;
  }
  let doesThrow = false;
  let error = null;
  try {
    fn();
  } catch (e) {
    if (e instanceof Error === false) {
      throw new AssertionError("A non-Error object was thrown.");
    }
    if (ErrorClass && !(e instanceof ErrorClass)) {
      msg = `Expected error to be instance of "${ErrorClass.name}", but was "${
        typeof e === "object" ? e?.constructor?.name : "[not an object]"
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    if (
      msgIncludes && (!(e instanceof Error) ||
        !stripColor(e.message).includes(stripColor(msgIncludes)))
    ) {
      msg = `Expected error message to include "${msgIncludes}", but got "${
        e instanceof Error ? e.message : "[not an Error]"
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    doesThrow = true;
    error = e;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new AssertionError(msg);
  }
  if (typeof errorCallback == "function") {
    errorCallback(error as Error);
  }
}

/**
 * Executes a function which returns a promise, expecting it to throw or reject.
 * If it does not, then it throws. An error class and a string that should be
 * included in the error message can also be asserted. Or you can pass a
 * callback which will be passed the error, usually to apply some custom
 * assertions on it.
 */
export function assertRejects(
  fn: () => Promise<unknown>,
  ErrorClass?: Constructor,
  msgIncludes?: string,
  msg?: string,
): Promise<void>;
export function assertRejects(
  fn: () => Promise<unknown>,
  errorCallback: (e: Error) => unknown,
  msg?: string,
): Promise<void>;
export async function assertRejects(
  fn: () => Promise<unknown>,
  errorClassOrCallback?: Constructor | ((e: Error) => unknown),
  msgIncludesOrMsg?: string,
  msg?: string,
): Promise<void> {
  let ErrorClass;
  let msgIncludes;
  let errorCallback;
  if (
    errorClassOrCallback == null ||
    errorClassOrCallback.prototype instanceof Error ||
    errorClassOrCallback.prototype === Error.prototype
  ) {
    ErrorClass = errorClassOrCallback;
    msgIncludes = msgIncludesOrMsg;
    errorCallback = null;
  } else {
    ErrorClass = null;
    msgIncludes = null;
    errorCallback = errorClassOrCallback as (e: Error) => unknown;
    msg = msgIncludesOrMsg;
  }
  let doesThrow = false;
  let error = null;
  try {
    await fn();
  } catch (e) {
    if (e instanceof Error === false) {
      throw new AssertionError("A non-Error object was thrown or rejected.");
    }
    if (ErrorClass && !(e instanceof ErrorClass)) {
      msg = `Expected error to be instance of "${ErrorClass.name}", but was "${
        typeof e === "object" ? e?.constructor?.name : "[not an object]"
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    if (
      msgIncludes && (!(e instanceof Error) ||
        !stripColor(e.message).includes(stripColor(msgIncludes)))
    ) {
      msg = `Expected error message to include "${msgIncludes}", but got "${
        e instanceof Error ? e.message : "[not an Error]"
      }"${msg ? `: ${msg}` : "."}`;
      throw new AssertionError(msg);
    }
    doesThrow = true;
    error = e;
  }
  if (!doesThrow) {
    msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
    throw new AssertionError(msg);
  }
  if (typeof errorCallback == "function") {
    errorCallback(error as Error);
  }
}

/**
 * Executes a function which returns a promise, expecting it to throw or reject.
 * If it does not, then it throws.  An error class and a string that should be
 * included in the error message can also be asserted.
 *
 * @deprecated
 */
export { assertRejects as assertThrowsAsync };

/** Use this to stub out methods that will throw when invoked. */
export function unimplemented(msg?: string): never {
  throw new AssertionError(msg || "unimplemented");
}

/** Use this to assert unreachable code. */
export function unreachable(): never {
  throw new AssertionError("unreachable");
}
