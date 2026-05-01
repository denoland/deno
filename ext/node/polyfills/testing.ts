// Copyright 2018-2026 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeForEach,
  ArrayPrototypeIndexOf,
  ArrayPrototypePush,
  ArrayPrototypeSplice,
  Error,
  ErrorPrototype,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  ObjectDefineProperty,
  ObjectGetOwnPropertyDescriptor,
  ObjectGetPrototypeOf,
  ObjectPrototypeIsPrototypeOf,
  Promise,
  PromisePrototypeThen,
  ReflectApply,
  SafeArrayIterator,
  SafeMap,
  SafePromiseAll,
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

// Tracks the number of node:test tests currently executing. The global error
// handlers only suppress events while at least one test is running, so they
// don't interfere with Deno.test or other code after node:test completes.
let activeNodeTests = 0;

// When a callback-style test is in progress, this holds a reject function
// so that caught async errors can unblock the pending done() callback.
// node:test tests run sequentially via Deno.test (one top-level test at a
// time), so only one callback test can be pending at any given moment.
let pendingCallbackReject: ((err: unknown) => void) | null = null;

// Non-Error thrown values with a custom inspect that throws can crash
// Deno's error formatting. Test the actual formatting path before
// re-throwing to Deno.test.
function sanitizeThrowValue(err: unknown): unknown {
  if (err === null || err === undefined || typeof err !== "object") {
    return err;
  }
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
    return err;
  }
  // Only objects with a custom inspect symbol need validation
  const inspectSymbol = SymbolFor("nodejs.util.inspect.custom");
  if (typeof (err as Record<symbol, unknown>)[inspectSymbol] !== "function") {
    return err;
  }
  try {
    // Test the actual formatting path that Deno's error reporter uses
    Deno.inspect(err);
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
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
  });

  globalThis.addEventListener("error", (event) => {
    if (activeNodeTests > 0) {
      event.preventDefault();
    }
    // If a callback test is pending, unblock it so the test doesn't hang
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}
import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  validateFunction,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";
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

class TestPlan {
  #expected: number;
  #actual: number = 0;

  constructor(count: number) {
    this.#expected = count;
  }

  increment() {
    this.#actual++;
  }

  check() {
    if (this.#actual !== this.#expected) {
      throw new Error(
        `plan expected ${this.#expected} assertion(s) but received ${this.#actual}`,
      );
    }
  }
}

class NodeTestContext {
  #denoContext: Deno.TestContext;
  #afterHooks: (() => void)[] = [];
  #beforeHooks: (() => void)[] = [];
  #parent: NodeTestContext | undefined;
  #skipped = false;
  #name: string;
  #abortController: AbortController = new AbortController();
  #plan: TestPlan | undefined;
  #planAssert: Record<string, unknown> | undefined;
  #beforeEachHooks: (() => void | Promise<void>)[] = [];
  #afterEachHooks: (() => void | Promise<void>)[] = [];

  constructor(
    t: Deno.TestContext,
    parent: NodeTestContext | undefined,
    name: string,
  ) {
    this.#denoContext = t;
    this.#parent = parent;
    this.#name = name;
  }

  get [skippedSymbol]() {
    return this.#skipped || (this.#parent?.[skippedSymbol] ?? false);
  }

  get assert() {
    if (this.#plan) {
      if (!this.#planAssert) {
        const plan = this.#plan;
        const base = getAssertObject();
        const wrapped = { __proto__: null };
        ArrayPrototypeForEach(methodsToCopy, (method) => {
          wrapped[method] = function (...args) {
            plan.increment();
            return ReflectApply(base[method], this, args);
          };
        });
        this.#planAssert = wrapped;
      }
      return this.#planAssert;
    }
    return getAssertObject();
  }

  plan(count: number) {
    validateInteger(count, "count", 1);
    this.#plan = new TestPlan(count);
  }

  _checkPlan() {
    if (this.#plan) this.#plan.check();
  }

  get signal() {
    return this.#abortController.signal;
  }

  get name() {
    return this.#name;
  }

  get fullName() {
    if (this.#parent) {
      return this.#parent.fullName + " > " + this.#name;
    }
    return this.#name;
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
            prepared.name,
          );
          try {
            await before();
            for (
              const hook of new SafeArrayIterator(
                parentContext.#beforeEachHooks,
              )
            ) {
              await hook();
            }
            await prepared.fn(newNodeTextContext);
            newNodeTextContext._checkPlan();
            await after();
          } catch (err) {
            if (!newNodeTextContext[skippedSymbol]) {
              throw err;
            }
            try {
              await after();
            } catch { /* ignore, test is already failing */ }
          } finally {
            for (
              const hook of new SafeArrayIterator(
                parentContext.#afterEachHooks,
              )
            ) {
              await hook();
            }
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

  beforeEach(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("beforeEach() requires a function");
    }
    ArrayPrototypePush(this.#beforeEachHooks, fn);
  }

  afterEach(fn, _options) {
    if (typeof fn !== "function") {
      throw new TypeError("afterEach() requires a function");
    }
    ArrayPrototypePush(this.#afterEachHooks, fn);
  }
}

let currentSuite: TestSuite | null = null;

class TestSuite {
  #denoTestContext: Deno.TestContext;
  steps: Promise<boolean>[] = [];
  beforeAllHooks: (() => void | Promise<void>)[] = [];
  afterAllHooks: (() => void | Promise<void>)[] = [];
  beforeEachHooks: (() => void | Promise<void>)[] = [];
  afterEachHooks: (() => void | Promise<void>)[] = [];

  constructor(t: Deno.TestContext) {
    this.#denoTestContext = t;
  }

  addTest(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const beforeEach = this.beforeEachHooks;
    const afterEach = this.afterEachHooks;
    const step = this.#denoTestContext.step({
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(
          denoTestContext,
          undefined,
          prepared.name,
        );
        try {
          for (const hook of new SafeArrayIterator(beforeEach)) {
            await hook();
          }
          const result = await prepared.fn(newNodeTextContext);
          newNodeTextContext._checkPlan();
          return result;
        } catch (err) {
          if (newNodeTextContext[skippedSymbol]) {
            return undefined;
          } else {
            throw err;
          }
        } finally {
          for (const hook of new SafeArrayIterator(afterEach)) {
            await hook();
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

function wrapTestFn(fn, resolve, name) {
  return async function (t) {
    const nodeTestContext = new NodeTestContext(t, undefined, name);
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
      nodeTestContext._checkPlan();
    } catch (err) {
      if (!nodeTestContext[skippedSymbol]) {
        throw sanitizeThrowValue(err);
      }
    } finally {
      activeNodeTests--;
      resolve();
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  // Increment at registration so handlers stay active until all tests complete.
  // Decremented in wrapTestFn's finally block when the test finishes.
  activeNodeTests++;

  // TODO(iuioiua): Update once there's a primordial for `Promise.withResolvers()`.
  // deno-lint-ignore prefer-primordials
  const { promise, resolve } = Promise.withResolvers();

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapTestFn(prepared.fn, resolve, prepared.name),
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
  return async function (t) {
    const prevSuite = currentSuite;
    const suite = currentSuite = new TestSuite(t);
    try {
      fn();
    } finally {
      currentSuite = prevSuite;
    }
    try {
      for (const hook of new SafeArrayIterator(suite.beforeAllHooks)) {
        await hook();
      }
      await SafePromiseAll(suite.steps);
    } finally {
      try {
        for (const hook of new SafeArrayIterator(suite.afterAllHooks)) {
          await hook();
        }
      } finally {
        activeNodeTests--;
        resolve();
      }
    }
  };
}

function prepareDenoTestForSuite(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  // Increment at registration so handlers stay active until all tests complete.
  // Decremented in wrapSuiteFn's finally callback when the suite finishes.
  activeNodeTests++;

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

export function before(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("before() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeAllHooks, fn);
    return;
  }
  notImplemented("test.before (module-level, outside suite)");
}

export function after(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("after() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterAllHooks, fn);
    return;
  }
  notImplemented("test.after (module-level, outside suite)");
}

export function beforeEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("beforeEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeEachHooks, fn);
    return;
  }
  notImplemented("test.beforeEach (module-level, outside suite)");
}

export function afterEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("afterEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterEachHooks, fn);
    return;
  }
  notImplemented("test.afterEach (module-level, outside suite)");
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
  #onceImplementations: Map<
    number,
    (...args: unknown[]) => unknown
  > = new SafeMap();

  constructor(
    implementation?: (...args: unknown[]) => unknown,
    restore?: () => void,
    times?: number,
  ) {
    this.#implementation = implementation;
    this.#restore = restore;
    this.#times = times;
  }

  get calls(): readonly MockCall[] {
    return this.#calls;
  }

  callCount(): number {
    return this.#calls.length;
  }

  mockImplementation(
    implementation: (...args: unknown[]) => unknown,
  ): void {
    validateFunction(implementation, "implementation");
    this.#implementation = implementation;
  }

  mockImplementationOnce(
    implementation: (...args: unknown[]) => unknown,
    onCall?: number,
  ): void {
    validateFunction(implementation, "implementation");
    if (onCall !== undefined) {
      validateInteger(onCall, "onCall", 0);
    }
    const call = onCall ?? this.#calls.length;
    MapPrototypeSet(this.#onceImplementations, call, implementation);
  }

  resetCalls(): void {
    ArrayPrototypeSplice(this.#calls, 0, this.#calls.length);
  }

  restore(): void {
    if (this.#restore) {
      this.#restore();
      this.#restore = undefined;
    }
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

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

  _shouldMock(): boolean {
    if (this.#times === undefined) return true;
    return this.#calls.length < this.#times;
  }

  _getImplementation(): ((...args: unknown[]) => unknown) | undefined {
    return this.#implementation;
  }

  _nextImpl(): ((...args: unknown[]) => unknown) | undefined {
    const nextCall = this.#calls.length;
    const onceImpl = MapPrototypeGet(this.#onceImplementations, nextCall);
    if (onceImpl) {
      MapPrototypeDelete(this.#onceImplementations, nextCall);
      return onceImpl as (...args: unknown[]) => unknown;
    }
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
    const impl = ctx._shouldMock()
      ? (ctx._nextImpl() ?? implementation ?? original)
      : original;

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

function findPropertyDescriptor(
  obj: object,
  name: string | symbol,
): PropertyDescriptor | undefined {
  let current = obj;
  while (current !== null && current !== undefined) {
    const desc = ObjectGetOwnPropertyDescriptor(current, name);
    if (desc) return desc;
    current = ObjectGetPrototypeOf(current);
  }
  return undefined;
}

type MockMethodOptions = { times?: number; getter?: boolean; setter?: boolean };

function mockMethodImpl<T extends object>(
  object: T,
  methodName: keyof T,
  implementation:
    | ((...args: unknown[]) => unknown)
    | Record<string, unknown>
    | undefined,
  options?: MockMethodOptions,
): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } {
  // Handle overloaded signature: method(obj, name, options)
  if (
    implementation !== null && typeof implementation === "object" &&
    typeof implementation !== "function"
  ) {
    options = implementation as MockMethodOptions;
    implementation = undefined;
  }

  const descriptor = findPropertyDescriptor(object, methodName as string);
  if (!descriptor) {
    throw new TypeError(
      `Cannot mock property '${String(methodName)}' because it does not exist`,
    );
  }

  const isGetter = options?.getter ?? false;
  const isSetter = options?.setter ?? false;

  // deno-lint-ignore no-explicit-any
  let original: ((...args: any[]) => any) | undefined;
  if (isGetter) {
    original = descriptor.get;
  } else if (isSetter) {
    original = descriptor.set;
  } else {
    original = descriptor.value;
  }

  if (typeof original !== "function") {
    throw new TypeError(
      `Cannot mock property '${
        String(methodName)
      }' because it is not a function`,
    );
  }

  const restore = () => {
    ObjectDefineProperty(object, methodName as string, descriptor);
  };

  const impl = implementation === undefined ? original : implementation;
  const ctx = new MockFunctionContext(
    impl as (...args: unknown[]) => unknown,
    restore,
    options?.times,
  );
  ArrayPrototypePush(activeMocks, ctx);

  const mockFn = createMockFunction(
    original as (...args: unknown[]) => unknown,
    impl as (...args: unknown[]) => unknown,
    ctx,
  );

  const mockDescriptor: PropertyDescriptor = {
    configurable: descriptor.configurable,
    enumerable: descriptor.enumerable,
  };

  if (isGetter) {
    mockDescriptor.get = mockFn;
    mockDescriptor.set = descriptor.set;
  } else if (isSetter) {
    mockDescriptor.get = descriptor.get;
    mockDescriptor.set = mockFn;
  } else {
    mockDescriptor.writable = descriptor.writable;
    mockDescriptor.value = mockFn;
  }

  ObjectDefineProperty(object, methodName as string, mockDescriptor);

  return mockFn as ((...args: unknown[]) => unknown) & {
    mock: MockFunctionContext;
  };
}

export const mock = {
  fn: (
    original?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    // Handle overloaded signatures: fn(options), fn(original, options)
    if (original !== null && typeof original === "object") {
      options = original as { times?: number };
      original = undefined;
      implementation = undefined;
    } else if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = original as
        | ((...args: unknown[]) => unknown)
        | undefined;
    }

    const ctx = new MockFunctionContext(
      (implementation ?? original) as
        | ((...args: unknown[]) => unknown)
        | undefined,
      undefined,
      options?.times,
    );
    ArrayPrototypePush(activeMocks, ctx);

    const mockFn = createMockFunction(
      original as ((...args: unknown[]) => unknown) | undefined,
      (implementation ?? original) as
        | ((...args: unknown[]) => unknown)
        | undefined,
      ctx,
    );
    return mockFn as ((...args: unknown[]) => unknown) & {
      mock: MockFunctionContext;
    };
  },

  getter: <T extends object>(
    object: T,
    methodName: keyof T,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number },
  ) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = undefined;
    }
    return mockMethodImpl(object, methodName, implementation, {
      ...options,
      getter: true,
    });
  },

  method: <T extends object>(
    object: T,
    methodName: keyof T,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number },
  ): ((...args: unknown[]) => unknown) & { mock: MockFunctionContext } => {
    return mockMethodImpl(object, methodName, implementation, options);
  },

  reset: (): void => {
    ArrayPrototypeForEach(activeMocks, (ctx: MockFunctionContext) => {
      ctx.resetCalls();
    });
  },

  restoreAll: (): void => {
    while (activeMocks.length > 0) {
      const ctx = activeMocks[activeMocks.length - 1];
      ctx.restore();
    }
  },

  setter: <T extends object>(
    object: T,
    methodName: keyof T,
    implementation?:
      | ((...args: unknown[]) => unknown)
      | Record<string, unknown>,
    options?: { times?: number },
  ) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation as { times?: number };
      implementation = undefined;
    }
    return mockMethodImpl(object, methodName, implementation, {
      ...options,
      setter: true,
    });
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
