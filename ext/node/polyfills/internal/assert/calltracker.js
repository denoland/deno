// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  Error,
  FunctionPrototype,
  ObjectFreeze,
  Proxy,
  ReflectApply,
  SafeSet,
  SafeWeakMap,
} = primordials;

import { codes } from "ext:deno_node/internal/errors.ts";
const {
  ERR_INVALID_ARG_VALUE,
  ERR_UNAVAILABLE_DURING_EXIT,
} = codes;
import { AssertionError } from "ext:deno_node/assertion_error.ts";
import { validateUint32 } from "ext:deno_node/internal/validators.mjs";

const noop = FunctionPrototype;

class CallTrackerContext {
  #expected;
  #calls;
  #name;
  #stackTrace;
  constructor({ expected, stackTrace, name }) {
    this.#calls = [];
    this.#expected = expected;
    this.#stackTrace = stackTrace;
    this.#name = name;
  }

  track(thisArg, args) {
    const argsClone = ObjectFreeze(ArrayPrototypeSlice(args));
    ArrayPrototypePush(
      this.#calls,
      ObjectFreeze({ thisArg, arguments: argsClone }),
    );
  }

  get delta() {
    return this.#calls.length - this.#expected;
  }

  reset() {
    this.#calls = [];
  }
  getCalls() {
    return ObjectFreeze(ArrayPrototypeSlice(this.#calls));
  }

  report() {
    if (this.delta !== 0) {
      const message = `Expected the ${this.#name} function to be ` +
        `executed ${this.#expected} time(s) but was ` +
        `executed ${this.#calls.length} time(s).`;
      return {
        message,
        actual: this.#calls.length,
        expected: this.#expected,
        operator: this.#name,
        stack: this.#stackTrace,
      };
    }
  }
}

class CallTracker {
  #callChecks = new SafeSet();
  #trackedFunctions = new SafeWeakMap();

  #getTrackedFunction(tracked) {
    if (!this.#trackedFunctions.has(tracked)) {
      throw new ERR_INVALID_ARG_VALUE(
        "tracked",
        tracked,
        "is not a tracked function",
      );
    }
    return this.#trackedFunctions.get(tracked);
  }

  reset(tracked) {
    if (tracked === undefined) {
      this.#callChecks.forEach((check) => check.reset());
      return;
    }

    this.#getTrackedFunction(tracked).reset();
  }

  getCalls(tracked) {
    return this.#getTrackedFunction(tracked).getCalls();
  }

  calls(fn, expected = 1) {
    // deno-lint-ignore no-process-global
    if (process._exiting) {
      throw new ERR_UNAVAILABLE_DURING_EXIT();
    }
    if (typeof fn === "number") {
      expected = fn;
      fn = noop;
    } else if (fn === undefined) {
      fn = noop;
    }

    validateUint32(expected, "expected", true);

    const context = new CallTrackerContext({
      expected,
      // eslint-disable-next-line no-restricted-syntax
      stackTrace: new Error(),
      name: fn.name || "calls",
    });
    const tracked = new Proxy(fn, {
      __proto__: null,
      apply(fn, thisArg, argList) {
        context.track(thisArg, argList);
        return ReflectApply(fn, thisArg, argList);
      },
    });
    this.#callChecks.add(context);
    this.#trackedFunctions.set(tracked, context);
    return tracked;
  }

  report() {
    const errors = [];
    for (const context of this.#callChecks) {
      const message = context.report();
      if (message !== undefined) {
        ArrayPrototypePush(errors, message);
      }
    }
    return errors;
  }

  verify() {
    const errors = this.report();
    if (errors.length === 0) {
      return;
    }
    const message = errors.length === 1
      ? errors[0].message
      : "Functions were not called the expected number of times";
    throw new AssertionError({
      message,
      details: errors,
    });
  }
}

export { CallTracker };
