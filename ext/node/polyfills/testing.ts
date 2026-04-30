// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  Error,
  ErrorPrototype,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  ReflectApply,
  SafeArrayIterator,
  SafePromiseAll,
  SafePromisePrototypeFinally,
  String,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

// --------------------------------------------------------------------------
// Unhandled rejection / uncaught exception handling for node:test
// --------------------------------------------------------------------------
// In Node.js, unhandled rejections and uncaught exceptions during a test
// cause test warnings rather than crashing the runner. In Deno, they're
// fatal for the entire module. We install global handlers that prevent
// Deno from treating them as fatal module errors.

let errorHandlersInstalled = false;

// When a callback-style test is in progress, this holds a reject function
// so that caught async errors can unblock the pending done() callback.
let pendingCallbackReject: ((err: unknown) => void) | null = null;

// Non-Error thrown values with a custom inspect that throws can crash
// Deno's error formatting. Pre-validate before re-throwing to Deno.test.
function sanitizeThrowValue(err: unknown): unknown {
  if (err === null || err === undefined || typeof err !== "object") {
    return err;
  }
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
    return err;
  }
  try {
    const inspectSymbol = SymbolFor("nodejs.util.inspect.custom");
    const inspectFn = (err as Record<symbol, unknown>)[inspectSymbol];
    if (typeof inspectFn === "function") {
      ReflectApply(inspectFn, err, []);
    }
    return err;
  } catch {
    return new Error(
      "test threw a non-Error object with a throwing custom inspect",
    );
  }
}

function installErrorHandlers() {
  if (errorHandlersInstalled) return;
  errorHandlersInstalled = true;

  globalThis.addEventListener("unhandledrejection", (event) => {
    event.preventDefault();
  });

  globalThis.addEventListener("error", (event) => {
    event.preventDefault();
    // If a callback test is pending, unblock it so the test doesn't hang
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}
import { notImplemented } from "ext:deno_node/_utils.ts";
import assert from "node:assert";

const methodsToCopy = [
  "deepEqual",
  "deepStrictEqual",
  "doesNotMatch",
  "doesNotReject",
  "doesNotThrow",
  "equal",
  "fail",
  "ifError",
  "match",
  "notDeepEqual",
  "notDeepStrictEqual",
  "notEqual",
  "notStrictEqual",
  "partialDeepStrictEqual",
  "rejects",
  "strictEqual",
  "throws",
  "ok",
];

/** `assert` object available via t.assert */
let assertObject = undefined;
function getAssertObject() {
  if (assertObject === undefined) {
    assertObject = { __proto__: null };
    ArrayPrototypeForEach(methodsToCopy, (method) => {
      assertObject[method] = assert[method];
    });
  }
  return assertObject;
}

export function run() {
  notImplemented("test.run");
}

function noop() {}

const skippedSymbol = Symbol("skipped");

class NodeTestContext {
  #denoContext: Deno.TestContext;
  #afterHooks: (() => void)[] = [];
  #beforeHooks: (() => void)[] = [];
  #parent: NodeTestContext | undefined;
  #skipped = false;

  constructor(t: Deno.TestContext, parent: NodeTestContext | undefined) {
    this.#denoContext = t;
    this.#parent = parent;
  }

  get [skippedSymbol]() {
    return this.#skipped || (this.#parent?.[skippedSymbol] ?? false);
  }

  get assert() {
    return getAssertObject();
  }

  get signal() {
    notImplemented("test.TestContext.signal");
    return null;
  }

  get name() {
    notImplemented("test.TestContext.name");
    return null;
  }

  diagnostic(message) {
    // deno-lint-ignore no-console
    console.log("DIAGNOSTIC:", message);
  }

  get mock() {
    return mock;
  }

  runOnly() {
    notImplemented("test.TestContext.runOnly");
    return null;
  }

  skip() {
    this.#skipped = true;
    return null;
  }

  todo() {
    this.#skipped = true;
    return null;
  }

  test(name, options, fn) {
    const prepared = prepareOptions(name, options, fn, {});
    // deno-lint-ignore no-this-alias
    const parentContext = this;
    const after = async () => {
      for (const hook of new SafeArrayIterator(this.#afterHooks)) {
        await hook();
      }
    };
    const before = async () => {
      for (const hook of new SafeArrayIterator(this.#beforeHooks)) {
        await hook();
      }
    };
    return PromisePrototypeThen(
      this.#denoContext.step({
        name: prepared.name,
        fn: async (denoTestContext) => {
          const newNodeTextContext = new NodeTestContext(
            denoTestContext,
            parentContext,
          );
          try {
            await before();
            await prepared.fn(newNodeTextContext);
            await after();
          } catch (err) {
            if (!newNodeTextContext[skippedSymbol]) {
              throw err;
            }
            try {
              await after();
            } catch { /* ignore, test is already failing */ }
          }
        },
        ignore: !!prepared.options.todo || !!prepared.options.skip,
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      }),
      () => undefined,
    );
  }

  before(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("before() requires a function");
    }
    ArrayPrototypePush(this.#beforeHooks, fn);
  }

  after(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("after() requires a function");
    }
    ArrayPrototypePush(this.#afterHooks, fn);
  }

  beforeEach(_fn, _options) {
    notImplemented("test.TestContext.beforeEach");
  }

  afterEach(_fn, _options) {
    notImplemented("test.TestContext.afterEach");
  }
}

let currentSuite: TestSuite | null = null;

class TestSuite {
  #denoTestContext: Deno.TestContext;
  steps: Promise<boolean>[] = [];

  constructor(t: Deno.TestContext) {
    this.#denoTestContext = t;
  }

  addTest(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const step = this.#denoTestContext.step({
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(
          denoTestContext,
          undefined,
        );
        try {
          return await prepared.fn(newNodeTextContext);
        } catch (err) {
          if (newNodeTextContext[skippedSymbol]) {
            return undefined;
          } else {
            throw err;
          }
        }
      },
      ignore: !!prepared.options.todo || !!prepared.options.skip,
      sanitizeExit: false,
      sanitizeOps: false,
      sanitizeResources: false,
    });
    ArrayPrototypePush(this.steps, step);
  }

  addSuite(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    // deno-lint-ignore prefer-primordials
    const { promise, resolve } = Promise.withResolvers();
    const step = this.#denoTestContext.step({
      name: prepared.name,
      fn: wrapSuiteFn(prepared.fn, resolve),
      ignore: !!prepared.options.todo || !!prepared.options.skip,
      sanitizeExit: false,
      sanitizeOps: false,
      sanitizeResources: false,
    });
    ArrayPrototypePush(this.steps, step);
    return promise;
  }
}

