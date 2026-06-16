// Copyright 2018-2026 the Deno authors. MIT license.

import events, {
  addAbortListener,
  errorMonitor,
  EventEmitter,
} from "node:events";
import * as eventsNs from "node:events";
import { createRequire } from "node:module";
import { assert, assertEquals, assertStrictEquals } from "@std/assert";

EventEmitter.captureRejections = true;

Deno.test("regression #20441", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const ee = new EventEmitter();

  ee.on("foo", function () {
    const p = new Promise((_resolve, reject) => {
      setTimeout(() => {
        reject();
      }, 100);
    });
    return p;
  });

  ee.on("error", function (_: unknown) {
    resolve();
  });

  ee.emit("foo");
  await promise;
});

Deno.test("eventemitter async resource", () => {
  // @ts-ignore: @types/node is outdated
  class Foo extends events.EventEmitterAsyncResource {}

  const foo = new Foo();
  // @ts-ignore: @types/node is outdated
  foo.emit("bar");
});

Deno.test("addAbortListener", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const abortController = new AbortController();
  addAbortListener(abortController.signal, () => {
    resolve();
  });
  abortController.abort();
  await promise;
});

Deno.test("EventEmitter works when Object.create is deleted (#29929)", () => {
  const ObjectCreate = Object.create;
  Object.create = undefined!;
  try {
    const emitter = new EventEmitter();
    let called = false;
    emitter.on("foo", () => {
      called = true;
    });
    emitter.emit("foo");
    if (!called) throw new Error("Listener was not called");
  } finally {
    Object.create = ObjectCreate;
  }
});

Deno.test("EventEmitter works if Array.prototype.unshift is deleted", () => {
  const ArrayPrototypeUnshift = Array.prototype.unshift;
  // @ts-ignore -- this is fine for testing purposes
  delete Array.prototype.unshift;
  try {
    const emitter = new EventEmitter();
    let called = false;
    emitter.on("bar", () => {
      called = true;
    });
    emitter.emit("bar");
    if (!called) throw new Error("Listener was not called");
  } finally {
    Array.prototype.unshift = ArrayPrototypeUnshift;
  }
});

Deno.test("EventEmitter works if Array.prototype.push is deleted", () => {
  const ArrayPrototypePush = Array.prototype.push;
  // @ts-ignore -- this is fine for testing purposes
  delete Array.prototype.push;
  try {
    const emitter = new EventEmitter();
    let called = false;
    emitter.on("baz", () => {
      called = true;
    });
    emitter.emit("baz");
    if (!called) throw new Error("Listener was not called");
  } finally {
    Array.prototype.push = ArrayPrototypePush;
  }
});

Deno.test("node:events default and namespace shape matches Node", () => {
  // The default export is the EventEmitter constructor with named exports
  // attached as static properties (issue #34261).
  assertStrictEquals(events, EventEmitter);
  assertStrictEquals(eventsNs.default, EventEmitter);

  // Both ESM default and namespace expose addAbortListener as a function.
  assertEquals(typeof events.addAbortListener, "function");
  // @ts-ignore: @types/node namespace shape is incomplete here
  assertEquals(typeof eventsNs.addAbortListener, "function");
  assertStrictEquals(events.addAbortListener, addAbortListener);
  // @ts-ignore: @types/node namespace shape is incomplete here
  assertStrictEquals(eventsNs.addAbortListener, addAbortListener);

  // errorMonitor must be a symbol on the EventEmitter function, on the
  // namespace import, and on the named import (all three were broken before).
  assertEquals(typeof events.errorMonitor, "symbol");
  assertEquals(typeof eventsNs.errorMonitor, "symbol");
  assertEquals(typeof errorMonitor, "symbol");
  assertStrictEquals(events.errorMonitor, errorMonitor);
  assertStrictEquals(eventsNs.errorMonitor, errorMonitor);

  // CommonJS require() must agree with the ESM default export.
  const require = createRequire(import.meta.url);
  const cjsEvents = require("node:events");
  assertStrictEquals(cjsEvents, EventEmitter);
  assertEquals(typeof cjsEvents.addAbortListener, "function");
  assertStrictEquals(cjsEvents.addAbortListener, addAbortListener);
  assertStrictEquals(cjsEvents.errorMonitor, errorMonitor);
});

Deno.test("require('node:events').addAbortListener works (issue #34261)", async () => {
  const require = createRequire(import.meta.url);
  const { addAbortListener: addAbortListenerCjs } = require("node:events");
  assert(typeof addAbortListenerCjs === "function");

  const { promise, resolve } = Promise.withResolvers<void>();
  const ac = new AbortController();
  addAbortListenerCjs(ac.signal, () => resolve());
  ac.abort();
  await promise;
});

Deno.test("EventEmitter works if Object.setPrototypeOf is deleted", () => {
  const ObjectSetPrototypeOf = Object.setPrototypeOf;
  Object.setPrototypeOf = undefined!;
  try {
    const emitter = new EventEmitter();
    let called = false;
    emitter.on("zap", () => {
      called = true;
    });
    emitter.emit("zap");
    if (!called) throw new Error("Listener was not called");
  } finally {
    Object.setPrototypeOf = ObjectSetPrototypeOf;
  }
});
