// Copyright 2018-2025 the Deno authors. MIT license.

import events, { addAbortListener, EventEmitter } from "node:events";
import assert from "node:assert";

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

  ee.on("error", function (_) {
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

Deno.test("emit stack trace", () => {
  const ee = new EventEmitter();
  ee.on("foo", () => {
    throw new Error("foo error");
  });
  let error;
  try {
    ee.emit("foo");
  } catch (err) {
    error = err;
  }

  // @ts-ignore
  assert(error.stack.includes("at EventEmitter.emit (node:events"));
});
