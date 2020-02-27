// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { errors } from "../errors.ts";
import { inspect } from "../console.ts";
const { AssertionError } = errors;

function isKeyedCollection(x: unknown): x is Set<unknown> {
  return [Symbol.iterator, "size"].every(k => k in (x as Set<unknown>));
}

function compare(a: unknown, b: unknown, seen: Map<unknown, unknown>): boolean {
  if (a && b) {
    // Have to render RegExp & Date for string comparison
    // unless it's mistreated as object
    if (a instanceof RegExp && b instanceof RegExp) {
      return String(a) === String(b);
    }

    if (a instanceof Date && b instanceof Date) {
      return String(a) === String(b);
    }

    if (Object.is(a, b)) {
      return true;
    }

    if (typeof a === "object" && typeof b === "object") {
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
              (aKey === aValue &&
                bKey === bValue &&
                compare(aKey, bKey, seen)) ||
              (compare(aKey, bKey, seen) && compare(aValue, bValue, seen))
            ) {
              unmatchedEntries--;
            }
          }
        }

        return unmatchedEntries === 0;
      }

      const merged = { ...a, ...b };
      for (const key in merged) {
        type Key = keyof typeof merged;
        if (!compare(a && a[key as Key], b && b[key as Key], seen)) {
          return false;
        }
      }

      seen.set(a, b);
      return true;
    }
  }

  return false;
}

export function equal(actual: unknown, expected: unknown): boolean {
  const seen = new Map();
  return compare(actual, expected, seen);
}

export interface Assert {
  /** Make an assertion, if not `true`, then throw. */
  (expr: unknown, msg?: string): asserts expr;
  /** Use this to stub out methods that will throw when invoked. */
  unimplemented(msg?: string): never;
  /** Use this to assert unreachable code. */
  unreachable(msg?: string): never;
  /**
   * Make an assertion that `actual` and `expected` are equal, deeply. If not
   * deeply equal, then throw.
   */
  equals(actual: unknown, expected: unknown, msg?: string): void;
  /**
   * Make an assertion that `actual` and `expected` are not equal, deeply.
   * If not then throw.
   */
  notEquals(actual: unknown, expected: unknown, msg?: string): void;
  /**
   * Make an assertion that actual contains expected. If not
   * then thrown.
   */
  strContains(actual: string, expected: string, msg?: string): void;
  /**
   * Make an assertion that `actual` and `expected` are strictly equal.  If
   * not then throw.
   */
  strictEq(actual: unknown, expected: unknown, msg?: string): void;
  /**
   * Make an assertion that `actual` contains the `expected` values
   * If not then thrown.
   */
  arrayContains(actual: unknown[], expected: unknown[], msg?: string): void;
  /**
   * Make an assertion that `actual` match RegExp `expected`. If not
   * then thrown
   */
  match(actual: string, expected: RegExp, msg?: string): void;
}

function unimplemented(msg?: string): never {
  throw new AssertionError(msg || "unimplemented");
}

function unreachable(msg?: string): never {
  throw new AssertionError(msg || "unreachable");
}

function throwAssertionError(actual: any, expected: any, msg?: string): never {
  if (!msg) {
    msg = `actual: ${inspect(actual)} expected: ${inspect(expected)}`;
  }
  const e = new AssertionError(msg);
  e.actual = actual;
  e.expected = expected;
  throw e;
}

export function equals(actual: unknown, expected: unknown, msg?: string): void {
  if (equal(actual, expected)) {
    return;
  }
  throwAssertionError(actual, expected, msg);
}

export function notEquals(
  actual: unknown,
  expected: unknown,
  msg?: string
): void {
  if (!equal(actual, expected)) {
    return;
  }
  throwAssertionError(actual, expected, msg);
}

/**
 * Make an assertion that `actual` and `expected` are strictly equal.  If
 * not then throw.
 */
function strictEq(actual: unknown, expected: unknown, msg?: string): void {
  if (actual !== expected) {
    throwAssertionError(actual, expected, msg);
  }
}

export function strContains(
  actual: string,
  expected: string,
  msg?: string
): void {
  if (!actual.includes(expected)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to contains: "${expected}"`;
    }
    throwAssertionError(actual, expected, msg);
  }
}

function arrayContains(
  actual: unknown[],
  expected: unknown[],
  msg?: string
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
    msg = `actual: "${actual}" expected to contains: "${expected}"`;
    msg += "\n";
    msg += `missing: ${missing}`;
  }
  throwAssertionError(actual, expected, msg);
}

function match(actual: string, expected: RegExp, msg?: string): void {
  if (!expected.test(actual)) {
    if (!msg) {
      msg = `actual: "${actual}" expected to match: "${expected}"`;
    }
    throwAssertionError(actual, expected, msg);
  }
}

export const assert: Assert = Object.assign(
  (expr: unknown, msg = ""): asserts expr => {
    if (!expr) {
      throw new AssertionError(msg);
    }
  },
  {
    unimplemented,
    unreachable,
    equals,
    notEquals,
    strContains,
    strictEq,
    arrayContains,
    match
  }
);
