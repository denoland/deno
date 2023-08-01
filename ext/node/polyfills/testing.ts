// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";

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

  test(_name, _options, _fn) {
    notImplemented("test.TestContext.test");
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

function prepareDenoTest(name, options, fn) {
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

  // TODO(bartlomieju): warn once on each unsupported option
  const { todo, only, concurrency, timeout, signal } = options;

  if (typeof fn !== "function") {
    fn = noop;
  }

  if (typeof name !== "string" || name === "") {
    name = fn.name || "<anonymous>";
  }

  const wrappedFn = async (t) => {
    const nodeTestContext = new NodeTestContext(t);
    await fn(nodeTestContext);
  };

  const denoTestOptions = {
    name,
    fn: wrappedFn,
    only,
    ignore: todo,
  };
  return Deno.test(denoTestOptions);
}

export function test(name, options, fn) {
  prepareDenoTest(name, options, fn);
  // TODO(bartlomieju): fix return type
}

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
