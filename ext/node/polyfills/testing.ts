// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  PromisePrototypeThen,
  ArrayPrototypePush,
  ArrayPrototypeForEach,
  SafePromiseAll,
  TypeError,
  SafeArrayIterator,
  SafePromisePrototypeFinally,
  Symbol,
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
      await fn(nodeTestContext);
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

export const mock = {
  fn: () => {
    notImplemented("test.mock.fn");
  },
  getter: () => {
    notImplemented("test.mock.getter");
  },
  method: () => {
    notImplemented("test.mock.method");
  },
  reset: () => {
    notImplemented("test.mock.reset");
  },
  restoreAll: () => {
    notImplemented("test.mock.restoreAll");
  },
  setter: () => {
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

export default test;
