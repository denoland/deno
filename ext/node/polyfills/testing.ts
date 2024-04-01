// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";

export function run() {
  notImplemented("test.run");
}

function noop() {}

type SuiteFn = () => unknown;

class NodeSuiteContext {
  before: SuiteFn | null = null;
  beforeEach: SuiteFn | null = null;
  afterEach: SuiteFn | null = null;
  after: SuiteFn | null = null;
  didRunBefore = false;
  firstTestId = -1;
  lastTestId = -1;
  only = false;
  skip = false;

  constructor(public parent: NodeSuiteContext | null, public name: string) {}
}
let TEST_ID = 0;
const ROOT_SUITE = new NodeSuiteContext(null, "");
let CURRENT_SUITE = ROOT_SUITE;

class NodeTestContext {
  #denoContext: Deno.TestContext;

  constructor(t: Deno.TestContext) {
    this.#denoContext = t;
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
    return this.#denoContext.step({
      name: prepared.name,
      fn: async (denoTestContext) => {
        const newNodeTextContext = new NodeTestContext(denoTestContext);
        await prepared.fn(newNodeTextContext);
      },
      ignore: prepared.options.todo || prepared.options.skip,
      sanitizeExit: false,
      sanitizeOps: false,
      sanitizeResources: false,
    }).then(() => undefined);
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

function wrapTestFn(
  fn: (ctx: NodeTestContext, done: () => void) => unknown,
  ancestors: NodeSuiteContext[],
  id: number,
  resolve: () => void,
) {
  return async function (t) {
    let i = ancestors.length;
    while (i--) {
      const ancestor = ancestors[i];
      if (ancestor.firstTestId === id) {
        await ancestor.before?.();
      }

      await ancestor.beforeEach?.();
    }

    const nodeTestContext = new NodeTestContext(t);
    try {
      await fn(nodeTestContext, () => {
        throw "done";
      });
    } catch (err) {
      if (err !== "done") {
        throw err;
      }
    } finally {
      try {
        for (let i = 0; i < ancestors.length; i++) {
          const ancestor = ancestors[i];
          await ancestor.afterEach?.();

          if (ancestor.lastTestId === id) {
            await ancestor.after?.();
          }
        }
      } finally {
        resolve();
      }
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  const { promise, resolve } = Promise.withResolvers<void>();
  let ignore = prepared.options.todo || prepared.options.skip;
  let id = -1;
  if (!ignore) {
    id = TEST_ID++;
    if (CURRENT_SUITE.firstTestId === -1) {
      CURRENT_SUITE.firstTestId = id;
    }

    CURRENT_SUITE.lastTestId = id;
  }

  let testName = prepared.name;
  const ancestors: NodeSuiteContext[] = [];
  let parent: NodeSuiteContext | null = CURRENT_SUITE;
  let only = prepared.options.only;

  while (parent !== null && parent.parent !== null) {
    ancestors.push(parent);

    testName = parent.name + " > " + testName;
    if (!ignore) {
      if (parent.firstTestId === -1) {
        parent.firstTestId = id;
      }

      if (parent.only) {
        only = true;
      }
      if (parent.skip) {
        ignore = true;
        only = false;
      }
    }

    parent = parent.parent;
  }

  const denoTestOptions = {
    name: testName,
    fn: wrapTestFn(prepared.fn, ancestors, id, resolve),
    only,
    ignore,
    sanitizeExit: false,
    sanitizeOps: false,
    sanitizeResources: false,
  };
  Deno.test(denoTestOptions);
  return promise;
}

export function test(name, options, fn) {
  return prepareDenoTest(name, options, fn, {});
}

test.skip = function skip(name, options, fn) {
  return prepareDenoTest(name, options, fn, { skip: true });
};

test.todo = function todo(name, options, fn) {
  return prepareDenoTest(name, options, fn, { todo: true });
};

test.only = function only(name, options, fn) {
  return prepareDenoTest(name, options, fn, { only: true });
};

function prepareDescribe(
  name: string,
  fn: () => unknown,
  options: { skip?: boolean; only?: boolean } = {},
) {
  const ctx = new NodeSuiteContext(CURRENT_SUITE, name);
  ctx.only = Boolean(options.only);
  ctx.skip = Boolean(options.skip);

  const prev = CURRENT_SUITE;
  CURRENT_SUITE = ctx;
  try {
    fn();
  } finally {
    CURRENT_SUITE = prev;
    prev.lastTestId = TEST_ID;
  }
}

export function describe(
  name: string,
  fn: () => unknown,
): void | Promise<void> {
  return prepareDescribe(name, fn);
}
describe.only = (name: string, fn: () => unknown) => {
  return prepareDescribe(name, fn, { only: true });
};
describe.skip = (name: string, fn: () => unknown) => {
  return prepareDescribe(name, fn, { skip: true });
};

export const it = test;

export function before(fn: () => unknown) {
  CURRENT_SUITE.before = fn;
}

export function after(fn: () => unknown) {
  CURRENT_SUITE.after = fn;
}

export function beforeEach(fn: () => unknown) {
  CURRENT_SUITE.beforeEach = fn;
}

export function afterEach(fn: () => unknown) {
  CURRENT_SUITE.afterEach = fn;
}

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

export default test;
