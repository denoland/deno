// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file ban-types prefer-primordials

import { AssertionError } from "ext:deno_node/internal/assert/assertion_error.js";
import { inspect } from "node:util";
import {
  ERR_AMBIGUOUS_ARGUMENT,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_INVALID_RETURN_VALUE,
  ERR_MISSING_ARGS,
} from "ext:deno_node/internal/errors.ts";
import {
  isDeepEqual,
  isDeepStrictEqual,
  isPartialStrictEqual,
} from "ext:deno_node/internal/util/comparisons.ts";
import { primordials } from "ext:core/mod.js";
import { CallTracker } from "ext:deno_node/internal/assert/calltracker.js";
import { deprecate } from "node:util";
import { isPromise, isRegExp } from "ext:deno_node/internal_binding/types.ts";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";

const {
  ArrayPrototypeIndexOf,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  ErrorPrototype,
  NumberIsNaN,
  ObjectIs,
  ObjectKeys,
  ObjectPrototypeIsPrototypeOf,
  ReflectApply,
  RegExpPrototypeExec,
  StringPrototypeIndexOf,
  StringPrototypeSlice,
  StringPrototypeSplit,
  String,
} = primordials;

type AssertPredicate =
  | RegExp
  | (new () => object)
  | ((thrown: unknown) => boolean)
  | object
  | Error;

const NO_EXCEPTION_SENTINEL = {};

function innerFail(obj: {
  actual?: unknown;
  expected?: unknown;
  message?: string | Error;
  operator?: string;
  stackStartFn?: Function;
}) {
  if (obj.message instanceof Error) {
    throw obj.message;
  }

  throw new AssertionError({
    actual: obj.actual,
    expected: obj.expected,
    message: obj.message,
    operator: obj.operator,
    stackStartFn: obj.stackStartFn,
  });
}

function assert(actual: unknown, message?: string | Error): asserts actual {
  if (arguments.length === 0) {
    throw new AssertionError({
      message: "No value argument passed to `assert.ok()`",
    });
  }
  if (actual) {
    return;
  }

  equal(actual, true, message);
}
const ok = assert;

class Comparison {
  constructor(obj: object, keys: string[], actual?: unknown) {
    for (const key of keys) {
      if (key in obj) {
        if (
          actual !== undefined &&
          typeof actual[key] === "string" &&
          isRegExp(obj[key]) &&
          RegExpPrototypeExec(obj[key], actual[key]) !== null
        ) {
          this[key] = actual[key];
        } else {
          this[key] = obj[key];
        }
      }
    }
  }
}

function compareExceptionKey(
  actual: object,
  expected: object,
  key: string,
  message: string | Error | undefined,
  keys: string[],
  fn: () => unknown | (() => Promise<unknown>),
) {
  if (!(key in actual) || !isDeepStrictEqual(actual[key], expected[key])) {
    if (!message) {
      // Create placeholder objects to create a nice output.
      const a = new Comparison(actual, keys);
      const b = new Comparison(expected, keys, actual);

      const err = new AssertionError({
        actual: a,
        expected: b,
        operator: "deepStrictEqual",
        stackStartFn: fn,
      });
      err.actual = actual;
      err.expected = expected;
      err.operator = fn.name;
      throw err;
    }
    innerFail({
      actual,
      expected,
      message,
      operator: fn.name,
      stackStartFn: fn,
    });
  }
}

