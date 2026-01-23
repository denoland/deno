// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  Error,
  ObjectDefineProperty,
  Promise,
  PromisePrototypeThen,
  ReflectApply,
  SafeArrayIterator,
  SafePromiseAll,
  SafePromisePrototypeFinally,
  String,
  Symbol,
  TypeError,
} = primordials;
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
    notImplemented("test.TestContext.mock");
    return null;
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
        ignore: prepared.options.todo || prepared.options.skip,
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
      ignore: prepared.options.todo || prepared.options.skip,
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
      ignore: prepared.options.todo || prepared.options.skip,
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
      // Check if the test function expects a done callback (2 parameters)
      if (fn.length >= 2) {
        // Callback-style async test
        await new Promise((testResolve, testReject) => {
          const done = (err?: Error) => {
            if (err) {
              testReject(err);
            } else {
              testResolve(undefined);
            }
          };
          try {
            fn(nodeTestContext, done);
          } catch (err) {
            testReject(err);
          }
        });
      } else {
        // Promise-style or sync test
        await fn(nodeTestContext);
      }
    } catch (err) {
      if (!nodeTestContext[skippedSymbol]) {
        throw err;
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
    ignore: prepared.options.todo || prepared.options.skip,
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
    ignore: prepared.options.todo || prepared.options.skip,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  return promise;
}

export function test(name, options, fn, overrides) {
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

export function describe(name, options, fn) {
  return suite(name, options, fn, {});
}

describe.skip = function skip(name, options, fn) {
  return suite.skip(name, options, fn);
};
describe.todo = function todo(name, options, fn) {
  return suite.todo(name, options, fn);
};
describe.only = function only(name, options, fn) {
  return suite.only(name, options, fn);
};

export function suite(name, options, fn, overrides) {
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

export function it(name, options, fn) {
  return test(name, options, fn, {});
}

it.skip = function skip(name, options, fn) {
  return test.skip(name, options, fn);
};

it.todo = function todo(name, options, fn) {
  return test.todo(name, options, fn);
};

it.only = function only(name, options, fn) {
  return test.only(name, options, fn);
};

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

test.it = it;
test.describe = describe;
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