function prepareOptions(name, options, fn, overrides) {
  if (typeof name === "function") {
    fn = name;
  } else if (name !== null && typeof name === "object") {
    fn = options;
    options = name;
  } else if (typeof options === "function") {
    fn = options;
  }

  if (options === null || typeof options !== "object") {
    options = {};
  }

  const finalOptions = { ...options, ...overrides };
  // TODO(bartlomieju): these options are currently not handled
  // const { concurrency, timeout, signal } = finalOptions;

  if (typeof fn !== "function") {
    fn = noop;
  }

  if (typeof name !== "string" || name === "") {
    name = fn.name || "<anonymous>";
  }

  return { fn, options: finalOptions, name };
}

function wrapTestFn(fn, resolve) {
  return async function (t) {
    const nodeTestContext = new NodeTestContext(t, undefined);
    try {
      if (fn.length >= 2) {
        // Callback-style test
        await new Promise((testResolve, testReject) => {
          // Allow error handler to unblock this test if an async throw occurs
          pendingCallbackReject = testReject;
          const done = (err?: Error) => {
            pendingCallbackReject = null;
            if (err) {
              testReject(err);
            } else {
              testResolve(undefined);
            }
          };
          try {
            const result = ReflectApply(fn, nodeTestContext, [
              nodeTestContext,
              done,
            ]);
            // If the function returns a thenable (async fn with done callback),
            // also listen for its rejection to avoid hanging
            if (
              result !== null && result !== undefined &&
              typeof result.then === "function"
            ) {
              PromisePrototypeThen(result, undefined, (err) => {
                pendingCallbackReject = null;
                testReject(err);
              });
            }
          } catch (err) {
            pendingCallbackReject = null;
            testReject(err);
          }
        });
      } else {
        // Promise-style or sync test
        await ReflectApply(fn, nodeTestContext, [nodeTestContext]);
      }
    } catch (err) {
      if (!nodeTestContext[skippedSymbol]) {
        throw sanitizeThrowValue(err);
      }
    } finally {
      resolve();
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  // TODO(iuioiua): Update once there's a primordial for `Promise.withResolvers()`.
  // deno-lint-ignore prefer-primordials
  const { promise, resolve } = Promise.withResolvers();

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapTestFn(prepared.fn, resolve),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  return promise;
}

function wrapSuiteFn(fn, resolve) {
  return function (t) {
    const prevSuite = currentSuite;
    const suite = currentSuite = new TestSuite(t);
    try {
      fn();
    } finally {
      currentSuite = prevSuite;
    }
    return SafePromisePrototypeFinally(SafePromiseAll(suite.steps), resolve);
  };
}

function prepareDenoTestForSuite(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  // deno-lint-ignore prefer-primordials
  const { promise, resolve } = Promise.withResolvers();

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapSuiteFn(prepared.fn, resolve),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  return promise;
}

export function test(name, options, fn, overrides) {
  installErrorHandlers();
  if (currentSuite) {
    return currentSuite.addTest(name, options, fn, overrides);
  }
  return prepareDenoTest(name, options, fn, overrides);
}

test.skip = function skip(name, options, fn) {
  return test(name, options, fn, { skip: true });
};

test.todo = function todo(name, options, fn) {
  return test(name, options, fn, { todo: true });
};

test.only = function only(name, options, fn) {
  return test(name, options, fn, { only: true });
};

export function suite(name, options, fn, overrides) {
  installErrorHandlers();
  if (currentSuite) {
    return currentSuite.addSuite(name, options, fn, overrides);
  }
  return prepareDenoTestForSuite(name, options, fn, overrides);
}

suite.skip = function skip(name, options, fn) {
  return suite(name, options, fn, { skip: true });
};
suite.todo = function todo(name, options, fn) {
  return suite(name, options, fn, { todo: true });
};
suite.only = function only(name, options, fn) {
  return suite(name, options, fn, { only: true });
};

// Match Node: `it` is just an alias for `test`, and `describe` for `suite`.
// See https://github.com/nodejs/node/blob/main/lib/test.js
export const it = test;
export const describe = suite;

export function before() {
  notImplemented("test.before");
}

export function after() {
  notImplemented("test.after");
}

export function beforeEach() {
  notImplemented("test.beforeEach");
}

export function afterEach() {
  notImplemented("test.afterEach");
}

test.it = test;
test.describe = suite;
test.suite = suite;

// Store all active mocks for restoreAll()
const activeMocks: MockFunctionContext[] = [];

/** Represents a call to a mock function */
interface MockCall {
  arguments: unknown[];
  error?: Error;
  result?: unknown;
  stack: Error;
  target?: unknown;
  this: unknown;
}

/** Context for a mock function with call tracking */
class MockFunctionContext {
  #calls: MockCall[] = [];
  #implementation: ((...args: unknown[]) => unknown) | undefined;
  #restore: (() => void) | undefined;
  #times: number | undefined;

  constructor(
    implementation?: (...args: unknown[]) => unknown,
    restore?: () => void,
    times?: number,
  ) {
    this.#implementation = implementation;
    this.#restore = restore;
    this.#times = times;
  }

  /** Array of call information */
  get calls(): readonly MockCall[] {
    return this.#calls;
  }

  /** Number of times the mock has been called */
  callCount(): number {
    return this.#calls.length;
  }

  /** Reset the call history */
  resetCalls(): void {
    ArrayPrototypeSplice(this.#calls, 0, this.#calls.length);
  }

  /** Restore the original function */
  restore(): void {
    if (this.#restore) {
      this.#restore();
      this.#restore = undefined;
    }
    // Remove from active mocks
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

  /** Internal: record a call */
  _recordCall(
    thisArg: unknown,
    args: unknown[],
    result: unknown,
    error?: Error,
  ): void {
    ArrayPrototypePush(this.#calls, {
      arguments: args,
      error,
      result,
      stack: new Error(),
      this: thisArg,
    });
  }

  /** Internal: check if mock should still be active based on times limit */
  _shouldMock(): boolean {
    if (this.#times === undefined) return true;
    return this.#calls.length < this.#times;
  }

  /** Internal: get the mock implementation */
  _getImplementation(): ((...args: unknown[]) => unknown) | undefined {
    return this.#implementation;
  }
}

/** Creates a mock function wrapper */
function createMockFunction(
  original: ((...args: unknown[]) => unknown) | undefined,
  implementation: ((...args: unknown[]) => unknown) | undefined,
  ctx: MockFunctionContext,
): (...args: unknown[]) => unknown {
  const mockFn = function (this: unknown, ...args: unknown[]): unknown {
    const impl = ctx._shouldMock() ? (implementation ?? original) : original;

    let result: unknown;
    let error: Error | undefined;

    try {
      result = impl ? ReflectApply(impl, this, args) : undefined;
    } catch (e) {
      error = e;
      ctx._recordCall(this, args, undefined, error);
      throw e;
    }

    ctx._recordCall(this, args, result);
    return result;
  };

  // Attach the mock context to the function
  ObjectDefineProperty(mockFn, "mock", {
    __proto__: null,
    value: ctx,
    writable: false,
    enumerable: false,
    configurable: false,
  });

  return mockFn;
}

export const mock = {
  /**
   * Creates a mock function.
   * @param original - Optional original function to wrap
   * @param implementation - Optional mock implementation
   * @param options - Optional configuration
   */
  fn: (
    original?: (...args: unknown[]) => unknown,
    implementation?: (...args: unknown[]) => unknown,
    options?: { times?: number },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    const ctx = new MockFunctionContext(
      implementation ?? original,
      undefined,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(
      original,
      implementation ?? original,
      ctx,
    );
    return mockFn as ((...args: unknown[]) => unknown) & {
      mock: MockFunctionContext;
    };
  },

  /**
   * Mocks a getter on an object.
   */
  getter: (
    _object: object,
    _methodName: string,
    _implementation?: () => unknown,
    _options?: { times?: number },
  ) => {
    notImplemented("test.mock.getter");
  },

  /**
   * Mocks a method on an object.
   * @param object - The object containing the method
   * @param methodName - The name of the method to mock
   * @param implementation - Optional mock implementation
   * @param options - Optional configuration
   */
  method: <T extends object>(
    object: T,
    methodName: keyof T,
    implementation?: (...args: unknown[]) => unknown,
    options?: { times?: number },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    const original = object[methodName] as (
      ...args: unknown[]
    ) => unknown;

    if (typeof original !== "function") {
      throw new TypeError(
        `Cannot mock property '${
          String(methodName)
        }' because it is not a function`,
      );
    }

    const restore = () => {
      object[methodName] = original as T[keyof T];
    };

    const ctx = new MockFunctionContext(
      implementation,
      restore,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(original, implementation, ctx);
    object[methodName] = mockFn as T[keyof T];

    return mockFn as ((...args: unknown[]) => unknown) & {
      mock: MockFunctionContext;
    };
  },

  /**
   * Resets the call history of all mocks.
   */
  reset: (): void => {
    ArrayPrototypeForEach(activeMocks, (ctx) => {
      ctx.resetCalls();
    });
  },

  /**
   * Restores all mocked methods to their original implementations.
   */
  restoreAll: (): void => {
    // Restore in reverse order
    while (activeMocks.length > 0) {
      const ctx = activeMocks[activeMocks.length - 1];
      ctx.restore();
    }
  },

  /**
   * Mocks a setter on an object.
   */
  setter: (
    _object: object,
    _methodName: string,
    _implementation?: (value: unknown) => void,
    _options?: { times?: number },
  ) => {
    notImplemented("test.mock.setter");
  },

  timers: {
    enable: () => {
      notImplemented("test.mock.timers.enable");
    },
    reset: () => {
      notImplemented("test.mock.timers.reset");
    },
    tick: () => {
      notImplemented("test.mock.timers.tick");
    },
    runAll: () => {
      notImplemented("test.mock.timers.runAll");
    },
  },
};

test.test = test;
test.mock = mock;

export default test;
