// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";

export function run() {
  notImplemented("test.run");
}

export function test(nameOrOptionsOrFn, optionsOrFn, fn) {
  if (typeof nameOrOptionsOrFn === "undefined") {
    return;
  }

  let testName = "<anonymous>";
  let testOptions = {};
  let testFn = () => {};

  if (typeof nameOrOptionsOrFn === "string" && typeof optionsOrFn === "undefined") {
    if (nameOrOptionsOrFn.length) {
      testName = nameOrOptionsOrFn;
    }
    Deno.test(testName, testFn);
    return;
  }

  if (typeof nameOrOptionsOrFn === "function") {
    testFn = nameOrOptionsOrFn;
    console.log(testName, testFn);
    Deno.test(testName, testFn);
    return;
  }

  if (typeof optionsOrFn === "undefined") {
    testOptions = nameOrOptionsOrFn;
  } else if (typeof fn === "undefined") {
    testOptions = nameOrOptionsOrFn;
    testFn = optionsOrFn;
  }
  console.log(testName, testFn);

  Deno.test(testName, testOptions, testFn);
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
