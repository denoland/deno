// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// vendored from std/testing/asserts.ts

import { red } from "ext:deno_node/_util/std_fmt_colors.ts";
import {
  buildMessage,
  diff,
  diffstr,
} from "ext:deno_node/_util/std_testing_diff.ts";

/** Converts the input into a string. Objects, Sets and Maps are sorted so as to
 * make tests less flaky */
export function format(v: unknown): string {
  // deno-lint-ignore no-explicit-any
  const { Deno } = globalThis as any;
  return typeof Deno?.inspect === "function"
    ? Deno.inspect(v, {
      depth: Infinity,
      sorted: true,
      trailingComma: true,
      compact: false,
      iterableLimit: Infinity,
      // getters should be true in assertEquals.
      getters: true,
    })
    : `"${String(v).replace(/(?=["\\])/g, "\\")}"`;
}

const CAN_NOT_DISPLAY = "[Cannot display]";

export class AssertionError extends Error {
  override name = "AssertionError";
  constructor(message: string) {
    super(message);
  }
}

function isKeyedCollection(x: unknown): x is Set<unknown> {
  return [Symbol.iterator, "size"].every((k) => k in (x as Set<unknown>));
}

/** Deep equality comparison used in assertions */
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
      return aTime === bTime;
    }
    if (typeof a === "number" && typeof b === "number") {
      return Number.isNaN(a) && Number.isNaN(b) || a === b;
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
      seen.set(a, b);
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
              break;
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

/** Make an assertion that `actual` and `expected` are equal, deeply. If not
 * deeply equal, then throw. */
export function assertEquals<T>(actual: T, expected: T, msg?: string) {
  if (equal(actual, expected)) {
    return;
  }
  let message = "";
  const actualString = format(actual);
  const expectedString = format(expected);
  try {
    const stringDiff = (typeof actual === "string") &&
      (typeof expected === "string");
    const diffResult = stringDiff
      ? diffstr(actual as string, expected as string)
      : diff(actualString.split("\n"), expectedString.split("\n"));
    const diffMsg = buildMessage(diffResult, { stringDiff }).join("\n");
    message = `Values are not equal:\n${diffMsg}`;
  } catch {
    message = `\n${red(red(CAN_NOT_DISPLAY))} + \n\n`;
  }
  if (msg) {
    message = msg;
  }
  throw new AssertionError(message);
}

/** Make an assertion that `actual` and `expected` are not equal, deeply.
 * If not then throw. */
export function assertNotEquals<T>(actual: T, expected: T, msg?: string) {
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
    msg = `actual: ${actualString} expected not to be: ${expectedString}`;
  }
  throw new AssertionError(msg);
}

/** Make an assertion that `actual` and `expected` are strictly equal. If
 * not then throw. */
export function assertStrictEquals<T>(
  actual: unknown,
  expected: T,
  msg?: string,
): asserts actual is T {
  if (Object.is(actual, expected)) {
    return;
  }

  let message: string;

  if (msg) {
    message = msg;
  } else {
    const actualString = format(actual);
    const expectedString = format(expected);

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
        message = `\n${CAN_NOT_DISPLAY} + \n\n`;
      }
    }
  }

  throw new AssertionError(message);
}

/** Make an assertion that `actual` and `expected` are not strictly equal.
 * If the values are strictly equal then throw. */
export function assertNotStrictEquals<T>(
  actual: T,
  expected: T,
  msg?: string,
) {
  if (!Object.is(actual, expected)) {
    return;
  }

  throw new AssertionError(
    msg ?? `Expected "actual" to be strictly unequal to: ${format(actual)}\n`,
  );
}

/** Make an assertion that `actual` match RegExp `expected`. If not
 * then throw. */
export function assertMatch(
  actual: string,
  expected: RegExp,
  msg?: string,
) {
  if (!expected.test(actual)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to match: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}

/** Make an assertion that `actual` not match RegExp `expected`. If match
 * then throw. */
export function assertNotMatch(
  actual: string,
  expected: RegExp,
  msg?: string,
) {
  if (expected.test(actual)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to not match: "${expected}"`;
    }
    throw new AssertionError(msg);
  }
}