function expectedException(
  actual: unknown,
  expected: AssertPredicate,
  message: string | Error | undefined,
  fn: Function,
) {
  let generatedMessage = false;
  let throwError = false;

  if (typeof expected !== "function") {
    // Handle regular expressions.
    if (isRegExp(expected)) {
      const str = String(actual);
      if (RegExpPrototypeExec(expected, str) !== null) {
        return;
      }

      if (!message) {
        generatedMessage = true;
        message = "The input did not match the regular expression " +
          `${inspect(expected)}. Input:\n\n${inspect(str)}\n`;
      }
      throwError = true;
      // Handle primitives properly.
    } else if (typeof actual !== "object" || actual === null) {
      const err = new AssertionError({
        actual,
        expected,
        message,
        operator: "deepStrictEqual",
        stackStartFn: fn,
      });
      err.operator = fn.name;
      throw err;
    } else {
      // Handle validation objects.
      const keys = ObjectKeys(expected);
      // Special handle errors to make sure the name and the message are
      // compared as well.
      if (expected instanceof Error) {
        ArrayPrototypePush(keys, "name", "message");
      } else if (keys.length === 0) {
        throw new ERR_INVALID_ARG_VALUE(
          "error",
          expected,
          "may not be an empty object",
        );
      }
      for (const key of keys) {
        if (
          typeof actual[key] === "string" &&
          isRegExp(expected[key]) &&
          RegExpPrototypeExec(expected[key], actual[key]) !== null
        ) {
          continue;
        }
        compareExceptionKey(actual, expected, key, message, keys, fn);
      }
      return;
    }
    // Guard instanceof against arrow functions as they don't have a prototype.
    // Check for matching Error classes.
  } else if (expected.prototype !== undefined && actual instanceof expected) {
    return;
  } else if (ObjectPrototypeIsPrototypeOf(Error, expected)) {
    if (!message) {
      generatedMessage = true;
      message = "The error is expected to be an instance of " +
        `"${expected.name}". Received `;
      if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, actual)) {
        const name = (actual.constructor?.name) ||
          actual.name;
        if (expected.name === name) {
          message += "an error with identical name but a different prototype.";
        } else {
          message += `"${name}"`;
        }
        if (actual.message) {
          message += `\n\nError message:\n\n${actual.message}`;
        }
      } else {
        message += `"${inspect(actual, { depth: -1 })}"`;
      }
    }
    throwError = true;
  } else {
    // Check validation functions return value.
    const res = ReflectApply(expected, {}, [actual]);
    if (res !== true) {
      if (!message) {
        generatedMessage = true;
        const name = expected.name ? `"${expected.name}" ` : "";
        message = `The ${name}validation function is expected to return` +
          ` "true". Received ${inspect(res)}`;

        if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, actual)) {
          message += `\n\nCaught error:\n\n${actual}`;
        }
      }
      throwError = true;
    }
  }

  if (throwError) {
    const err = new AssertionError({
      actual,
      expected,
      message,
      operator: fn.name,
      stackStartFn: fn,
    });
    err.generatedMessage = generatedMessage;
    throw err;
  }
}

function getActual(fn: () => unknown): typeof NO_EXCEPTION_SENTINEL | unknown {
  validateFunction(fn, "fn");
  try {
    fn();
  } catch (e) {
    return e;
  }
  return NO_EXCEPTION_SENTINEL;
}

function checkIsPromise(obj: unknown): obj is Promise<unknown> {
  // Accept native ES6 promises and promises that are implemented in a similar
  // way. Do not accept thenables that use a function as `obj` and that have no
  // `catch` handler.
  return isPromise(obj) ||
    (obj !== null && typeof obj === "object" &&
      typeof obj.then === "function" &&
      typeof obj.catch === "function");
}

async function waitForActual(
  promiseFn: (() => Promise<unknown>) | Promise<unknown>,
): Promise<unknown> {
  let resultPromise;
  if (typeof promiseFn === "function") {
    // Return a rejected promise if `promiseFn` throws synchronously.
    resultPromise = promiseFn();
    // Fail in case no promise is returned.
    if (!checkIsPromise(resultPromise)) {
      throw new ERR_INVALID_RETURN_VALUE(
        "instance of Promise",
        "promiseFn",
        resultPromise,
      );
    }
  } else if (checkIsPromise(promiseFn)) {
    resultPromise = promiseFn;
  } else {
    throw new ERR_INVALID_ARG_TYPE(
      "promiseFn",
      ["Function", "Promise"],
      promiseFn,
    );
  }

  try {
    await resultPromise;
  } catch (e) {
    return e;
  }
  return NO_EXCEPTION_SENTINEL;
}

