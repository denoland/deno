// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";

export function deferred() {
  let methods;
  const promise = new Promise((resolve, reject) => {
    methods = {
      async resolve(value) {
        await value;
        resolve(value);
      },
      // deno-lint-ignore no-explicit-any
      reject(reason?: any) {
        reject(reason);
      },
    };
  });
  return Object.assign(promise, methods);
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
      fn: prepared.fn,
      ignore: prepared.options.todo || prepared.options.skip,
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
  const { concurrency, timeout, signal } = finalOptions;

  if (typeof concurrency !== "undefined") {
    warnNotImplemented("test.options.concurrency");
  }
  if (typeof timeout !== "undefined") {
    warnNotImplemented("test.options.timeout");
  }
  if (typeof signal !== "undefined") {
    warnNotImplemented("test.options.signal");
  }

  if (typeof fn !== "function") {
    fn = noop;
  }

  if (typeof name !== "string" || name === "") {
    name = fn.name || "<anonymous>";
  }

  return { fn, options: finalOptions, name };
}

function wrapTestFn(fn, promise) {
  return async function (t) {
    const nodeTestContext = new NodeTestContext(t);
    try {
      await fn(nodeTestContext);
    } finally {
      promise.resolve(undefined);
    }
  };
}

function prepareDenoTest(name, options, fn, overrides) {
  const prepared = prepareOptions(name, options, fn, overrides);

  const promise = deferred();

  const denoTestOptions = {
    name: prepared.name,
    fn: wrapTestFn(prepared.fn, promise),
    only: prepared.options.only,
    ignore: prepared.options.todo || prepared.options.skip,
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

export function describe() {
  notImplemented("test.describe");
}

export function it() {
  notImplemented("test.it");
}

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
