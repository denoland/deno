// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  PromisePrototypeThen,
  ArrayPrototypePush,
  ArrayPrototypeForEach,
  SafePromiseAll,
  SafePromisePrototypeFinally,
} = primordials;
import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";
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

class NodeTestContext {
  #denoContext: Deno.TestContext;

  constructor(t: Deno.TestContext) {
    this.#denoContext = t;
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
    warnNotImplemented("test.TestContext.skip");
    return null;
  }

  todo() {
    warnNotImplemented("test.TestContext.todo");
    return null;
  }

  test(name, options, fn) {
    const prepared = prepareOptions(name, options, fn, {});
    return PromisePrototypeThen(
      this.#denoContext.step({
        name: prepared.name,
        fn: async (denoTestContext) => {
          const newNodeTextContext = new NodeTestContext(denoTestContext);
          await prepared.fn(newNodeTextContext);
        },
        ignore: prepared.options.todo || prepared.options.skip,
        sanitizeExit: false,
        sanitizeOps: false,
        sanitizeResources: false,
      }),
      () => undefined,
    );
  }

  before(_fn, _options) {
    notImplemented("test.TestContext.before");
  }

  after(_fn, _options) {
    notImplemented("test.TestContext.after");
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
      fn: (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(denoTestContext);
        return prepared.fn(newNodeTextContext);
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
    const nodeTestContext = new NodeTestContext(t);
    try {
      await fn(nodeTestContext);
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
