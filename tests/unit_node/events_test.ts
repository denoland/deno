// Copyright 2018-2025 the Deno authors. MIT license.

import events, { addAbortListener, EventEmitter } from "node:events";

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