function expectsError(
  stackStartFn: Function,
  actual: unknown,
  error: AssertPredicate | string | undefined,
  message?: string | Error,
) {
  if (typeof error === "string") {
    if (arguments.length === 4) {
      throw new ERR_INVALID_ARG_TYPE("error", [
        "Object",
        "Error",
        "Function",
        "RegExp",
      ], error);
    }
    if (typeof actual === "object" && actual !== null) {
      if (actual.message === error) {
        throw new ERR_AMBIGUOUS_ARGUMENT(
          "error/message",
          `The error message "${actual.message}" is identical to the message.`,
        );
      }
    } else if (actual === error) {
      throw new ERR_AMBIGUOUS_ARGUMENT(
        "error/message",
        `The error "${actual}" is identical to the message.`,
      );
    }
    message = error;
    error = undefined;
  } else if (
    error != null &&
    typeof error !== "object" &&
    typeof error !== "function"
  ) {
    throw new ERR_INVALID_ARG_TYPE("error", [
      "Object",
      "Error",
      "Function",
      "RegExp",
    ], error);
  }

  if (actual === NO_EXCEPTION_SENTINEL) {
    let details = "";
    if (error?.name) {
      details += ` (${error.name})`;
    }
    details += message ? `: ${message}` : ".";
    const fnType = stackStartFn === rejects ? "rejection" : "exception";
    innerFail({
      actual: undefined,
      expected: error,
      operator: stackStartFn.name,
      message: `Missing expected ${fnType}${details}`,
      stackStartFn,
    });
  }

  if (!error) {
    return;
  }

  expectedException(actual, error, message, stackStartFn);
}

function hasMatchingError(actual: unknown, expected: unknown): boolean {
  if (typeof expected !== "function") {
    if (isRegExp(expected)) {
      const str = String(actual);
      return RegExpPrototypeExec(expected, str) !== null;
    }
    throw new ERR_INVALID_ARG_TYPE(
      "expected",
      ["Function", "RegExp"],
      expected,
    );
  }
  // Guard instanceof against arrow functions as they don't have a prototype.
  if (expected.prototype !== undefined && actual instanceof expected) {
    return true;
  }
  if (ObjectPrototypeIsPrototypeOf(Error, expected)) {
    return false;
  }
  return ReflectApply(expected, {}, [actual]) === true;
}

function expectsNoError(
  stackStartFn: Function,
  actual: unknown,
  error: AssertPredicate | string | undefined,
  message?: string | Error,
) {
  if (actual === NO_EXCEPTION_SENTINEL) {
    return;
  }

  if (typeof error === "string") {
    message = error;
    error = undefined;
  }

  if (!error || hasMatchingError(actual, error)) {
    const details = message ? `: ${message}` : ".";
    const fnType = stackStartFn === doesNotReject ? "rejection" : "exception";
    innerFail({
      actual,
      expected: error,
      operator: stackStartFn.name,
      message: `Got unwanted ${fnType}${details}\n` +
        `Actual message: "${actual?.message}"`,
      stackStartFn,
    });
  }
  throw actual;
}

function throws(
  fn: () => void,
  message?: string,
): void;
function throws(
  fn: () => void,
  error?: Function,
  message?: string | Error,
): void;
function throws(
  fn: () => void,
  error?: RegExp,
  message?: string,
): void;
function throws(
  fn: () => void,
  expected?: AssertPredicate | string,
  message?: Error | string,
) {
  expectsError(throws, getActual(fn), expected, message);
}

function doesNotThrow(
  fn: () => void,
  message?: string,
): void;
function doesNotThrow(
  fn: () => void,
  error?: Function,
  message?: string | Error,
): void;
function doesNotThrow(
  fn: () => void,
  error?: RegExp,
  message?: string,
): void;
function doesNotThrow(
  fn: () => void,
  expected?: AssertPredicate | string,
  message?: Error | string,
) {
  expectsNoError(() => {}, getActual(fn), expected, message);
}

function equal(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (actual != expected && (!NumberIsNaN(actual) || !NumberIsNaN(expected))) {
    innerFail({
      actual,
      expected,
      message,
      operator: "==",
      stackStartFn: equal,
    });
  }
}
function notEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (actual == expected || (NumberIsNaN(actual) && NumberIsNaN(expected))) {
    innerFail({
      actual,
      expected,
      message,
      operator: "!=",
      stackStartFn: notEqual,
    });
  }
}

function strictEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (!ObjectIs(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "strictEqual",
      stackStartFn: strictEqual,
    });
  }
}

function notStrictEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (ObjectIs(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "notStrictEqual",
      stackStartFn: notStrictEqual,
    });
  }
}

function partialDeepStrictEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }
  if (!isPartialStrictEqual(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "partialDeepStrictEqual",
      stackStartFn: partialDeepStrictEqual,
    });
  }
}

function deepEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (!isDeepEqual(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "deepEqual",
      stackStartFn: deepEqual,
    });
  }
}

function notDeepEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (isDeepEqual(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "notDeepEqual",
      stackStartFn: notDeepEqual,
    });
  }
}

function deepStrictEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (!isDeepStrictEqual(actual, expected)) {
    innerFail({
      message,
      actual,
      expected,
      operator: "deepStrictEqual",
      stackStartFn: deepStrictEqual,
    });
  }
}

function notDeepStrictEqual(
  actual: unknown,
  expected: unknown,
  message?: string | Error,
) {
  if (arguments.length < 2) {
    throw new ERR_MISSING_ARGS("actual", "expected");
  }

  if (isDeepStrictEqual(actual, expected)) {
    innerFail({
      actual,
      expected,
      message,
      operator: "notDeepStrictEqual",
      stackStartFn: notDeepStrictEqual,
    });
  }
}

let warned = false;

function fail(
  actual?: string | Error,
  expected?: unknown,
  message?: string | Error,
  operator?: string,
  stackStartFn?: Function,
): never {
  const argsLen = arguments.length;

  let internalMessage = false;
  if (actual == null && argsLen <= 1) {
    internalMessage = true;
    message = "Failed";
  } else if (argsLen === 1) {
    message = actual;
    actual = undefined;
  } else {
    if (warned === false) {
      warned = true;
      // deno-lint-ignore no-process-global
      process.emitWarning(
        "assert.fail() with more than one argument is deprecated. " +
          "Please use assert.strictEqual() instead or only pass a message.",
        "DeprecationWarning",
        "DEP0094",
      );
    }
    if (argsLen === 2) {
      operator = "!=";
    }
  }

  if (message instanceof Error) throw message;

  const errArgs = {
    actual,
    expected,
    operator: operator === undefined ? "fail" : operator,
    stackStartFn: stackStartFn || fail,
    message,
  };
  const err = new AssertionError(errArgs);
  if (internalMessage) {
    err.generatedMessage = true;
  }
  throw err;
}

function internalMatch(
  string: string,
  regexp: RegExp,
  message: string | Error | undefined,
  fn: typeof match | typeof doesNotMatch,
) {
  if (!isRegExp(regexp)) {
    throw new ERR_INVALID_ARG_TYPE(
      "regexp",
      "RegExp",
      regexp,
    );
  }
  const matchFn = fn === match;
  if (
    typeof string !== "string" ||
    RegExpPrototypeExec(regexp, string) !== null !== matchFn
  ) {
    if (message instanceof Error) {
      throw message;
    }

    const generatedMessage = !message;

    // 'The input was expected to not match the regular expression ' +
    message ||= typeof string !== "string"
      ? 'The "string" argument must be of type string. Received type ' +
        `${typeof string} (${inspect(string)})`
      : (matchFn
        ? "The input did not match the regular expression "
        : "The input was expected to not match the regular expression ") +
        `${inspect(regexp)}. Input:\n\n${inspect(string)}\n`;
    const err = new AssertionError({
      actual: string,
      expected: regexp,
      message,
      operator: fn.name,
      stackStartFn: fn,
    });
    err.generatedMessage = generatedMessage;
    throw err;
  }
}

function match(string: string, regexp: RegExp, message?: string | Error) {
  internalMatch(string, regexp, message, match);
}

function doesNotMatch(
  string: string,
  regexp: RegExp,
  message?: string | Error,
) {
  internalMatch(string, regexp, message, doesNotMatch);
}

function strict(actual: unknown, message?: string | Error): asserts actual {
  if (arguments.length === 0) {
    throw new AssertionError({
      message: "No value argument passed to `assert.ok()`",
    });
  }
  assert(actual, message);
}

async function rejects(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  error?: RegExp | Function | Error,
): Promise<void>;

async function rejects(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  message?: string,
): Promise<void>;

