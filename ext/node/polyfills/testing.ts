// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file prefer-primordials

(function () {
"use strict";
const { core, primordials } = globalThis.__bootstrap;
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
  PromiseResolve,
  ReflectApply,
  ReflectConstruct,
  SafeArrayIterator,
  SafeMap,
  String,
  Symbol,
  SymbolFor,
  TypeError,
} = primordials;

let errorHandlersInstalled = false;

let activeNodeTests = 0;

let pendingCallbackReject = null;

function sanitizeThrowValue(err) {
  if (err === null || err === undefined || typeof err !== "object") {
    return err;
  }
  if (ObjectPrototypeIsPrototypeOf(ErrorPrototype, err)) {
    return err;
  }
  const inspectSymbol = SymbolFor("nodejs.util.inspect.custom");
  if (typeof err[inspectSymbol] !== "function") {
    return err;
  }
  try {
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
    if (pendingCallbackReject !== null) {
      pendingCallbackReject(event.error ?? new Error("uncaught error"));
      pendingCallbackReject = null;
    }
  });
}
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const {
  validateFunction,
  validateInteger,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const { default: assert } = core.loadExtScript("ext:deno_node/assert.ts");

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

function run() {
  notImplemented("test.run");
}

function noop() {}

const skippedSymbol = Symbol("skipped");

class TestPlan {
  #expected;
  #actual = 0;

  constructor(count) {
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
  #denoContext;
  #afterHooks = [];
  #beforeHooks = [];
  #parent;
  #skipped = false;
  #name;
  #abortController = new AbortController();
  #plan;
  #planAssert;
  #beforeEachHooks = [];
  #afterEachHooks = [];

  constructor(t, parent, name) {
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

  plan(count) {
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
    if (this.#plan) this.#plan.increment();
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

let currentSuite = null;

const rootBeforeHooks = [];
const rootAfterHooks = [];
const rootBeforeEachHooks = [];
const rootAfterEachHooks = [];
let rootBeforeRan = false;

async function runRootBeforeOnce() {
  if (rootBeforeRan) return;
  rootBeforeRan = true;
  if (rootBeforeHooks.length === 0) return;
  const rootCtx = { name: "<root>", fullName: "<root>" };
  for (const hook of new SafeArrayIterator(rootBeforeHooks)) {
    await hook(rootCtx);
  }
}

async function runRootAfterIfDone() {
  if (activeNodeTests !== 0) return;
  if (rootAfterHooks.length === 0) return;
  const rootCtx = { name: "<root>", fullName: "<root>" };
  // Snapshot and clear so we only run once even if more tests get queued.
  const hooks = ArrayPrototypeSplice(rootAfterHooks, 0, rootAfterHooks.length);
  for (const hook of new SafeArrayIterator(hooks)) {
    try {
      await hook(rootCtx);
    } catch { /* ignore */ }
  }
}

class TestSuite {
  #denoTestContext;
  nodeTestContext;
  entries = [];
  beforeAllHooks = [];
  afterAllHooks = [];
  beforeEachHooks = [];
  afterEachHooks = [];

  constructor(t, nodeTestContext) {
    this.#denoTestContext = t;
    this.nodeTestContext = nodeTestContext;
  }

  addTest(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const beforeEach = this.beforeEachHooks;
    const afterEach = this.afterEachHooks;
    const suiteNodeContext = this.nodeTestContext;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(
          denoTestContext,
          suiteNodeContext,
          prepared.name,
        );
        try {
          for (const hook of new SafeArrayIterator(beforeEach)) {
            await hook(newNodeTextContext);
          }
          let result;
          if (prepared.fn.length >= 2) {
            // Node-style callback API: fn(t, done) - wait for `done()` (or
            // promise rejection) before treating the test as complete.
            await new Promise((testResolve, testReject) => {
              pendingCallbackReject = testReject;
              const done = (err) => {
                pendingCallbackReject = null;
                if (err) testReject(err);
                else testResolve(undefined);
              };
              try {
                const r = ReflectApply(prepared.fn, newNodeTextContext, [
                  newNodeTextContext,
                  done,
                ]);
                if (
                  r !== null && r !== undefined && typeof r.then === "function"
                ) {
                  PromisePrototypeThen(r, undefined, (err) => {
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
            result = await prepared.fn(newNodeTextContext);
          }
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
            try {
              await hook(newNodeTextContext);
            } catch { /* ignore */ }
          }
        }
      },
      ignore: !!prepared.options.todo || !!prepared.options.skip,
    });
  }

  addSuite(name, options, fn, overrides) {
    const prepared = prepareOptions(name, options, fn, overrides);
    const { promise, resolve } = Promise.withResolvers();
    const parentSuiteContext = this.nodeTestContext;
    ArrayPrototypePush(this.entries, {
      name: prepared.name,
      fn: wrapSuiteFn(prepared.fn, resolve, prepared.name, parentSuiteContext),
      ignore: !!prepared.options.todo || !!prepared.options.skip,
    });
    return promise;
  }

  async execute() {
    for (const entry of new SafeArrayIterator(this.entries)) {
      await this.#denoTestContext.step({
        name: entry.name,
        fn: entry.fn,
        ignore: entry.ignore,
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      });
    }
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
    let beforeEachOk = false;
    try {
      await runRootBeforeOnce();
      for (const hook of new SafeArrayIterator(rootBeforeEachHooks)) {
        await hook(nodeTestContext);
      }
      beforeEachOk = true;
      if (fn.length >= 2) {
        await new Promise((testResolve, testReject) => {
          pendingCallbackReject = testReject;
          const done = (err) => {
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
        await ReflectApply(fn, nodeTestContext, [nodeTestContext]);
      }
      nodeTestContext._checkPlan();
    } catch (err) {
      if (!nodeTestContext[skippedSymbol]) {
        throw sanitizeThrowValue(err);
      }
    } finally {
      if (beforeEachOk) {
        for (const hook of new SafeArrayIterator(rootAfterEachHooks)) {
          try {
            await hook(nodeTestContext);
          } catch { /* swallow to match node behavior on hook error */ }
        }
      }
      activeNodeTests--;
      await runRootAfterIfDone();
      resolve();
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  activeNodeTests++;

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapTestFn(prepared.fn, noop, prepared.name),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  // Node resolves the returned promise on test completion, but the
  // Deno runner only executes registered tests after the module
  // finishes evaluating, so top-level `await test(...)` deadlocks.
  // Resolve immediately to unblock; the test still runs and is
  // reported normally. Trade-off: code that awaits `test()` for
  // sequencing (`await test('a'); await test('b')`) sees them run
  // out of order.
  return PromiseResolve();
}

function wrapSuiteFn(fn, resolve, name, parentNodeContext) {
  return async function (t) {
    const isTopLevel = parentNodeContext === undefined;
    if (isTopLevel) await runRootBeforeOnce();
    const suiteNodeContext = new NodeTestContext(t, parentNodeContext, name);
    const prevSuite = currentSuite;
    const suite = currentSuite = new TestSuite(t, suiteNodeContext);
    try {
      fn(suiteNodeContext);
    } finally {
      currentSuite = prevSuite;
    }
    try {
      for (const hook of new SafeArrayIterator(suite.beforeAllHooks)) {
        await hook();
      }
      await suite.execute();
    } finally {
      try {
        for (const hook of new SafeArrayIterator(suite.afterAllHooks)) {
          await hook();
        }
      } finally {
        if (isTopLevel) {
          activeNodeTests--;
          await runRootAfterIfDone();
        }
        resolve();
      }
    }
  };
}

function prepareDenoTestForSuite(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  activeNodeTests++;

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapSuiteFn(prepared.fn, noop, prepared.name, undefined),
    only: prepared.options.only,
    ignore: !!prepared.options.todo || !!prepared.options.skip,
    sanitizeOnly: false,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  // See `prepareDenoTest` for the Node-divergence trade-off; top-level
  // `await suite(...)` would deadlock if we waited for completion.
  return PromiseResolve();
}

function test(name, options, fn, overrides) {
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

function suite(name, options, fn, overrides) {
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

const it = test;
const describe = suite;

function before(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("before() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeAllHooks, fn);
    return;
  }
  ArrayPrototypePush(rootBeforeHooks, fn);
}

function after(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("after() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterAllHooks, fn);
    return;
  }
  ArrayPrototypePush(rootAfterHooks, fn);
}

function beforeEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("beforeEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.beforeEachHooks, fn);
    return;
  }
  ArrayPrototypePush(rootBeforeEachHooks, fn);
}

function afterEach(fn, _options) {
  if (typeof fn !== "function") {
    throw new TypeError("afterEach() requires a function argument");
  }
  if (currentSuite) {
    ArrayPrototypePush(currentSuite.afterEachHooks, fn);
    return;
  }
  ArrayPrototypePush(rootAfterEachHooks, fn);
}

test.it = test;
test.describe = suite;
test.suite = suite;
test.before = before;
test.after = after;
test.beforeEach = beforeEach;
test.afterEach = afterEach;

const activeMocks = [];

class MockFunctionContext {
  #calls = [];
  #implementation;
  #restore;
  #times;
  #onceImplementations = new SafeMap();

  constructor(implementation, restore, times) {
    this.#implementation = implementation;
    this.#restore = restore;
    this.#times = times;
  }

  get calls() {
    return this.#calls;
  }

  callCount() {
    return this.#calls.length;
  }

  mockImplementation(implementation) {
    validateFunction(implementation, "implementation");
    this.#implementation = implementation;
  }

  mockImplementationOnce(implementation, onCall) {
    validateFunction(implementation, "implementation");
    if (onCall !== undefined) {
      validateInteger(onCall, "onCall", 0);
    }
    const call = onCall ?? this.#calls.length;
    MapPrototypeSet(this.#onceImplementations, call, implementation);
  }

  resetCalls() {
    ArrayPrototypeSplice(this.#calls, 0, this.#calls.length);
  }

  restore() {
    if (this.#restore) {
      this.#restore();
      this.#restore = undefined;
    }
    this._restored = true;
    const idx = ArrayPrototypeIndexOf(activeMocks, this);
    if (idx !== -1) {
      ArrayPrototypeSplice(activeMocks, idx, 1);
    }
  }

  _recordCall(thisArg, args, result, error, target) {
    ArrayPrototypePush(this.#calls, {
      arguments: args,
      error,
      result,
      stack: new Error(),
      target,
      this: thisArg,
    });
  }

  _shouldMock() {
    if (this._restored) return false;
    if (this.#times === undefined) return true;
    return this.#calls.length < this.#times;
  }

  _getImplementation() {
    return this.#implementation;
  }

  _nextImpl() {
    const nextCall = this.#calls.length;
    const onceImpl = MapPrototypeGet(this.#onceImplementations, nextCall);
    if (onceImpl) {
      MapPrototypeDelete(this.#onceImplementations, nextCall);
      return onceImpl;
    }
    return this.#implementation;
  }
}

function createMockFunction(original, implementation, ctx) {
  const mockFn = function (...args) {
    const newTarget = new.target;
    const isCtor = newTarget !== undefined;
    // The IIFE wrapping this module is sloppy, so a plain call leaks
    // globalThis as `this`. Match strict-mode/Node semantics.
    const thisArg = !isCtor && this === globalThis ? undefined : this;
    const impl = ctx._shouldMock()
      ? (ctx._nextImpl() ?? implementation ?? original)
      : original;

    let result;
    let error;

    // If called directly (not via subclass), use the original constructor
    // so the produced instance has its prototype, and so call.target reports
    // the user's class (not the mock wrapper).
    const ctorTarget = isCtor && newTarget === mockFn ? impl : newTarget;
    try {
      if (isCtor) {
        result = impl ? ReflectConstruct(impl, args, ctorTarget) : undefined;
      } else {
        result = impl ? ReflectApply(impl, thisArg, args) : undefined;
      }
    } catch (e) {
      error = e;
      ctx._recordCall(
        isCtor ? thisArg : thisArg,
        args,
        undefined,
        error,
        ctorTarget,
      );
      throw e;
    }

    ctx._recordCall(
      isCtor ? result : thisArg,
      args,
      result,
      undefined,
      ctorTarget,
    );
    return result;
  };

  ObjectDefineProperty(mockFn, "mock", {
    __proto__: null,
    value: ctx,
    writable: false,
    enumerable: false,
    configurable: false,
  });

  return mockFn;
}

function findPropertyDescriptor(obj, name) {
  let current = obj;
  while (current !== null && current !== undefined) {
    const desc = ObjectGetOwnPropertyDescriptor(current, name);
    if (desc) return desc;
    current = ObjectGetPrototypeOf(current);
  }
  return undefined;
}

function mockMethodImpl(object, methodName, implementation, options) {
  if (
    implementation !== null && typeof implementation === "object" &&
    typeof implementation !== "function"
  ) {
    options = implementation;
    implementation = undefined;
  }

  const descriptor = findPropertyDescriptor(object, methodName);
  if (!descriptor) {
    throw new TypeError(
      `Cannot mock property '${String(methodName)}' because it does not exist`,
    );
  }

  const isGetter = options?.getter ?? false;
  const isSetter = options?.setter ?? false;

  let original;
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
    ObjectDefineProperty(object, methodName, descriptor);
  };

  const impl = implementation === undefined ? original : implementation;
  const ctx = new MockFunctionContext(impl, restore, options?.times);
  ArrayPrototypePush(activeMocks, ctx);

  const mockFn = createMockFunction(original, impl, ctx);

  const mockDescriptor = {
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

  ObjectDefineProperty(object, methodName, mockDescriptor);

  return mockFn;
}

const mock = {
  fn: (original, implementation, options) => {
    if (original !== null && typeof original === "object") {
      options = original;
      original = undefined;
      implementation = undefined;
    } else if (implementation !== null && typeof implementation === "object") {
      options = implementation;
      implementation = original;
    }

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
    return mockFn;
  },

  getter: (object, methodName, implementation, options) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation;
      implementation = undefined;
    }
    return mockMethodImpl(object, methodName, implementation, {
      ...options,
      getter: true,
    });
  },

  method: (object, methodName, implementation, options) => {
    return mockMethodImpl(object, methodName, implementation, options);
  },

  reset: () => {
    ArrayPrototypeForEach(activeMocks, (ctx) => {
      ctx.resetCalls();
    });
  },

  restoreAll: () => {
    while (activeMocks.length > 0) {
      const ctx = activeMocks[activeMocks.length - 1];
      ctx.restore();
    }
  },

  setter: (object, methodName, implementation, options) => {
    if (implementation !== null && typeof implementation === "object") {
      options = implementation;
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
test.before = before;
test.after = after;
test.beforeEach = beforeEach;
test.afterEach = afterEach;
test.run = run;

return {
  run,
  test,
  suite,
  it,
  describe,
  before,
  after,
  beforeEach,
  afterEach,
  mock,
  default: test,
};
})();
