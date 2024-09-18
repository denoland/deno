// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
