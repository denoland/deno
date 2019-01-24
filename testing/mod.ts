// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Do not add imports in this file in order to be compatible with Node.

interface Constructor {
  new (...args: any[]): any;
}

const assertions = {
  /** Make an assertion, if not `true`, then throw. */
  assert(expr: boolean, msg = ""): void {
    if (!expr) {
      throw new Error(msg);
    }
  },

  /** Make an assertion that `actual` and `expected` are equal, deeply. If not
   * deeply equal, then throw.
   */
  equal(actual: unknown, expected: unknown, msg?: string): void {
    if (!equal(actual, expected)) {
      let actualString: string;
      let expectedString: string;
      try {
        actualString = String(actual);
      } catch (e) {
        actualString = "[Cannot display]";
      }
      try {
        expectedString = String(expected);
      } catch (e) {
        expectedString = "[Cannot display]";
      }
      console.error(
        "assertEqual failed. actual =",
        actualString,
        "expected =",
        expectedString
      );
      if (!msg) {
        msg = `actual: ${actualString} expected: ${expectedString}`;
      }
      throw new Error(msg);
    }
  },

  /** Make an assertion that `actual` and `expected` are strictly equal.  If
   * not then throw.
   */
  strictEqual(actual: unknown, expected: unknown, msg = ""): void {
    if (actual !== expected) {
      let actualString: string;
      let expectedString: string;
      try {
        actualString = String(actual);
      } catch (e) {
        actualString = "[Cannot display]";
      }
      try {
        expectedString = String(expected);
      } catch (e) {
        expectedString = "[Cannot display]";
      }
      console.error(
        "strictEqual failed. actual =",
        actualString,
        "expected =",
        expectedString
      );
      if (!msg) {
        msg = `actual: ${actualString} expected: ${expectedString}`;
      }
      throw new Error(msg);
    }
  },

  /** Executes a function, expecting it to throw.  If it does not, then it
   * throws.  An error class and a string that should be included in the
   * error message can also be asserted.
   */
  throws(
    fn: () => void,
    ErrorClass?: Constructor,
    msgIncludes = "",
    msg = ""
  ): void {
    let doesThrow = false;
    try {
      fn();
    } catch (e) {
      if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
        msg = `Expected error to be instance of "${ErrorClass.name}"${
          msg ? `: ${msg}` : "."
        }`;
        throw new Error(msg);
      }
      if (msgIncludes) {
        if (!e.message.includes(msgIncludes)) {
          msg = `Expected error message to include "${msgIncludes}", but got "${
            e.message
          }"${msg ? `: ${msg}` : "."}`;
          throw new Error(msg);
        }
      }
      doesThrow = true;
    }
    if (!doesThrow) {
      msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
      throw new Error(msg);
    }
  },

  async throwsAsync(
    fn: () => Promise<void>,
    ErrorClass?: Constructor,
    msgIncludes = "",
    msg = ""
  ): Promise<void> {
    let doesThrow = false;
    try {
      await fn();
    } catch (e) {
      if (ErrorClass && !(Object.getPrototypeOf(e) === ErrorClass.prototype)) {
        msg = `Expected error to be instance of "${ErrorClass.name}"${
          msg ? `: ${msg}` : "."
        }`;
        throw new Error(msg);
      }
      if (msgIncludes) {
        if (!e.message.includes(msgIncludes)) {
          msg = `Expected error message to include "${msgIncludes}", but got "${
            e.message
          }"${msg ? `: ${msg}` : "."}`;
          throw new Error(msg);
        }
      }
      doesThrow = true;
    }
    if (!doesThrow) {
      msg = `Expected function to throw${msg ? `: ${msg}` : "."}`;
      throw new Error(msg);
    }
  }
};

type Assert = typeof assertions.assert & typeof assertions;

// Decorate assertions.assert with all the assertions
Object.assign(assertions.assert, assertions);

export const assert = assertions.assert as Assert;

/**
 * An alias to assert.equal
 * @deprecated
 */
export const assertEqual = assert.equal;

export function equal(c: unknown, d: unknown): boolean {
  const seen = new Map();
  return (function compare(a: unknown, b: unknown) {
    if (Object.is(a, b)) {
      return true;
    }
    if (a && typeof a === "object" && b && typeof b === "object") {
      if (seen.get(a) === b) {
        return true;
      }
      if (Object.keys(a || {}).length !== Object.keys(b || {}).length) {
        return false;
      }
      const merged = { ...a, ...b };
      for (const key in merged) {
        type Key = keyof typeof merged;
        if (!compare(a && a[key as Key], b && b[key as Key])) {
          return false;
        }
      }
      seen.set(a, b);
      return true;
    }
    return false;
  })(c, d);
}

export type TestFunction = () => void | Promise<void>;

export interface TestDefinition {
  fn: TestFunction;
  name: string;
}

export const exitOnFail = true;

let filterRegExp: RegExp | null;
const tests: TestDefinition[] = [];

let filtered = 0;
const ignored = 0;
const measured = 0;

// Must be called before any test() that needs to be filtered.
export function setFilter(s: string): void {
  filterRegExp = new RegExp(s, "i");
}

export function test(t: TestDefinition | TestFunction): void {
  const fn: TestFunction = typeof t === "function" ? t : t.fn;
  const name: string = t.name;

  if (!name) {
    throw new Error("Test function may not be anonymous");
  }
  if (filter(name)) {
    tests.push({ fn, name });
  } else {
    filtered++;
  }
}

function filter(name: string): boolean {
  if (filterRegExp) {
    return filterRegExp.test(name);
  } else {
    return true;
  }
}

const RESET = "\x1b[0m";
const FG_RED = "\x1b[31m";
const FG_GREEN = "\x1b[32m";

function red_failed() {
  return FG_RED + "FAILED" + RESET;
}

function green_ok() {
  return FG_GREEN + "ok" + RESET;
}

export async function runTests() {
  let passed = 0;
  let failed = 0;

  console.log("running", tests.length, "tests");
  for (let i = 0; i < tests.length; i++) {
    const { fn, name } = tests[i];
    let result = green_ok();
    // See https://github.com/denoland/deno/pull/1452
    // about this usage of groupCollapsed
    console.groupCollapsed(`test ${name} `);
    try {
      await fn();
      passed++;
      console.log("...", result);
      console.groupEnd();
    } catch (e) {
      result = red_failed();
      console.log("...", result);
      console.groupEnd();
      console.error((e && e.stack) || e);
      failed++;
      if (exitOnFail) {
        break;
      }
    }
  }

  // Attempting to match the output of Rust's test runner.
  const result = failed > 0 ? red_failed() : green_ok();
  console.log(
    `\ntest result: ${result}. ${passed} passed; ${failed} failed; ` +
      `${ignored} ignored; ${measured} measured; ${filtered} filtered out\n`
  );

  if (failed === 0) {
    // All good.
  } else {
    // Use setTimeout to avoid the error being ignored due to unhandled
    // promise rejections being swallowed.
    setTimeout(() => {
      throw new Error(`There were ${failed} test failures.`);
    }, 0);
  }
}