// Intentionally avoid using async/await because test-assert-async.js requires it
async function rejects(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  expected?: AssertPredicate | string,
  message?: Error | string,
) {
  expectsError(rejects, await waitForActual(asyncFn), expected, message);
}

async function doesNotReject(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  error?: RegExp | Function,
): Promise<void>;

async function doesNotReject(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  message?: string,
): Promise<void>;

// Intentionally avoid using async/await because test-assert-async.js requires it
async function doesNotReject(
  // deno-lint-ignore no-explicit-any
  asyncFn: Promise<any> | (() => Promise<any>),
  expected?: AssertPredicate | string,
  message?: Error | string,
) {
  expectsNoError(
    doesNotReject,
    await waitForActual(asyncFn),
    expected,
    message,
  );
}

/**
 * Throws `value` if the value is not `null` or `undefined`.
 *
 * @param err
 */
// deno-lint-ignore no-explicit-any
function ifError(err: any) {
  if (err !== null && err !== undefined) {
    let message = "ifError got unwanted exception: ";

    if (typeof err === "object" && typeof err.message === "string") {
      if (err.message.length === 0 && err.constructor) {
        message += err.constructor.name;
      } else {
        message += err.message;
      }
    } else {
      message += inspect(err);
    }

    const newErr = new AssertionError({
      actual: err,
      expected: null,
      operator: "ifError",
      message,
      stackStartFn: ifError,
    });

    // Make sure we actually have a stack trace!
    const origStack = err.stack;

    if (typeof origStack === "string") {
      // This will remove any duplicated frames from the error frames taken
      // from within `ifError` and add the original error frames to the newly
      // created ones.
      const origStackStart = StringPrototypeIndexOf(origStack, "\n    at");
      if (origStackStart !== -1) {
        const originalFrames = StringPrototypeSplit(
          StringPrototypeSlice(origStack, origStackStart + 1),
          "\n",
        );
        // Filter all frames existing in err.stack.
        let newFrames = StringPrototypeSplit(newErr.stack, "\n");
        for (const errFrame of originalFrames) {
          // Find the first occurrence of the frame.
          const pos = ArrayPrototypeIndexOf(newFrames, errFrame);
          if (pos !== -1) {
            // Only keep new frames.
            newFrames = ArrayPrototypeSlice(newFrames, 0, pos);
            break;
          }
        }
        const stackStart = ArrayPrototypeJoin(newFrames, "\n");
        const stackEnd = ArrayPrototypeJoin(originalFrames, "\n");
        newErr.stack = `${stackStart}\n${stackEnd}`;
      }
    }

    throw newErr;
  }
}

const CallTracker_ = deprecate(
  CallTracker,
  "assert.CallTracker is deprecated.",
  "DEP0173",
);

Object.assign(strict, {
  AssertionError,
  CallTracker: CallTracker_,
  deepEqual: deepStrictEqual,
  deepStrictEqual,
  doesNotMatch,
  doesNotReject,
  doesNotThrow,
  equal: strictEqual,
  fail,
  ifError,
  match,
  notDeepEqual: notDeepStrictEqual,
  notDeepStrictEqual,
  notEqual: notStrictEqual,
  notStrictEqual,
  ok,
  rejects,
  strict,
  strictEqual,
  throws,
});

export default Object.assign(assert, {
  AssertionError,
  CallTracker: CallTracker_,
  deepEqual,
  deepStrictEqual,
  doesNotMatch,
  doesNotReject,
  doesNotThrow,
  equal,
  fail,
  ifError,
  match,
  notDeepEqual,
  notDeepStrictEqual,
  notEqual,
  notStrictEqual,
  ok,
  partialDeepStrictEqual,
  rejects,
  strict,
  strictEqual,
  throws,
});

export {
  AssertionError,
  CallTracker_ as CallTracker,
  deepEqual,
  deepStrictEqual,
  doesNotMatch,
  doesNotReject,
  doesNotThrow,
  equal,
  fail,
  ifError,
  match,
  notDeepEqual,
  notDeepStrictEqual,
  notEqual,
  notStrictEqual,
  ok,
  partialDeepStrictEqual,
  rejects,
  strict,
  strictEqual,
  throws,
};
